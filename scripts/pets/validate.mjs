#!/usr/bin/env node
// Validates the bundled pets against the Codex pet atlas contract.
//
//   node scripts/pets/validate.mjs [petDir...]
//
// With no arguments, checks every bundled pet in src-tauri/pets/, and that
// those folders are exactly PET_IDS. Checks:
//   - pet.json parses and has id/displayName/description/spritesheetPath,
//     with a one-line description
//   - the atlas is exactly 1536x1872
//   - every used cell has opaque pixels; every unused cell is fully transparent
//   - nothing is drawn in the outermost 2 px of any cell (no bleeding)
//   - pixels are fully opaque or fully transparent (no anti-aliasing)
//   - the art sits on a 4x4 grid (nearest-neighbour scaled pixel art)
//   - the feet baseline (lowest opaque row) stays within +-2 art px across
//     the idle / waiting / running / review / failed frames
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { PNG } from 'pngjs';
import { ATLAS_ROWS, BASELINE_ROWS, PET_IDS } from './lib/contract.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const ATLAS_W = 1536;
const ATLAS_H = 1872;
const CELL_W = 192;
const CELL_H = 208;
const COLS = 8;
const MARGIN = 2;
const GRID = 4;
const BASELINE_TOLERANCE = 2; // art px

export function validatePet(dir) {
  const errors = [];
  const info = [];
  const fail = (msg) => errors.push(msg);

  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(path.join(dir, 'pet.json'), 'utf8'));
  } catch (e) {
    fail(`pet.json: ${e.message}`);
    return { errors, info };
  }
  for (const key of ['id', 'displayName', 'description', 'spritesheetPath']) {
    if (typeof manifest[key] !== 'string' || !manifest[key]) fail(`pet.json: missing "${key}"`);
  }
  if (typeof manifest.description === 'string' && /[\r\n]/.test(manifest.description)) {
    fail('pet.json: description must be one line');
  }
  if (manifest.id && manifest.id !== path.basename(dir)) {
    fail(`pet.json: id "${manifest.id}" does not match folder "${path.basename(dir)}"`);
  }
  const sheetPath = path.join(dir, manifest.spritesheetPath ?? 'spritesheet.png');
  if (!fs.existsSync(sheetPath)) {
    fail(`spritesheet not found: ${sheetPath}`);
    return { errors, info };
  }
  const size = fs.statSync(sheetPath).size;
  if (size > 20 * 1024 * 1024) fail(`spritesheet is ${size} bytes (> 20 MiB)`);

  const png = PNG.sync.read(fs.readFileSync(sheetPath));
  if (png.width !== ATLAS_W || png.height !== ATLAS_H) {
    fail(`atlas is ${png.width}x${png.height}, expected ${ATLAS_W}x${ATLAS_H}`);
    return { errors, info };
  }
  const alpha = (x, y) => png.data[(y * png.width + x) * 4 + 3];
  const rgba = (x, y) => png.data.readUInt32BE((y * png.width + x) * 4);

  // Whole-image checks: binary alpha, 4x4 grid, palette size.
  const colors = new Set();
  let partial = 0;
  let offGrid = 0;
  for (let y = 0; y < ATLAS_H; y++) {
    for (let x = 0; x < ATLAS_W; x++) {
      const a = alpha(x, y);
      if (a !== 0 && a !== 255) partial++;
      if (a) colors.add(rgba(x, y));
      const gx = x - (x % GRID);
      const gy = y - (y % GRID);
      if (rgba(x, y) !== rgba(gx, gy)) offGrid++;
    }
  }
  if (partial) fail(`${partial} semi-transparent pixels (expected hard pixel edges)`);
  if (offGrid) fail(`${offGrid} pixels break the ${GRID}x${GRID} art grid`);
  info.push(`${colors.size} colours`);

  const baselines = {};
  ATLAS_ROWS.forEach((row, r) => {
    for (let c = 0; c < COLS; c++) {
      const x0 = c * CELL_W;
      const y0 = r * CELL_H;
      let opaque = 0;
      let edge = 0;
      let lowest = -1;
      for (let y = 0; y < CELL_H; y++) {
        for (let x = 0; x < CELL_W; x++) {
          if (!alpha(x0 + x, y0 + y)) continue;
          opaque++;
          lowest = Math.max(lowest, y);
          if (x < MARGIN || y < MARGIN || x >= CELL_W - MARGIN || y >= CELL_H - MARGIN) edge++;
        }
      }
      const where = `row ${r} (${row.name}) frame ${c}`;
      if (c < row.frames) {
        if (!opaque) fail(`${where}: used cell is empty`);
        if (edge) fail(`${where}: ${edge} pixels inside the ${MARGIN}px cell margin`);
        if (BASELINE_ROWS.includes(row.name)) (baselines[row.name] ??= []).push(lowest);
      } else if (opaque) {
        fail(`${where}: unused cell has ${opaque} opaque pixels`);
      }
    }
  });

  const ref = baselines.idle?.[0];
  if (ref != null) {
    for (const [name, rows] of Object.entries(baselines)) {
      rows.forEach((lowest, c) => {
        const diff = Math.abs(lowest - ref) / GRID;
        if (diff > BASELINE_TOLERANCE) {
          fail(`row ${name} frame ${c}: feet baseline off by ${diff} art px (lowest row ${lowest}, idle ${ref})`);
        }
      });
    }
    info.push(`baseline y=${ref}px`);
  }
  return { errors, info };
}

/** Every bundled pet folder in src-tauri/pets. */
export function bundledPetDirs() {
  const root = path.join(ROOT, 'src-tauri', 'pets');
  return fs
    .readdirSync(root, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => path.join(root, d.name));
}

/** The bundled folders must be exactly the pets the generator builds. */
export function checkBundledSet(dirs) {
  const found = dirs.map((d) => path.basename(d));
  const missing = PET_IDS.filter((id) => !found.includes(id));
  const extra = found.filter((id) => !PET_IDS.includes(id));
  return [
    ...missing.map((id) => `missing bundled pet "${id}" (run build.mjs)`),
    ...extra.map((id) => `src-tauri/pets/${id} is not in PET_IDS`),
  ];
}

function main(args) {
  const dirs = args.length ? args.map((d) => path.resolve(d)) : bundledPetDirs();
  let failed = 0;
  for (const dir of dirs) {
    const { errors, info } = validatePet(dir);
    const name = path.basename(dir);
    if (errors.length) {
      failed++;
      console.log(`FAIL ${name}`);
      for (const e of errors) console.log(`  - ${e}`);
    } else {
      console.log(`ok   ${name} (${info.join(', ')})`);
    }
  }
  if (!args.length) {
    for (const e of checkBundledSet(dirs)) {
      failed++;
      console.log(`FAIL ${e}`);
    }
  }
  if (!dirs.length) {
    console.log('no pets found');
    return 1;
  }
  return failed ? 1 : 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  process.exit(main(process.argv.slice(2)));
}
