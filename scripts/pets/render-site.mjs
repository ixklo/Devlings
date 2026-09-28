#!/usr/bin/env node
// Renders the landing page's own images from the pet atlases (no browser needed):
//
//   cd scripts/pets && npm install && node render-site.mjs
//
// Writes, into site/img/ (transparent, drawn at 4 px per art pixel and shown at half size, so they stay crisp):
//   hero-pets.png             the nine pets in a row, still, for the top of the page (the demo below it moves)
//   mood-<row>.gif / .png     Perch in the four moods the page explains: working, waiting, review, failed
//   pet-<id>.gif / .png       each built-in pet idling, for the gallery
// The .png files are the first frame, shown instead when the visitor asks for reduced motion.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { newImage, readPng, writePng } from './lib/engine.mjs';
import { ATLAS_ROWS, PET_IDS } from './lib/contract.mjs';
import { artFrame, blit, encodeTransparentGif, unionBox } from './render-docs.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const OUT = path.join(ROOT, 'site', 'img');
const S = 4;
const PAD = 8;

const loadAtlas = (id) => readPng(path.join(ROOT, 'src-tauri', 'pets', id, 'spritesheet.png'));
const rowOf = (name) => {
  const i = ATLAS_ROWS.findIndex((r) => r.name === name);
  return { index: i, ...ATLAS_ROWS[i] };
};
function frameAt(row, t) {
  let acc = 0;
  for (let c = 0; c < row.frames; c++) {
    acc += row.durations[c];
    if (t < acc) return c;
  }
  return row.frames - 1;
}
const framesOf = (atlas, row) => row.durations.map((_, c) => artFrame(atlas, row.index, c));

/** One looping clip of `frames` (a row of one pet) in a canvas sized to `box`, on the row's own timings. */
function clip(row, frames, box) {
  const W = box.w * S + PAD * 2;
  const H = box.h * S + PAD * 2;
  return frames.map((frame, c) => {
    const img = newImage(W, H);
    blit(img, frame, box, PAD, PAD, S);
    return { img, delay: row.durations[c] };
  });
}

function save(name, frames) {
  const gif = encodeTransparentGif(frames, path.join(OUT, `${name}.gif`));
  writePng(path.join(OUT, `${name}.png`), frames[0].img);
  const { width, height } = frames[0].img;
  console.log(`site/img/${name}.gif: ${width}x${height}, ${gif.frames} frames, ${gif.bytes} bytes (+ .png)`);
}

/** The nine pets in a row, feet on one line, each at a different point of its idle loop. */
function heroPets() {
  const idle = rowOf('idle');
  const pets = PET_IDS.map((id) => ({ frames: framesOf(loadAtlas(id), idle) }));
  const all = unionBox(pets.flatMap((p) => p.frames));
  const GAP = 6 * S;
  pets.forEach((p) => {
    const b = unionBox(p.frames);
    p.box = { x: b.x, y: all.y, w: b.w, h: all.h };
  });
  const W = pets.reduce((sum, p) => sum + p.box.w * S, 0) + GAP * (pets.length - 1) + PAD * 2;
  const H = all.h * S + PAD * 2;
  const phase = [0, 370, 740];
  const img = newImage(W, H);
  let left = PAD;
  pets.forEach((p, i) => {
    blit(img, p.frames[frameAt(idle, phase[i % phase.length])], p.box, left, PAD, S);
    left += p.box.w * S + GAP;
  });
  return img;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  fs.mkdirSync(OUT, { recursive: true });
  const hero = heroPets();
  writePng(path.join(OUT, 'hero-pets.png'), hero);
  console.log(`site/img/hero-pets.png: ${hero.width}x${hero.height}, ${fs.statSync(path.join(OUT, 'hero-pets.png')).size} bytes`);

  // The moods share one canvas size, so they line up in a row on the page.
  const perch = loadAtlas('perch');
  const moods = ['running', 'waiting', 'review', 'failed'].map((name) => ({ name, row: rowOf(name) }));
  moods.forEach((m) => (m.frames = framesOf(perch, m.row)));
  const moodBox = unionBox(moods.flatMap((m) => m.frames));
  for (const m of moods) save(`mood-${m.name}`, clip(m.row, m.frames, moodBox));

  // Gallery tiles share one canvas size too.
  const idle = rowOf('idle');
  const pets = PET_IDS.map((id) => ({ id, frames: framesOf(loadAtlas(id), idle) }));
  const petBox = unionBox(pets.flatMap((p) => p.frames));
  for (const p of pets) save(`pet-${p.id}`, clip(idle, p.frames, petBox));
}

