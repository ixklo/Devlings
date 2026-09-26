#!/usr/bin/env node
// Builds Perch's bundled pets in the Codex pet format.
//
//   node scripts/pets/build.mjs
//
// Writes, for each pet id:
//   src-tauri/pets/<id>/pet.json
//   src-tauri/pets/<id>/spritesheet.png   1536x1872, 8x9 cells of 192x208
//   docs/pets/preview-<id>.png            labelled contact sheet (0.5x)
//   docs/pets/preview-<id>-big.png        idle frame 0 at 3x
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  ATLAS_W, ATLAS_H, CELL_W, CELL_H, COLS,
  newImage, paintLayer, drawImage, fillRect, writePng,
} from './lib/engine.mjs';
import { PETS, paletteFor } from './lib/palettes.mjs';
import { ROW_SPECS } from './lib/rows.mjs';
import { ATLAS_ROWS, PET_IDS } from './lib/contract.mjs';
import { drawText } from './lib/font.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

function buildFrames() {
  return ATLAS_ROWS.map((row, r) => {
    const spec = ROW_SPECS[r];
    if (spec.name !== row.name) throw new Error(`Row ${r}: expected ${row.name}, got ${spec.name}`);
    const frames = spec.frames();
    if (frames.length !== row.frames) {
      throw new Error(`Row ${row.name}: expected ${row.frames} frames, got ${frames.length}`);
    }
    return frames;
  });
}

function buildAtlas(frames, palette) {
  const img = newImage(ATLAS_W, ATLAS_H);
  frames.forEach((row, r) => {
    row.forEach((layer, c) => paintLayer(img, layer, palette, c * CELL_W, r * CELL_H));
  });
  return img;
}

const INK = [40, 44, 52, 255];
const MUTED = [120, 128, 140, 255];

function checker(img, x, y, w, h, a, b, size = 8) {
  for (let j = 0; j < h; j++) {
    for (let i = 0; i < w; i++) {
      const on = (Math.floor(i / size) + Math.floor(j / size)) % 2 === 0;
      img.data.set(on ? a : b, ((y + j) * img.width + (x + i)) * 4);
    }
  }
}

function contactSheet(id, atlas) {
  const s = 0.5;
  const cw = CELL_W * s;
  const ch = CELL_H * s;
  const gap = 4;
  const labelW = 214;
  const top = 64;
  const darkX = labelW + COLS * (cw + gap) + 16;
  const W = darkX + cw + 16;
  const H = top + ATLAS_ROWS.length * (ch + gap) + 12;
  const img = newImage(W, H, [246, 247, 249, 255]);
  drawText(img, `${id}  -  codex pet atlas 1536x1872  (cells at 0.5x)`, 16, 14, INK, 2);
  for (let c = 0; c < COLS; c++) drawText(img, String(c), labelW + c * (cw + gap) + cw / 2 - 5, 42, MUTED, 2);
  drawText(img, 'on dark', darkX + 6, 42, MUTED, 2);
  ATLAS_ROWS.forEach((row, r) => {
    const y = top + r * (ch + gap);
    drawText(img, `${r} ${row.name}`, 16, y + ch / 2 - 14, INK, 2);
    drawText(img, `${row.frames} frames`, 16, y + ch / 2 + 6, MUTED, 2);
    for (let c = 0; c < COLS; c++) {
      const x = labelW + c * (cw + gap);
      if (c < row.frames) checker(img, x, y, cw, ch, [255, 255, 255, 255], [233, 236, 241, 255]);
      else fillRect(img, x, y, cw, ch, [226, 229, 234, 255]);
      drawImage(img, atlas, x, y, { scale: s, sx: c * CELL_W, sy: r * CELL_H, sw: CELL_W, sh: CELL_H });
    }
    fillRect(img, darkX, y, cw, ch, [31, 31, 31, 255]);
    drawImage(img, atlas, darkX, y, { scale: s, sx: 0, sy: r * CELL_H, sw: CELL_W, sh: CELL_H });
  });
  return img;
}

function bigPreview(atlas) {
  const s = 3;
  const img = newImage(CELL_W * s, CELL_H * s, [238, 240, 244, 255]);
  drawImage(img, atlas, 0, 0, { scale: s, sx: 0, sy: 0, sw: CELL_W, sh: CELL_H });
  return img;
}

const frames = buildFrames();
for (const id of PET_IDS) {
  const pet = PETS[id];
  const atlas = buildAtlas(frames, paletteFor(id));
  const dir = path.join(ROOT, 'src-tauri', 'pets', id);
  writePng(path.join(dir, 'spritesheet.png'), atlas);
  const manifest = {
    id,
    displayName: pet.displayName,
    description: pet.description,
    spritesheetPath: 'spritesheet.png',
  };
  fs.writeFileSync(path.join(dir, 'pet.json'), JSON.stringify(manifest, null, 2) + '\n');
  writePng(path.join(ROOT, 'docs', 'pets', `preview-${id}.png`), contactSheet(id, atlas));
  writePng(path.join(ROOT, 'docs', 'pets', `preview-${id}-big.png`), bigPreview(atlas));
  console.log(`built ${id}: ${path.relative(ROOT, dir)}`);
}
