// Tiny pixel-art engine: string-map sprites, layered compositing with
// automatic per-material outlines, nearest-neighbour scaling, PNG output.
import fs from 'node:fs';
import path from 'node:path';
import { PNG } from 'pngjs';

// Codex pet atlas contract (openai/skills hatch-pet).
export const CELL_W = 192;
export const CELL_H = 208;
export const COLS = 8;
export const ROWS = 9;
export const ATLAS_W = CELL_W * COLS; // 1536
export const ATLAS_H = CELL_H * ROWS; // 1872

// Art is authored on a coarse grid and scaled up 4x with nearest neighbour.
export const SCALE = 4;
export const ART_W = CELL_W / SCALE; // 48
export const ART_H = CELL_H / SCALE; // 52

const TRANSPARENT = new Set(['.', ' ']);

/**
 * Parse a sprite from rows of characters. Each character is a colour role
 * (see palettes.mjs); '.' and ' ' are transparent. Leading/trailing blank
 * lines of a template literal are ignored, and common indentation is removed.
 */
export function sprite(src) {
  let lines = src.split('\n');
  while (lines.length && lines[0].trim() === '') lines.shift();
  while (lines.length && lines[lines.length - 1].trim() === '') lines.pop();
  const indent = Math.min(
    ...lines.filter((l) => l.trim()).map((l) => l.match(/^ */)[0].length),
  );
  lines = lines.map((l) => l.slice(indent).replace(/\s+$/, ''));
  const w = Math.max(...lines.map((l) => l.length));
  return { w, h: lines.length, rows: lines.map((l) => l.padEnd(w, '.')) };
}

export function at(s, x, y) {
  if (x < 0 || y < 0 || x >= s.w || y >= s.h) return null;
  const ch = s.rows[y][x];
  return TRANSPARENT.has(ch) ? null : ch;
}

/** Mirror a sprite horizontally. */
export function flipX(s) {
  return { w: s.w, h: s.h, rows: s.rows.map((r) => [...r].reverse().join('')) };
}

/** Remove the given rows (squash) from a sprite. */
export function dropRows(s, indices) {
  const drop = new Set(indices);
  const rows = s.rows.filter((_, i) => !drop.has(i));
  return { w: s.w, h: rows.length, rows };
}

/** Duplicate the given rows (stretch). */
export function dupRows(s, indices) {
  const dup = new Set(indices);
  const rows = [];
  s.rows.forEach((r, i) => {
    rows.push(r);
    if (dup.has(i)) rows.push(r);
  });
  return { w: s.w, h: rows.length, rows };
}

/** Duplicate the given columns (widen). */
export function dupCols(s, indices) {
  const dup = new Set(indices);
  const rows = s.rows.map((r) => {
    let out = '';
    [...r].forEach((ch, i) => {
      out += ch;
      if (dup.has(i)) out += ch;
    });
    return out;
  });
  return { w: rows[0].length, h: s.h, rows };
}

/** Replace colour roles inside a sprite, e.g. { h: 'b' }. */
export function recolor(s, map) {
  return {
    w: s.w,
    h: s.h,
    rows: s.rows.map((r) => [...r].map((ch) => map[ch] ?? ch).join('')),
  };
}

export class Layer {
  constructor(w = ART_W, h = ART_H) {
    this.w = w;
    this.h = h;
    this.px = new Array(w * h).fill(null);
  }
  get(x, y) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return null;
    return this.px[y * this.w + x];
  }
  set(x, y, v) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    this.px[y * this.w + x] = v;
  }
  /** Draw a sprite with its top-left at (x, y). `only` limits drawing to pixels already filled (clip). */
  blit(s, x, y, { clipTo = null } = {}) {
    for (let j = 0; j < s.h; j++) {
      for (let i = 0; i < s.w; i++) {
        const ch = at(s, i, j);
        if (!ch) continue;
        if (clipTo && !clipTo.get(x + i, y + j)) continue;
        this.set(x + i, y + j, ch);
      }
    }
    return this;
  }
  /** Draw another layer on top of this one. */
  over(layer) {
    for (let i = 0; i < this.px.length; i++) {
      if (layer.px[i] != null) this.px[i] = layer.px[i];
    }
    return this;
  }
  clone() {
    const l = new Layer(this.w, this.h);
    l.px = this.px.slice();
    return l;
  }
}

/**
 * Add a 1-px outline (4-neighbour) around every filled pixel of `layer`.
 * The outline colour is chosen per material: `outlineOf[role]` for the
 * neighbouring fill pixel. Roles listed in `lineRoles` are already line
 * pixels and do not spawn further outline.
 */
export function outline(layer, outlineOf, lineRoles) {
  const out = layer.clone();
  const spawn = (x, y) => {
    const v = layer.get(x, y);
    return v != null && !lineRoles.has(v) ? v : null;
  };
  for (let y = 0; y < layer.h; y++) {
    for (let x = 0; x < layer.w; x++) {
      if (layer.get(x, y) != null) continue;
      // Prefer the pixel below/above so vertical edges pick the body colour.
      const n = spawn(x, y + 1) ?? spawn(x, y - 1) ?? spawn(x - 1, y) ?? spawn(x + 1, y);
      if (n) out.set(x, y, outlineOf[n] ?? outlineOf.default);
    }
  }
  return out;
}

/**
 * Compose a frame from groups drawn back to front. Each group is
 * { parts: [{ s, x, y, clip? }], outline: true|false }. Each group is
 * rendered on its own layer and outlined separately, so a front group's
 * outline separates it from what is behind it.
 */
export function compose(groups, outlineOf, lineRoles) {
  const frame = new Layer();
  for (const g of groups) {
    if (!g) continue;
    const layer = new Layer();
    for (const p of g.parts) {
      if (!p) continue;
      layer.blit(p.s, p.x, p.y, { clipTo: p.clip ? layer.clone() : null });
    }
    frame.over(g.outline === false ? layer : outline(layer, outlineOf, lineRoles));
  }
  return frame;
}

export function hexToRgba(hex) {
  const h = hex.replace('#', '');
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
    h.length >= 8 ? parseInt(h.slice(6, 8), 16) : 255,
  ];
}

export function newImage(w, h, fill = [0, 0, 0, 0]) {
  const png = new PNG({ width: w, height: h });
  for (let i = 0; i < w * h; i++) png.data.set(fill, i * 4);
  return png;
}

/** Paint an art layer into an image at (ox, oy) with integer scale. */
export function paintLayer(img, layer, palette, ox, oy, scale = SCALE) {
  for (let y = 0; y < layer.h; y++) {
    for (let x = 0; x < layer.w; x++) {
      const role = layer.get(x, y);
      if (role == null) continue;
      const hex = palette[role];
      if (!hex) throw new Error(`No colour for role '${role}'`);
      const rgba = hexToRgba(hex);
      for (let j = 0; j < scale; j++) {
        for (let i = 0; i < scale; i++) {
          const px = ox + x * scale + i;
          const py = oy + y * scale + j;
          if (px < 0 || py < 0 || px >= img.width || py >= img.height) continue;
          img.data.set(rgba, (py * img.width + px) * 4);
        }
      }
    }
  }
}

export function fillRect(img, x, y, w, h, rgba) {
  for (let j = y; j < y + h; j++) {
    for (let i = x; i < x + w; i++) {
      if (i < 0 || j < 0 || i >= img.width || j >= img.height) continue;
      img.data.set(rgba, (j * img.width + i) * 4);
    }
  }
}

/** Alpha-composite src (straight alpha) onto dst at (ox, oy), sampling src with integer down/up scale. */
export function drawImage(dst, src, ox, oy, { scale = 1, sx = 0, sy = 0, sw = src.width, sh = src.height } = {}) {
  const dw = Math.round(sw * scale);
  const dh = Math.round(sh * scale);
  for (let j = 0; j < dh; j++) {
    for (let i = 0; i < dw; i++) {
      const srcX = sx + Math.floor(i / scale);
      const srcY = sy + Math.floor(j / scale);
      const si = (srcY * src.width + srcX) * 4;
      const a = src.data[si + 3] / 255;
      if (a === 0) continue;
      const x = ox + i;
      const y = oy + j;
      if (x < 0 || y < 0 || x >= dst.width || y >= dst.height) continue;
      const di = (y * dst.width + x) * 4;
      for (let k = 0; k < 3; k++) {
        dst.data[di + k] = Math.round(src.data[si + k] * a + dst.data[di + k] * (1 - a));
      }
      dst.data[di + 3] = Math.max(dst.data[di + 3], src.data[si + 3]);
    }
  }
}

export function writePng(file, img) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, PNG.sync.write(img, { colorType: 6 }));
}

export function readPng(file) {
  return PNG.sync.read(fs.readFileSync(file));
}
