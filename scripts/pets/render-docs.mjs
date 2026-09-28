#!/usr/bin/env node
// Renders the README images that come straight from the pet atlases (no browser needed):
//
//   cd scripts/pets && npm install && node render-docs.mjs
//
// Writes:
//   docs/screenshots/pets.gif   the nine built-in pets idling side by side, on a transparent background
//   docs/pets/template.png      a 1536x1872 guide for drawing your own pet (see docs/making-pets.md)
//
// Frame timings come from the atlas contract (lib/contract.mjs), so the pets move at the app's speed.
// The UI screenshots, the hero GIF and the social preview come from capture-ui.mjs instead.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import gifenc from 'gifenc';
import { CELL_W, CELL_H, COLS, SCALE, ART_W, ART_H, newImage, fillRect, readPng, writePng } from './lib/engine.mjs';
import { ATLAS_ROWS, BASELINE_ROWS, PET_IDS } from './lib/contract.mjs';
import { PETS } from './lib/pets.mjs';
import { drawText, textWidth } from './lib/font.mjs';

const { GIFEncoder } = gifenc;
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

/** The atlas cell at (row, col), sampled back down to its 1x art grid. */
export function artFrame(atlas, row, col) {
  const img = newImage(ART_W, ART_H);
  for (let y = 0; y < ART_H; y++) {
    for (let x = 0; x < ART_W; x++) {
      const si = ((row * CELL_H + y * SCALE) * atlas.width + col * CELL_W + x * SCALE) * 4;
      img.data.set(atlas.data.subarray(si, si + 4), (y * ART_W + x) * 4);
    }
  }
  return img;
}

/** Smallest box holding every opaque pixel of every frame, so a pet never jumps inside its slot. */
export function unionBox(frames) {
  let x0 = ART_W, y0 = ART_H, x1 = -1, y1 = -1;
  for (const f of frames) {
    for (let y = 0; y < ART_H; y++) {
      for (let x = 0; x < ART_W; x++) {
        if (f.data[(y * ART_W + x) * 4 + 3] === 0) continue;
        x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y);
      }
    }
  }
  return { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 };
}

/** Paint part of an art frame onto dst at an integer scale (the art is fully opaque or fully transparent). */
export function blit(dst, art, box, ox, oy, scale) {
  for (let y = 0; y < box.h; y++) {
    for (let x = 0; x < box.w; x++) {
      const si = ((box.y + y) * ART_W + box.x + x) * 4;
      if (art.data[si + 3] < 128) continue;
      fillRect(dst, ox + x * scale, oy + y * scale, scale, scale, [...art.data.subarray(si, si + 3), 255]);
    }
  }
}

/**
 * Encodes RGBA frames whose pixels are opaque or fully transparent, with one exact shared palette
 * (index 0 is transparent). Runs of identical frames merge into one longer frame. Every delay must
 * be at least 20 ms: browsers slow shorter GIF frames down to 100 ms.
 */
export function encodeTransparentGif(frames, file) {
  const key = (d, i) => (d[i] << 16) | (d[i + 1] << 8) | d[i + 2];
  const colours = new Map();
  for (const { img } of frames) {
    for (let i = 0; i < img.data.length; i += 4) {
      if (img.data[i + 3] >= 128 && !colours.has(key(img.data, i))) colours.set(key(img.data, i), colours.size + 1);
    }
  }
  if (colours.size > 255) throw new Error(`${file}: ${colours.size} colours, a GIF allows 255 plus transparency`);
  const palette = [[0, 0, 0], ...[...colours.keys()].map((k) => [(k >> 16) & 255, (k >> 8) & 255, k & 255])];
  while (palette.length < 2) palette.push([0, 0, 0]);

  const merged = [];
  for (const f of frames) {
    if (f.delay < 20) throw new Error(`${file}: a ${f.delay} ms frame (browsers show anything under 20 ms for 100 ms)`);
    const prev = merged[merged.length - 1];
    if (prev && Buffer.compare(prev.img.data, f.img.data) === 0) prev.delay += f.delay;
    else merged.push({ img: f.img, delay: f.delay });
  }

  const gif = GIFEncoder();
  merged.forEach(({ img, delay }, n) => {
    const index = new Uint8Array(img.width * img.height);
    for (let p = 0; p < index.length; p++) index[p] = img.data[p * 4 + 3] >= 128 ? colours.get(key(img.data, p * 4)) : 0;
    // Dispose 2: clear to transparent before the next frame, so a pet that moves leaves nothing behind.
    gif.writeFrame(index, img.width, img.height, {
      ...(n === 0 ? { palette, repeat: 0 } : {}),
      delay,
      transparent: true,
      transparentIndex: 0,
      dispose: 2,
    });
  });
  gif.finish();
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, gif.bytes());
  return { frames: merged.length, colours: colours.size, bytes: fs.statSync(file).size };
}

const loadAtlas = (id) => readPng(path.join(ROOT, 'src-tauri', 'pets', id, 'spritesheet.png'));
const rowIndex = (name) => ATLAS_ROWS.findIndex((r) => r.name === name);

// Readable on GitHub's light (#ffffff) and dark (#0d1117) pages alike.
const LABEL = [125, 133, 144, 255];

/**
 * All nine pets idling in a row, named, on a transparent background. Drawn at 4 px per art pixel so it stays
 * crisp on high-density screens when the README shows it at half size. One idle loop, neighbours at
 * different points in it so they don't bob in step (three phases keep the frame count, and the file, small);
 * frames are sampled every 20 ms.
 */
const PHASES = [0, 370, 740];
export function petsFrames() {
  const idle = ATLAS_ROWS[rowIndex('idle')];
  const cycle = idle.durations.reduce((a, b) => a + b, 0);
  const pets = PET_IDS.map((id) => {
    const atlas = loadAtlas(id);
    const frames = idle.durations.map((_, c) => artFrame(atlas, rowIndex('idle'), c));
    return { name: PETS[id].displayName, frames, box: unionBox(frames) };
  });
  // Each pet gets its own width; all share one vertical range, so their feet line up.
  const all = unionBox(pets.flatMap((p) => p.frames));

  const S = 4;
  const PAD = 24;
  const GAP = 36;
  const TEXT = 4;
  pets.forEach((p) => {
    p.w = Math.max(p.box.w * S, textWidth(p.name, TEXT)) + GAP;
    p.box = { x: p.box.x, y: all.y, w: p.box.w, h: all.h };
  });
  const W = pets.reduce((sum, p) => sum + p.w, 0) + PAD * 2;
  const labelY = PAD + all.h * S + 20;
  const H = labelY + 8 * TEXT + PAD;

  const frameAt = (t) => {
    let acc = 0;
    for (let c = 0; c < idle.frames; c++) {
      acc += idle.durations[c];
      if (t < acc) return c;
    }
    return idle.frames - 1;
  };
  const STEP = 20;
  const out = [];
  for (let t = 0; t < cycle; t += STEP) {
    const img = newImage(W, H);
    let left = PAD;
    pets.forEach((p, i) => {
      const offset = PHASES[i % PHASES.length];
      const cx = left + p.w / 2;
      blit(img, p.frames[frameAt((t + offset) % cycle)], p.box, Math.round(cx - (p.box.w * S) / 2), PAD, S);
      drawText(img, p.name, Math.round(cx - textWidth(p.name, TEXT) / 2), labelY, LABEL, TEXT, true);
      left += p.w;
    });
    out.push({ img, delay: Math.min(STEP, cycle - t) });
  }
  return out;
}

/**
 * A drawing guide at the atlas's exact size: every cell outlined and named, the cells a row doesn't use
 * hatched, and faint centre and feet lines (where the built-in pets stand) in the rows that keep the pet
 * still. Meant to sit on a layer under your art, hidden before you export.
 */
export function template() {
  const img = newImage(COLS * CELL_W, ATLAS_ROWS.length * CELL_H, [255, 255, 255, 255]);
  const LINE = [201, 209, 217, 255];
  const GUIDE = [176, 208, 240, 255];
  const INK = [87, 96, 106, 255];
  const MUTED = [140, 149, 159, 255];
  const HATCH_BG = [240, 242, 245, 255];
  const HATCH = [222, 226, 231, 255];
  const FEET_Y = 188;
  ATLAS_ROWS.forEach((row, r) => {
    for (let c = 0; c < COLS; c++) {
      const x0 = c * CELL_W;
      const y0 = r * CELL_H;
      if (c < row.frames) {
        for (let y = 8; y < CELL_H - 4; y += 8) fillRect(img, x0 + CELL_W / 2, y0 + y, 1, 4, GUIDE);
        if (BASELINE_ROWS.includes(row.name)) for (let x = 4; x < CELL_W - 4; x += 8) fillRect(img, x0 + x, y0 + FEET_Y, 4, 1, GUIDE);
        drawText(img, `${r} ${row.name}`, x0 + 6, y0 + 6, INK, 2);
        drawText(img, `frame ${c + 1}/${row.frames}`, x0 + 6, y0 + 24, MUTED, 1);
        drawText(img, `${row.durations[c]}ms`, x0 + CELL_W - 6 - textWidth(`${row.durations[c]}ms`, 1), y0 + CELL_H - 12, MUTED, 1);
      } else {
        fillRect(img, x0, y0, CELL_W, CELL_H, HATCH_BG);
        for (let y = 0; y < CELL_H; y++) {
          for (let x = 0; x < CELL_W; x++) if ((x + y) % 16 === 0) fillRect(img, x0 + x, y0 + y, 1, 1, HATCH);
        }
        drawText(img, 'leave empty', x0 + CELL_W / 2 - textWidth('leave empty', 2) / 2, y0 + CELL_H / 2 - 8, MUTED, 2);
      }
      fillRect(img, x0, y0, CELL_W, 1, LINE);
      fillRect(img, x0, y0, 1, CELL_H, LINE);
    }
  });
  fillRect(img, 0, img.height - 1, img.width, 1, LINE);
  fillRect(img, img.width - 1, 0, 1, img.height, LINE);
  return img;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const pets = encodeTransparentGif(petsFrames(), path.join(ROOT, 'docs', 'screenshots', 'pets.gif'));
  console.log(`docs/screenshots/pets.gif: ${pets.frames} frames, ${pets.colours} colours, ${pets.bytes} bytes`);
  const file = path.join(ROOT, 'docs', 'pets', 'template.png');
  writePng(file, template());
  console.log(`docs/pets/template.png: ${fs.statSync(file).size} bytes`);
}
