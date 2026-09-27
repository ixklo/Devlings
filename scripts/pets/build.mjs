#!/usr/bin/env node
// Builds Devlings' bundled pets in the Codex pet format.
//
//   node scripts/pets/build.mjs
//
// Writes, for each pet id:
//   src-tauri/pets/<id>/pet.json
//   src-tauri/pets/<id>/spritesheet.png   1536x1872, 8x9 cells of 192x208
//   docs/pets/preview-<id>.png            labelled contact sheet (0.5x)
//   docs/pets/preview-<id>-big.png        idle frame 0 at 3x
// and, for all of them together:
//   docs/pets/lineup.png                  every pet idle, side by side and named
//   docs/pets/check-0.6x.png              key moods at the app's default size (0.6x), light and dark
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { CELL_W, CELL_H, COLS, newImage, drawImage, fillRect, writePng } from './lib/engine.mjs';
import { ATLAS_ROWS, PET_IDS } from './lib/contract.mjs';
import { PETS } from './lib/pets.mjs';
import { buildAtlas, manifestOf } from './lib/atlas.mjs';
import { drawText } from './lib/font.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

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

/** Bounding box of the opaque pixels in one cell. */
function cellBounds(atlas, r, c) {
  let x0 = CELL_W;
  let y0 = CELL_H;
  let x1 = -1;
  let y1 = -1;
  for (let y = 0; y < CELL_H; y++) {
    for (let x = 0; x < CELL_W; x++) {
      if (!atlas.data[((r * CELL_H + y) * atlas.width + c * CELL_W + x) * 4 + 3]) continue;
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
    }
  }
  return { x0, y0, x1, y1 };
}

// 0.75x keeps every art pixel exactly 3 screen pixels square.
function lineup(atlases) {
  const s = 0.75;
  const boxes = atlases.map(({ atlas }) => cellBounds(atlas, 0, 0));
  // one shared vertical range, so pets keep their heights (the ghost floats)
  const y0 = Math.min(...boxes.map((b) => b.y0));
  const y1 = Math.max(...boxes.map((b) => b.y1));
  const slot = Math.ceil(Math.max(...boxes.map((b) => b.x1 - b.x0 + 1)) * s) + 16;
  const pad = 24;
  const artH = Math.ceil((y1 - y0 + 1) * s);
  const W = pad * 2 + slot * atlases.length;
  const H = pad + artH + 56;
  const img = newImage(W, H, [255, 255, 255, 255]);
  atlases.forEach(({ id, atlas }, i) => {
    const b = boxes[i];
    const w = Math.ceil((b.x1 - b.x0 + 1) * s);
    const x = pad + i * slot + Math.floor((slot - w) / 2);
    drawImage(img, atlas, x, pad, { scale: s, sx: b.x0, sy: y0, sw: b.x1 - b.x0 + 1, sh: y1 - y0 + 1 });
    const name = PETS[id].displayName;
    const species = PETS[id].species;
    const cx = pad + i * slot + slot / 2;
    drawText(img, name, Math.round(cx - (name.length * 12 - 2) / 2), pad + artH + 12, INK, 2);
    drawText(img, species, Math.round(cx - (species.length * 12 - 2) / 2), pad + artH + 34, MUTED, 2);
  });
  return img;
}

// The moods that matter most, at exactly the size the app draws by default.
const CHECK = [
  ['idle', 0, 0],
  ['run', 1, 1],
  ['wave', 3, 1],
  ['failed', 5, 2],
  ['waiting', 6, 0],
  ['working', 7, 1],
  ['review', 8, 2],
];
function check(atlases) {
  const s = 0.6;
  const cw = Math.round(CELL_W * s);
  const ch = Math.round(CELL_H * s);
  const labelW = 120;
  const gap = 4;
  const top = 34;
  const block = CHECK.length * (cw + gap);
  const W = labelW + block * 2 + 24;
  const H = top + atlases.length * (ch + gap) + 8;
  const img = newImage(W, H, [246, 247, 249, 255]);
  CHECK.forEach(([label], k) => {
    drawText(img, label, labelW + k * (cw + gap) + 4, 10, MUTED, 2);
    drawText(img, label, labelW + block + 16 + k * (cw + gap) + 4, 10, MUTED, 2);
  });
  atlases.forEach(({ id, atlas }, i) => {
    const y = top + i * (ch + gap);
    drawText(img, PETS[id].displayName, 12, Math.round(y + ch / 2 - 8), INK, 2);
    CHECK.forEach(([, r, c], k) => {
      const lx = labelW + k * (cw + gap);
      const dx = labelW + block + 16 + k * (cw + gap);
      fillRect(img, lx, y, cw, ch, [255, 255, 255, 255]);
      fillRect(img, dx, y, cw, ch, [31, 31, 31, 255]);
      for (const x of [lx, dx]) drawImage(img, atlas, x, y, { scale: s, sx: c * CELL_W, sy: r * CELL_H, sw: CELL_W, sh: CELL_H });
    });
  });
  return img;
}

const missing = PET_IDS.filter((id) => !PETS[id]);
const extra = Object.keys(PETS).filter((id) => !PET_IDS.includes(id));
if (missing.length || extra.length) {
  throw new Error(`PET_IDS and PETS disagree (missing: ${missing.join(', ') || '-'}; extra: ${extra.join(', ') || '-'})`);
}

const built = [];
for (const id of PET_IDS) {
  const atlas = buildAtlas(id);
  const dir = path.join(ROOT, 'src-tauri', 'pets', id);
  writePng(path.join(dir, 'spritesheet.png'), atlas);
  fs.writeFileSync(path.join(dir, 'pet.json'), JSON.stringify(manifestOf(id), null, 2) + '\n');
  writePng(path.join(ROOT, 'docs', 'pets', `preview-${id}.png`), contactSheet(id, atlas));
  writePng(path.join(ROOT, 'docs', 'pets', `preview-${id}-big.png`), bigPreview(atlas));
  console.log(`built ${id}: ${path.relative(ROOT, dir)}`);
  built.push({ id, atlas });
}
writePng(path.join(ROOT, 'docs', 'pets', 'lineup.png'), lineup(built));
writePng(path.join(ROOT, 'docs', 'pets', 'check-0.6x.png'), check(built));
console.log('built docs/pets/lineup.png and docs/pets/check-0.6x.png');
