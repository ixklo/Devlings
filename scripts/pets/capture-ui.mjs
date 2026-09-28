#!/usr/bin/env node
// Captures Devlings' real UI for the README and the website: the app's own pages, rendered by a headless
// browser against the browser preview's fake backend (src/shared/mock.ts) in fixed, made-up scenes
// (?scene=, src/shared/mockScenes.ts). Nothing here starts the app or talks to Claude Code.
//
//   npm install                                  (at the repository root, once: Vite and the app)
//   cd scripts/pets && npm install && node capture-ui.mjs
//
// Needs Microsoft Edge or Google Chrome; set BROWSER to its executable if it isn't found. It starts its own
// Vite dev server, or uses a running one with --url http://localhost:1420. The pages run in real time (the
// demo in slow motion, see recordDemo), so a re-run can catch the pet on a different frame.
//
// Writes, each in a light and a dark ("-dark") version, on a plain neutral backdrop:
//   docs/screenshots/demo.gif       the hero: two sessions start, one finishes, the cards tuck away behind a count
//   docs/screenshots/cards.png      the pet with thread cards
//   docs/screenshots/approval.png   a permission card with Deny and Allow
//   docs/screenshots/chat.png       the mini chat with a finished answer
//   docs/screenshots/composer.png   the composer, with a question typed in
//   docs/screenshots/settings.png   Settings, with the pet picker
// and docs/social-preview.png (1280x640: the name, the tagline and the nine pets).
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import gifenc from 'gifenc';
import { PNG } from 'pngjs';
import { PET_IDS } from './lib/contract.mjs';

const { GIFEncoder, quantize } = gifenc;
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const SHOTS_DIR = path.join(ROOT, 'docs', 'screenshots');

// The pet and settings windows' sizes (src-tauri/tauri.conf.json), drawn at 2x for high-density screens.
const PET_WINDOW = { width: 380, height: 600 };
const SETTINGS_WINDOW = { width: 480, height: 680 };
const DPR = 2;
// The pet window is transparent; in the images it sits on a plain neutral backdrop instead of a desktop.
const THEMES = {
  light: { suffix: '', backdrop: '#eaedf1' },
  dark: { suffix: '-dark', backdrop: '#30343a' },
};
const CORNER = 12; // CSS px

// `ready`: what to wait for first; `wait`: then how long to let things settle (entrances, the permission
// card's buttons arming).
const STILLS = [
  { name: 'cards', query: 'window=pet&scene=cards', ready: `document.querySelectorAll('.thread-card').length === 3`, wait: 1500 },
  { name: 'approval', query: 'window=pet&scene=approval', ready: `document.querySelector('.approval-card')?.dataset.armed === 'true'`, wait: 800 },
  {
    name: 'chat',
    query: 'window=pet&scene=chat&open=thread',
    ready: `document.querySelector('.thread-view')?.innerText.includes('All 25 tests pass') ?? false`,
    wait: 1500,
  },
  {
    name: 'composer',
    query: 'window=pet&scene=cards&open=compose',
    ready: `!!document.querySelector('.composer-card')`,
    wait: 800,
    type: 'Show the chance of rain on each day of the forecast',
  },
  {
    name: 'settings',
    query: 'window=settings&scene=cards',
    ready: `document.querySelectorAll('.pet-option .sprite').length === 9`,
    wait: 1500,
    window: SETTINGS_WINDOW,
  },
];

// The demo scene's script (mockScenes.ts): the first card at 1.2 s, the cards collapse at 8.2 s. The GIF
// starts with a second of the idle pet and holds the end for a moment before it loops. It's recorded at a
// quarter of real speed and played back at 25 frames a second.
const DEMO_FIRST_CARD_MS = 1200;
const DEMO_IDLE_MS = 1000;
const DEMO_END_MS = 10_800;
const DEMO_SPEED = 0.25;
const DEMO_FRAME_MS = 40;
// The GIF's first frame (what shows where animations don't play) is this far into the script: two cards,
// one done, and the pet inspecting it. The loop runs on from there and comes round again.
const DEMO_POSTER_MS = 6100;
const GIF_BUDGET = 2 * 1024 * 1024;

// ---- Browser -------------------------------------------------------------------------------------------

function findBrowser() {
  const candidates = [
    process.env.BROWSER,
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/usr/bin/microsoft-edge',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
  ].filter(Boolean);
  const found = candidates.find((p) => fs.existsSync(p));
  if (!found) throw new Error('No Edge or Chrome found. Set BROWSER to the browser executable.');
  return found;
}

/** A minimal Chrome DevTools Protocol client over the browser's WebSocket. */
class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.handlers = new Set();
    ws.addEventListener('message', (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject, method } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) reject(new Error(`${method}: ${msg.error.message}`));
        else resolve(msg.result);
      } else if (msg.method) {
        for (const h of this.handlers) h(msg);
      }
    });
  }

  send(method, params = {}, sessionId) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, method });
      this.ws.send(JSON.stringify({ id, method, params, sessionId }));
    });
  }

  /** Resolves with the next event `method` (from `sessionId`) that passes `pred`. */
  next(method, sessionId, pred = () => true) {
    return new Promise((resolve) => {
      const h = (msg) => {
        if (msg.method === method && msg.sessionId === sessionId && pred(msg.params)) {
          this.handlers.delete(h);
          resolve(msg.params);
        }
      };
      this.handlers.add(h);
    });
  }
}

async function launchBrowser() {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'devlings-capture-'));
  const args = [
    '--headless=new',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-extensions',
    '--hide-scrollbars',
    '--mute-audio',
    '--force-color-profile=srgb',
    'about:blank',
  ];
  const proc = spawn(findBrowser(), args, { stdio: ['ignore', 'ignore', 'pipe'] });
  const wsUrl = await new Promise((resolve, reject) => {
    let log = '';
    const timer = setTimeout(() => reject(new Error(`The browser didn't start:\n${log}`)), 20_000);
    proc.stderr.on('data', (d) => {
      log += d;
      const m = log.match(/DevTools listening on (ws:\/\/\S+)/);
      if (m) {
        clearTimeout(timer);
        resolve(m[1]);
      }
    });
  });
  const ws = new WebSocket(wsUrl);
  await new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true });
    ws.addEventListener('error', reject, { once: true });
  });
  const cdp = new Cdp(ws);
  return {
    cdp,
    async close() {
      const exited = new Promise((resolve) => {
        if (proc.exitCode !== null) resolve();
        proc.once('exit', resolve);
        setTimeout(resolve, 10_000);
      });
      await cdp.send('Browser.close').catch(() => {});
      ws.close();
      await exited;
      proc.kill();
      try {
        fs.rmSync(profile, { recursive: true, force: true, maxRetries: 20, retryDelay: 200 });
      } catch {
        // The browser can hold on to its profile for a moment after it exits; it's only a temp folder.
      }
    },
  };
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** The page's clocks and timers, running at `speed` times real time. */
function slowClock(speed) {
  return `(() => {
    const speed = ${speed};
    const realNow = performance.now.bind(performance);
    const t0 = realNow();
    const d0 = Date.now();
    const now = () => t0 + (realNow() - t0) * speed;
    performance.now = now;
    Date.now = () => Math.floor(d0 + (realNow() - t0) * speed);
    const [st, si, raf] = [window.setTimeout, window.setInterval, window.requestAnimationFrame];
    window.setTimeout = (fn, ms = 0, ...args) => st(fn, ms / speed, ...args);
    window.setInterval = (fn, ms = 0, ...args) => si(fn, ms / speed, ...args);
    window.requestAnimationFrame = (cb) => raf(() => cb(now()));
  })();`;
}

/** A tab with the given viewport and colour scheme, optionally a backdrop colour and slow motion. */
async function openTab(cdp, { width, height, theme, scale = DPR, speed = 1, backdrop = null }) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const s = (method, params) => cdp.send(method, params, sessionId);
  await s('Page.enable');
  await s('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: scale, mobile: false });
  await s('Emulation.setEmulatedMedia', {
    features: [
      { name: 'prefers-color-scheme', value: theme },
      { name: 'prefers-reduced-motion', value: 'no-preference' },
    ],
  });
  if (speed !== 1) {
    // Slow motion: CSS and web animations through the protocol, the page's clocks and timers by wrapping them
    // before any of its scripts run.
    await s('Animation.enable');
    await s('Animation.setPlaybackRate', { playbackRate: speed });
    await s('Page.addScriptToEvaluateOnNewDocument', { source: slowClock(speed) });
  }

  const tab = {
    s,
    async eval(expression) {
      const r = await s('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (r.exceptionDetails) throw new Error(`${expression}: ${r.exceptionDetails.text}`);
      return r.result.value;
    },
    async goto(url) {
      const loaded = cdp.next('Page.loadEventFired', sessionId);
      await s('Page.navigate', { url });
      await loaded;
      if (backdrop) await tab.eval(`document.documentElement.style.setProperty('background', '${backdrop}', 'important')`);
    },
    /** Polls until `expression` is true. */
    async waitFor(expression, what, limitMs = 15_000) {
      for (const end = Date.now() + limitMs; Date.now() < end; await sleep(50)) {
        if (await tab.eval(expression).catch(() => false)) return;
      }
      throw new Error(`Timed out waiting for ${what}`);
    },
    async screenshot() {
      const { data } = await s('Page.captureScreenshot', { format: 'png', optimizeForSpeed: true });
      return Buffer.from(data, 'base64');
    },
    close: () => cdp.send('Target.closeTarget', { targetId }),
  };
  return tab;
}

// ---- Pages -------------------------------------------------------------------------------------------

// The pet has its sheet (a data URL or the dev server's file) and every sprite on the page has decoded.
const SPRITES_READY = `(() => {
  const sprites = [...document.querySelectorAll('.sprite')];
  return sprites.length > 0 && sprites.every((el) => el.style.backgroundImage.includes('url('));
})()`;

// Everything the pet window shows: cards (threads, permission requests, composer, mini chat), the pet, its bar.
const PET_CONTENT = `(() => {
  const els = [...document.querySelectorAll('.card, .more-pill, .pet, .control-bar.is-visible')];
  const r = els.map((el) => el.getBoundingClientRect()).filter((b) => b.width && b.height);
  if (!r.length) return null;
  return { top: Math.min(...r.map((b) => b.top)), bottom: Math.max(...r.map((b) => b.bottom)) };
})()`;

// Settings' header and its first section, the pet picker.
const SETTINGS_CONTENT = `(() => {
  const head = document.querySelector('.page-head').getBoundingClientRect();
  const pet = document.querySelector('.settings-page section').getBoundingClientRect();
  return { top: head.top, bottom: pet.bottom };
})()`;

/** A crop of the page (CSS px): full width, from `top` to `bottom` plus room for card shadows. */
function cropFor(content, viewport, { above = 20, below = 18 } = {}) {
  const top = Math.max(0, Math.floor(content.top - above));
  const bottom = Math.min(viewport.height, Math.ceil(content.bottom + below));
  return { x: 0, y: top, width: viewport.width, height: bottom - top };
}

/** RGBA pixels of `png` inside `crop` (CSS px), at the device scale. */
function cropPixels(png, crop, scale = DPR) {
  const x0 = crop.x * scale;
  const y0 = crop.y * scale;
  const w = crop.width * scale;
  const h = crop.height * scale;
  const out = new PNG({ width: w, height: h });
  for (let y = 0; y < h; y++) png.data.copy(out.data, y * w * 4, ((y0 + y) * png.width + x0) * 4, ((y0 + y) * png.width + x0 + w) * 4);
  return out;
}

/** How much of each pixel lies inside a rectangle with rounded corners of radius `r` (1 inside, 0 outside). */
function coverage(x, y, w, h, r) {
  const cx = x < r ? r : x >= w - r ? w - r : null;
  const cy = y < r ? r : y >= h - r ? h - r : null;
  if (cx === null || cy === null) return 1;
  const d = Math.hypot(x + 0.5 - cx, y + 0.5 - cy);
  return Math.max(0, Math.min(1, r - d + 0.5));
}

/** Rounds the corners (anti-aliased, through alpha) and writes a PNG with no metadata. */
function writeRoundedPng(file, img, radius) {
  for (let y = 0; y < img.height; y++) {
    for (let x = 0; x < img.width; x++) {
      const c = coverage(x, y, img.width, img.height, radius);
      if (c < 1) img.data[(y * img.width + x) * 4 + 3] = Math.round(255 * c);
    }
  }
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, PNG.sync.write(img, { colorType: 6 }));
  return fs.statSync(file).size;
}

async function captureStill(cdp, base, shot, theme) {
  const viewport = shot.window ?? PET_WINDOW;
  const tab = await openTab(cdp, { ...viewport, theme, backdrop: shot.window ? null : THEMES[theme].backdrop });
  try {
    await tab.goto(`${base}/?${shot.query}`);
    await tab.waitFor(SPRITES_READY, `${shot.name}: the pet's sprite`);
    if (shot.ready) await tab.waitFor(shot.ready, `${shot.name}: ${shot.ready}`);
    await sleep(shot.wait);
    if (shot.type) {
      await tab.waitFor(`!!document.querySelector('.composer-text')`, `${shot.name}: the composer's text box`);
      await tab.eval(`document.querySelector('.composer-text').focus()`);
      await tab.s('Input.insertText', { text: shot.type });
      await sleep(400);
    }
    await tab.waitFor(SPRITES_READY, `${shot.name}: every sprite`);
    const content = await tab.eval(shot.window === SETTINGS_WINDOW ? SETTINGS_CONTENT : PET_CONTENT);
    const crop = cropFor(content, viewport, shot.window === SETTINGS_WINDOW ? { above: 16, below: 16 } : undefined);
    const png = PNG.sync.read(await tab.screenshot());
    const file = path.join(SHOTS_DIR, `${shot.name}${THEMES[theme].suffix}.png`);
    const bytes = writeRoundedPng(file, cropPixels(png, crop), CORNER * DPR);
    return { file, bytes, size: `${crop.width * DPR}x${crop.height * DPR}` };
  } finally {
    await tab.close();
  }
}

// ---- The demo GIF --------------------------------------------------------------------------------------

async function recordDemo(cdp, base, theme) {
  const tab = await openTab(cdp, { ...PET_WINDOW, theme, speed: DEMO_SPEED, backdrop: THEMES[theme].backdrop });
  const shots = [];
  let firstCard = null;
  try {
    await tab.goto(`${base}/?window=pet&scene=demo`);
    await tab.waitFor(SPRITES_READY, "the demo's sprite");
    // Screenshots as fast as the browser gives them, each stamped with the page's (slowed) time. How long
    // the page took to load varies, so the GIF is cut around the first card: a second of the idle pet
    // before it, then the rest of the scene's script.
    for (let start = null; ; ) {
      const { t, content, cards } = await tab.eval(
        `({ t: performance.now(), content: ${PET_CONTENT}, cards: document.querySelectorAll('.card').length })`,
      );
      start ??= t;
      if (firstCard !== null && t >= firstCard + DEMO_END_MS - DEMO_FIRST_CARD_MS) break;
      if (t - start > DEMO_END_MS * 2) throw new Error("The demo's first card never appeared");
      if (cards > 0 && firstCard === null) firstCard = t;
      shots.push({ t, png: await tab.screenshot(), content });
    }
  } finally {
    await tab.close();
  }
  const from = firstCard - DEMO_IDLE_MS;
  const to = firstCard + DEMO_END_MS - DEMO_FIRST_CARD_MS;
  const kept = shots.filter((f) => f.t >= from - DEMO_FRAME_MS);
  const boxes = kept.map((f) => f.content).filter(Boolean);
  const content = { top: Math.min(...boxes.map((b) => b.top)), bottom: Math.max(...boxes.map((b) => b.bottom)) };
  // Resampled to a steady frame rate: each frame is the latest screenshot taken by then.
  const frames = [];
  for (let tick = from, i = 0; tick < to; tick += DEMO_FRAME_MS) {
    while (i + 1 < kept.length && kept[i + 1].t <= tick) i++;
    const last = frames[frames.length - 1];
    if (last?.png === kept[i].png) last.delay += DEMO_FRAME_MS;
    else frames.push({ png: kept[i].png, delay: DEMO_FRAME_MS, at: tick - from });
  }
  const poster = frames.findIndex((f) => f.at >= DEMO_IDLE_MS + DEMO_POSTER_MS - DEMO_FIRST_CARD_MS);
  frames.push(...frames.splice(0, poster));
  return { frames, crop: cropFor(content, PET_WINDOW, { above: 24, below: 18 }) };
}

/**
 * One shared 255-colour palette for the whole GIF; each frame after the first only carries the pixels that
 * changed (the rest are transparent, and dispose 1 keeps the previous frame), and frames that change
 * nothing merge into the one before. The corners outside a rounded rectangle stay transparent throughout.
 */
function encodeDemo(timed, crop, file) {
  const frames = timed.map(({ png, delay }) => Object.assign(cropPixels(PNG.sync.read(png), crop), { delay }));
  const { width, height } = frames[0];
  const pick = Array.from({ length: 24 }, (_, i) => frames[Math.floor((i * (frames.length - 1)) / 23)]);
  const sample = new Uint8Array(pick.length * width * height * 4);
  pick.forEach((f, i) => sample.set(f.data, i * width * height * 4));
  // The most common exact colours (backdrop, card surfaces, text, the pet's own palette) keep their exact
  // value; the quantizer picks the rest.
  const counts = new Map();
  for (let i = 0; i < sample.length; i += 4) {
    const k = (sample[i] << 16) | (sample[i + 1] << 8) | sample[i + 2];
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }
  const exact = [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 64).map(([k]) => [(k >> 16) & 255, (k >> 8) & 255, k & 255]);
  const palette = [...exact, ...quantize(sample, 255 - exact.length, { format: 'rgb565' })];
  const T = palette.length;
  // Nearest palette colour at full precision (gifenc's applyPalette rounds to 16 bits first, which turns
  // white cards grey), cached per colour.
  const nearest = new Map();
  const toIndex = (data) => {
    const out = new Uint8Array(data.length / 4);
    for (let p = 0, i = 0; i < data.length; p++, i += 4) {
      const k = (data[i] << 16) | (data[i + 1] << 8) | data[i + 2];
      let best = nearest.get(k);
      if (best === undefined) {
        let d = Infinity;
        palette.forEach(([r, g, b], n) => {
          const e = (r - data[i]) ** 2 + (g - data[i + 1]) ** 2 + (b - data[i + 2]) ** 2;
          if (e < d) (d = e), (best = n);
        });
        nearest.set(k, best);
      }
      out[p] = best;
    }
    return out;
  };
  const gifPalette = [...palette, [255, 0, 255]];

  const outside = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) outside[y * width + x] = coverage(x, y, width, height, CORNER * DPR) < 0.5 ? 1 : 0;

  const gif = GIFEncoder();
  let shown = null;
  let pending = null;
  const flush = () => {
    if (!pending) return;
    gif.writeFrame(pending.index, width, height, {
      ...(pending.first ? { palette: gifPalette, repeat: 0 } : {}),
      // GIF delays are in 10 ms steps, and browsers slow anything under 20 ms down to 100 ms.
      delay: Math.max(20, Math.round(pending.delay / 10) * 10),
      transparent: true,
      transparentIndex: T,
      dispose: 1,
    });
    pending = null;
  };
  let written = 0;
  for (const f of frames) {
    const index = toIndex(f.data);
    for (let p = 0; p < index.length; p++) if (outside[p]) index[p] = T;
    if (!shown) {
      shown = index;
      pending = { index: index.slice(), delay: f.delay, first: true };
      continue;
    }
    let changed = false;
    const diff = new Uint8Array(index.length);
    for (let p = 0; p < index.length; p++) {
      if (index[p] === shown[p]) diff[p] = T;
      else {
        diff[p] = index[p];
        shown[p] = index[p];
        changed = true;
      }
    }
    if (!changed) {
      pending.delay += f.delay;
      continue;
    }
    flush();
    written++;
    pending = { index: diff, delay: f.delay, first: false };
  }
  flush();
  gif.finish();
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, gif.bytes());
  return { frames: written + 1, size: `${width}x${height}`, bytes: fs.statSync(file).size };
}

// ---- Social preview ------------------------------------------------------------------------------------

// Set in the system's UI font: the committed image was made on Windows (Segoe UI), so re-make it there to keep
// the same look.
function socialHtml() {
  const S = 0.75; // 3 screen px per art pixel
  const sheet = (id) => `data:image/png;base64,${fs.readFileSync(path.join(ROOT, 'src-tauri', 'pets', id, 'spritesheet.png')).toString('base64')}`;
  const pets = PET_IDS.map(
    (id) => `<div class="pet" style="background-image:url(${sheet(id)})"></div>`,
  ).join('');
  return `<!doctype html><html><head><meta charset="utf-8"><style>
  html, body { margin: 0; width: 1280px; height: 640px; overflow: hidden; }
  body {
    background: #f4f5f7;
    color: #0d0d0d;
    font-family: "Segoe UI Variable Display", "Segoe UI", system-ui, -apple-system, sans-serif;
    display: flex; flex-direction: column; align-items: center;
  }
  h1 { margin: 92px 0 0; font-size: 128px; line-height: 1; font-weight: 700; letter-spacing: -3px; }
  .tag { margin: 22px 0 0; font-size: 48px; line-height: 1.1; font-weight: 600; color: #3d3d3d; }
  .note { margin: 18px 0 0; font-size: 26px; color: #6b6b6b; }
  .pets { margin-top: 40px; display: flex; }
  .pet {
    width: ${192 * S}px; height: ${208 * S}px; margin: 0 -9px;
    background-size: ${1536 * S}px ${1872 * S}px; background-position: 0 0;
    image-rendering: pixelated;
  }
  </style></head><body>
  <h1>Devlings</h1>
  <p class="tag">Desktop pets for Claude Code</p>
  <p class="note">Free and open source · Windows, macOS and Linux</p>
  <div class="pets">${pets}</div>
  </body></html>`;
}

async function captureSocial(cdp) {
  const tab = await openTab(cdp, { width: 1280, height: 640, theme: 'light', scale: 1 });
  try {
    const url = `data:text/html;base64,${Buffer.from(socialHtml()).toString('base64')}`;
    await tab.goto(url);
    await tab.waitFor(`document.fonts.status === 'loaded' && document.querySelectorAll('.pet').length === ${PET_IDS.length}`, 'the social preview');
    await sleep(300);
    const png = PNG.sync.read(await tab.screenshot());
    const file = path.join(ROOT, 'docs', 'social-preview.png');
    // Re-encoded by pngjs: pixels only, no metadata.
    fs.writeFileSync(file, PNG.sync.write(png, { colorType: 2 }));
    return { file, size: `${png.width}x${png.height}`, bytes: fs.statSync(file).size };
  } finally {
    await tab.close();
  }
}

// ---- Main ----------------------------------------------------------------------------------------------

async function startVite() {
  const { createServer } = await import('vite');
  const server = await createServer({ root: ROOT, logLevel: 'warn', server: { port: 1430, strictPort: false } });
  await server.listen();
  const url = server.resolvedUrls?.local?.[0]?.replace(/\/$/, '');
  if (!url) throw new Error("Vite didn't report its address");
  return { url, close: () => server.close() };
}

/** `--name value` from the command line. */
function arg(name) {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : undefined;
}

async function main() {
  // --only cards,demo,social and --theme dark re-make just those; --url uses a dev server that's already running.
  const only = arg('only')?.split(',');
  const wanted = (name) => !only || only.includes(name);
  const themes = arg('theme') ? [arg('theme')] : Object.keys(THEMES);
  const vite = arg('url') ? { url: arg('url').replace(/\/$/, ''), close: async () => {} } : await startVite();
  const browser = await launchBrowser();
  const rel = (f) => path.relative(ROOT, f).split(path.sep).join('/');
  try {
    for (const theme of themes) {
      for (const shot of STILLS.filter((x) => wanted(x.name))) {
        const r = await captureStill(browser.cdp, vite.url, shot, theme);
        console.log(`${rel(r.file)}: ${r.size}, ${r.bytes} bytes`);
      }
      if (!wanted('demo')) continue;
      const demo = await recordDemo(browser.cdp, vite.url, theme);
      const file = path.join(SHOTS_DIR, `demo${THEMES[theme].suffix}.gif`);
      const r = encodeDemo(demo.frames, demo.crop, file);
      console.log(`${rel(file)}: ${r.size}, ${r.frames} frames, ${r.bytes} bytes`);
      if (r.bytes > GIF_BUDGET) console.warn(`  over the ${GIF_BUDGET} byte budget`);
    }
    if (wanted('social')) {
      const social = await captureSocial(browser.cdp);
      console.log(`${rel(social.file)}: ${social.size}, ${social.bytes} bytes`);
    }
  } finally {
    await browser.close();
    await vite.close();
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((e) => {
    console.error(e instanceof Error ? e.message : e);
    process.exit(1);
  });
}
