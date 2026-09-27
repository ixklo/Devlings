#!/usr/bin/env node
// Renders Perch's README/website images straight from the bundled sprite
// atlases, with nearest-neighbour scaling only:
//
//   cd scripts/pets && npm install && node render-demo.mjs
//
// Writes:
//   docs/screenshots/demo.gif   the teal pet going through its moods
//   docs/screenshots/pets.gif   the three built-in pets idling side by side
//   docs/social-preview.png     1280x640 repository social preview
//   site/favicon.png            the website's icon
//
// Frame timings come from the atlas contract (lib/contract.mjs), so the
// animation plays at the same speed as in the app.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import gifenc from 'gifenc';
import { CELL_W, CELL_H, SCALE, ART_W, ART_H, newImage, fillRect, readPng, writePng } from './lib/engine.mjs';
import { ATLAS_ROWS } from './lib/contract.mjs';
import { drawText, textWidth } from './lib/font.mjs';

const { GIFEncoder } = gifenc;
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

const BG = [238, 241, 244, 255];
const INK = [43, 49, 64, 255];
const MUTED = [107, 116, 131, 255];
const DOT_OFF = [201, 208, 217, 255];
const TEAL = [58, 163, 154, 255];

// The demo's story, in the order the pet lives it.
const MOODS = [
  { row: 'idle', label: 'Idle', loops: 2 },
  { row: 'running', label: 'Working', loops: 3 },
  { row: 'waiting', label: 'Needs input', loops: 3 },
  { row: 'review', label: 'Reviewing', loops: 3 },
  { row: 'failed', label: 'Failed', loops: 2 },
  { row: 'waving', label: 'Waving', loops: 4 },
];

const rowIndex = (name) => ATLAS_ROWS.findIndex((r) => r.name === name);

/** The atlas cell at (row, col), sampled back down to its 1x art grid. */
function artFrame(atlas, row, col) {
  const img = newImage(ART_W, ART_H);
  for (let y = 0; y < ART_H; y++) {
    for (let x = 0; x < ART_W; x++) {
      const si = ((row * CELL_H + y * SCALE) * atlas.width + col * CELL_W + x * SCALE) * 4;
      img.data.set(atlas.data.subarray(si, si + 4), (y * ART_W + x) * 4);
    }
  }
  return img;
}

/** Smallest box holding every opaque pixel of every frame, so the pet never jumps. */
function unionBox(frames) {
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

/** Paint a box of an art frame onto dst at integer scale (pixels are opaque or fully transparent). */
function blit(dst, art, box, ox, oy, scale) {
  for (let y = 0; y < box.h; y++) {
    for (let x = 0; x < box.w; x++) {
      const si = ((box.y + y) * ART_W + box.x + x) * 4;
      if (art.data[si + 3] < 128) continue;
      fillRect(dst, ox + x * scale, oy + y * scale, scale, scale, [...art.data.subarray(si, si + 3), 255]);
    }
  }
}

function centerText(img, text, cx, y, rgba, scale) {
  drawText(img, text, Math.round(cx - textWidth(text, scale) / 2), y, rgba, scale);
}

/** Encode opaque RGBA frames with one exact shared palette (pixel art has few colours). */
function encodeGif(frames, file) {
  const colours = new Map();
  const key = (d, i) => (d[i] << 16) | (d[i + 1] << 8) | d[i + 2];
  for (const { img } of frames) {
    for (let i = 0; i < img.data.length; i += 4) {
      const k = key(img.data, i);
      if (!colours.has(k)) colours.set(k, colours.size);
    }
  }
  if (colours.size > 256) throw new Error(`${file}: ${colours.size} colours, GIF allows 256`);
  const palette = [...colours.keys()].map((k) => [(k >> 16) & 255, (k >> 8) & 255, k & 255]);
  while (palette.length < 2) palette.push([0, 0, 0]);

  // Merge runs of identical frames into one longer frame.
  const merged = [];
  for (const f of frames) {
    const prev = merged[merged.length - 1];
    if (prev && Buffer.compare(prev.img.data, f.img.data) === 0) prev.delay += f.delay;
    else merged.push({ img: f.img, delay: f.delay });
  }

  const gif = GIFEncoder();
  merged.forEach(({ img, delay }, n) => {
    const index = new Uint8Array(img.width * img.height);
    for (let p = 0; p < index.length; p++) index[p] = colours.get(key(img.data, p * 4));
    gif.writeFrame(index, img.width, img.height, n === 0 ? { palette, delay, repeat: 0 } : { delay });
  });
  gif.finish();
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, gif.bytes());
  return { frames: merged.length, colours: colours.size, bytes: fs.statSync(file).size };
}

function loadAtlas(id) {
  return readPng(path.join(ROOT, 'src-tauri', 'pets', id, 'spritesheet.png'));
}

export function demoFrames() {
  const atlas = loadAtlas('perch');
  const clips = MOODS.map((m) => {
    const r = rowIndex(m.row);
    const spec = ATLAS_ROWS[r];
    return { ...m, frames: Array.from({ length: spec.frames }, (_, c) => artFrame(atlas, r, c)), durations: spec.durations };
  });
  const box = unionBox(clips.flatMap((c) => c.frames));

  const S = 5;
  const PAD = 24;
  const LABEL = 3;
  const labelW = Math.max(...MOODS.map((m) => textWidth(m.label, LABEL)));
  const W = Math.max(box.w * S, labelW) + PAD * 2;
  const petY = PAD;
  const labelY = petY + box.h * S + 18;
  const dotsY = labelY + 8 * LABEL + 14;
  const H = dotsY + 8 + PAD;
  const petX = Math.round((W - box.w * S) / 2);

  const out = [];
  clips.forEach((clip, m) => {
    for (let loop = 0; loop < clip.loops; loop++) {
      clip.frames.forEach((art, c) => {
        const img = newImage(W, H, BG);
        blit(img, art, box, petX, petY, S);
        centerText(img, clip.label, W / 2, labelY, INK, LABEL);
        const dotsX = Math.round(W / 2 - (MOODS.length * 16 - 8) / 2);
        MOODS.forEach((_, d) => fillRect(img, dotsX + d * 16, dotsY, 8, 8, d === m ? TEAL : DOT_OFF));
        out.push({ img, delay: clip.durations[c] });
      });
    }
  });
  return out;
}

function renderDemo() {
  return encodeGif(demoFrames(), path.join(ROOT, 'docs', 'screenshots', 'demo.gif'));
}

function renderPets() {
  const pets = [
    { id: 'perch', name: 'Perch' },
    { id: 'ember', name: 'Ember' },
    { id: 'plum', name: 'Plum' },
  ];
  const idle = ATLAS_ROWS[rowIndex('idle')];
  const cycle = idle.durations.reduce((a, b) => a + b, 0);
  for (const p of pets) {
    const atlas = loadAtlas(p.id);
    p.frames = Array.from({ length: idle.frames }, (_, c) => artFrame(atlas, rowIndex('idle'), c));
  }
  const box = unionBox(pets.flatMap((p) => p.frames));

  const S = 4;
  const PAD = 20;
  const LABEL = 3;
  const colW = box.w * S + 32;
  const W = colW * pets.length + PAD * 2;
  const labelY = PAD + box.h * S + 16;
  const H = labelY + 8 * LABEL + PAD;

  // Each pet starts at a different point of the idle loop, so they don't bob in lockstep.
  const offsets = pets.map((_, i) => Math.round((cycle * i) / pets.length));
  const frameAt = (t) => {
    let acc = 0;
    for (let c = 0; c < idle.frames; c++) {
      acc += idle.durations[c];
      if (t < acc) return c;
    }
    return idle.frames - 1;
  };
  const cuts = new Set([0]);
  for (const off of offsets) {
    let acc = 0;
    for (const d of idle.durations) {
      acc += d;
      cuts.add((acc - off + cycle) % cycle);
    }
  }
  const times = [...cuts].sort((a, b) => a - b);

  const out = times.map((t, n) => {
    const img = newImage(W, H, BG);
    pets.forEach((p, i) => {
      const x = PAD + i * colW + Math.round((colW - box.w * S) / 2);
      blit(img, p.frames[frameAt((t + offsets[i]) % cycle)], box, x, PAD, S);
      centerText(img, p.name, PAD + i * colW + colW / 2, labelY, INK, LABEL);
    });
    const next = n + 1 < times.length ? times[n + 1] : cycle;
    return { img, delay: next - t };
  });
  return encodeGif(out, path.join(ROOT, 'docs', 'screenshots', 'pets.gif'));
}

function renderSocial() {
  const W = 1280;
  const H = 640;
  const img = newImage(W, H, BG);
  const atlas = loadAtlas('perch');
  const art = artFrame(atlas, rowIndex('idle'), 0);
  const box = unionBox([art]);

  const S = Math.floor(Math.min(440 / box.w, 480 / box.h));
  const petX = 96;
  blit(img, art, box, petX, Math.round((H - box.h * S) / 2), S);

  const textX = petX + box.w * S + 72;
  const TITLE = 20;
  const TAG = 7;
  const NOTE = 4;
  const block = 7 * TITLE + 48 + 8 * TAG + 16 + 8 * TAG + 40 + 7 * NOTE;
  let y = Math.round((H - block) / 2);
  drawText(img, 'Perch', textX, y, INK, TITLE);
  y += 7 * TITLE + 48;
  drawText(img, 'A desktop pet', textX, y, INK, TAG);
  y += 8 * TAG + 16;
  drawText(img, 'for Claude Code', textX, y, INK, TAG);
  y += 8 * TAG + 40;
  drawText(img, 'Free and open source', textX, y, MUTED, NOTE);
  if (textX + textWidth('for Claude Code', TAG) > W - 40) throw new Error('social preview text overflows');

  const file = path.join(ROOT, 'docs', 'social-preview.png');
  writePng(file, img);
  return { bytes: fs.statSync(file).size };
}

function renderFavicon() {
  const art = artFrame(loadAtlas('perch'), rowIndex('idle'), 0);
  const box = unionBox([art]);
  const S = 2;
  const size = 96;
  const img = newImage(size, size);
  blit(img, art, box, Math.round((size - box.w * S) / 2), Math.round((size - box.h * S) / 2), S);
  const file = path.join(ROOT, 'site', 'favicon.png');
  writePng(file, img);
  return { bytes: fs.statSync(file).size };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  for (const [name, fn] of [['demo.gif', renderDemo], ['pets.gif', renderPets], ['social-preview.png', renderSocial], ['favicon.png', renderFavicon]]) {
    const r = fn();
    console.log(`${name}: ${Object.entries(r).map(([k, v]) => `${v} ${k}`).join(', ')}`);
  }
}
