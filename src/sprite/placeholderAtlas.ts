import { ATLAS_H, ATLAS_W, CELL_H, CELL_W, ROW_SPECS, type RowName } from "./atlas";

// A procedurally drawn pixel bird in the Codex atlas layout. Used by the
// browser preview, and as a stand-in if a pet's real spritesheet fails to load
// so the pet never disappears from the desktop.

const P = 8; // one art pixel = 8 atlas px, so a cell is a 24x26 grid
const GW = CELL_W / P; // 24
const GH = CELL_H / P; // 26

const EMPTY = 0, BODY = 1, DARK = 2, BELLY = 3, BEAK = 4, EYE = 5, OUTLINE = 6, SPARK = 7, DROP = 8, MOTE = 9;
const SOLID = new Set([BODY, DARK, BELLY, BEAK]);

interface Palette {
  body: string;
  dark: string;
  belly: string;
  beak: string;
}

const PALETTES: Record<string, Palette> = {
  perch: { body: "#35a89c", dark: "#1f7a71", belly: "#f4ecdc", beak: "#f2a541" },
  ember: { body: "#f0793b", dark: "#b9501f", belly: "#fde6d2", beak: "#ffd166" },
  plum: { body: "#9163d9", dark: "#6440ab", belly: "#f1e6fc", beak: "#f4b54a" },
  sprout: { body: "#69b947", dark: "#478f2f", belly: "#eef7e1", beak: "#f19b3c" },
};

function paletteFor(id: string): Palette {
  if (PALETTES[id]) return PALETTES[id];
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) % 360;
  return { body: `hsl(${h} 55% 52%)`, dark: `hsl(${h} 55% 36%)`, belly: `hsl(${h} 60% 93%)`, beak: "#f2a541" };
}

type Eyes = "open" | "closed" | "x" | "happy";
type Wing = "rest" | "mid" | "up";
type Legs = "stand" | "a" | "b" | "tuck";
type Fx = { kind: "spark" | "drop" | "dot" | "line"; x: number; y: number };

interface Pose {
  dx: number;
  dy: number;
  flip: boolean;
  eyes: Eyes;
  wing: Wing;
  legs: Legs;
  fx: Fx[];
}

const ORBIT: [number, number][] = [
  [3, 7],
  [19, 4],
  [21, 10],
  [2, 12],
];

function pose(row: RowName, f: number): Pose {
  const p: Pose = { dx: 0, dy: 0, flip: false, eyes: "open", wing: "rest", legs: "stand", fx: [] };
  switch (row) {
    case "idle":
      p.dy = [0, 0, 0, 1, 1, 0][f];
      p.eyes = f === 2 ? "closed" : "open";
      break;
    case "running-right":
    case "running-left":
      p.flip = row === "running-left";
      p.dy = f % 2 ? -1 : 0;
      p.legs = f % 2 ? "a" : "b";
      p.wing = f % 4 < 2 ? "mid" : "rest";
      p.fx = [
        { kind: "line", x: 0, y: 13 + (f % 2) },
        { kind: "line", x: 1, y: 18 - (f % 2) },
      ];
      break;
    case "waving":
      p.wing = (["mid", "up", "mid", "rest"] as Wing[])[f];
      p.eyes = f < 3 ? "happy" : "open";
      break;
    case "jumping":
      p.dy = [1, -3, -5, -3, 0][f];
      p.legs = f === 0 || f === 4 ? "stand" : "tuck";
      p.wing = f >= 1 && f <= 3 ? "up" : "rest";
      p.eyes = f >= 1 && f <= 3 ? "happy" : "open";
      break;
    case "failed":
      p.eyes = "x";
      p.dy = 1;
      p.dx = [0, 0, 1, 1, 0, 0, -1, -1][f];
      p.fx = [{ kind: "drop", x: 17, y: 15 + (f % 4) }];
      break;
    case "waiting": {
      const dots = [0, 1, 2, 3, 3, 3][f];
      p.fx = [0, 1, 2].slice(0, dots).map((i) => ({ kind: "dot", x: 14 + i * 2, y: 4 }) as Fx);
      p.dx = f >= 3 ? 1 : 0;
      break;
    }
    case "running":
      p.dy = f % 2 ? -1 : 0;
      p.wing = f % 2 ? "mid" : "rest";
      p.fx = [ORBIT[f % 4], ORBIT[(f + 2) % 4]].map(([x, y]) => ({ kind: "spark", x, y }) as Fx);
      break;
    case "review":
      p.eyes = "happy";
      p.dy = [0, -1, 0, 0, -1, 0][f];
      p.fx = [{ kind: "spark", ...(f % 2 ? { x: 20, y: 5 } : { x: 3, y: 8 }) }];
      break;
  }
  return p;
}

function drawGrid(p: Pose): Uint8Array {
  const g = new Uint8Array(GW * GH);
  const inside = (x: number, y: number) => x >= 0 && x < GW && y >= 0 && y < GH;
  const set = (x: number, y: number, c: number) => inside(x, y) && (g[y * GW + x] = c);
  const get = (x: number, y: number) => (inside(x, y) ? g[y * GW + x] : EMPTY);
  const disc = (cx: number, cy: number, rx: number, ry: number, c: number, onlyOn?: number) => {
    for (let y = 0; y < GH; y++)
      for (let x = 0; x < GW; x++) {
        const nx = (x + 0.5 - cx) / rx;
        const ny = (y + 0.5 - cy) / ry;
        if (nx * nx + ny * ny <= 1 && (onlyOn === undefined || get(x, y) === onlyOn)) set(x, y, c);
      }
  };

  const cx = 11.5 + p.dx;
  const cy = 16 + p.dy;
  const lx = 9 + p.dx;
  const rx = 13 + p.dx;
  const foot = 23 + Math.min(0, p.dy); // feet leave the ground when jumping

  // Legs sit behind the body.
  if (p.legs === "stand") {
    [lx, rx].forEach((x) => (set(x, foot, BEAK), set(x, foot + 1, BEAK), set(x + 1, foot + 1, BEAK)));
  } else if (p.legs === "a") {
    set(lx, foot, BEAK), set(lx - 1, foot + 1, BEAK);
    set(rx, foot, BEAK), set(rx + 1, foot + 1, BEAK), set(rx + 2, foot + 1, BEAK);
  } else if (p.legs === "b") {
    set(lx, foot, BEAK), set(lx + 1, foot + 1, BEAK), set(lx + 2, foot + 1, BEAK);
    set(rx, foot, BEAK), set(rx - 1, foot + 1, BEAK);
  } else {
    set(lx + 1, foot - 1, BEAK), set(rx - 1, foot - 1, BEAK);
  }

  disc(cx, cy, 7.5, 7, BODY);
  disc(cx + 1, cy + 2.5, 4.5, 3.8, BELLY, BODY);
  // Head tuft.
  set(Math.floor(cx) - 1, cy - 8, DARK), set(Math.floor(cx), cy - 9, DARK), set(Math.floor(cx), cy - 8, DARK);

  if (p.wing === "rest") disc(cx - 2.5, cy + 1.5, 3.5, 2.6, DARK);
  else if (p.wing === "mid") disc(cx - 3.5, cy - 0.5, 3.4, 2.4, DARK);
  else disc(cx - 6, cy - 5, 2.2, 3.6, DARK);

  // Beak on the front edge of row cy-2.
  const by = cy - 2;
  let edge = 0;
  for (let x = 0; x < GW; x++) if (SOLID.has(get(x, by))) edge = x;
  set(edge + 1, by, BEAK), set(edge + 2, by, BEAK), set(edge + 1, by + 1, BEAK);

  // Outline every empty cell touching the bird.
  const outline: number[] = [];
  for (let y = 0; y < GH; y++)
    for (let x = 0; x < GW; x++) {
      if (get(x, y) !== EMPTY) continue;
      if ([[1, 0], [-1, 0], [0, 1], [0, -1]].some(([ox, oy]) => SOLID.has(get(x + ox, y + oy)))) outline.push(y * GW + x);
    }
  outline.forEach((i) => (g[i] = OUTLINE));

  const ex = Math.floor(cx) + 4;
  const ey = cy - 4;
  if (p.eyes === "open") set(ex, ey, EYE), set(ex, ey + 1, EYE);
  else if (p.eyes === "closed") set(ex - 1, ey + 1, EYE), set(ex, ey + 1, EYE), set(ex + 1, ey + 1, EYE);
  else if (p.eyes === "happy") set(ex - 1, ey + 1, EYE), set(ex, ey, EYE), set(ex + 1, ey + 1, EYE);
  else [[-1, -1], [1, -1], [0, 0], [-1, 1], [1, 1]].forEach(([ox, oy]) => set(ex + ox, ey + oy, EYE));

  for (const fx of p.fx) {
    if (fx.kind === "spark") [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]].forEach(([ox, oy]) => set(fx.x + ox, fx.y + oy, SPARK));
    else if (fx.kind === "drop") set(fx.x, fx.y, DROP), set(fx.x, fx.y + 1, DROP);
    else if (fx.kind === "dot") set(fx.x, fx.y, MOTE);
    else set(fx.x, fx.y, MOTE), set(fx.x + 1, fx.y, MOTE), set(fx.x + 2, fx.y, MOTE);
  }

  if (p.flip) {
    for (let y = 0; y < GH; y++) g.subarray(y * GW, y * GW + GW).reverse();
  }
  return g;
}

const cache = new Map<string, string>();

/** A PNG data URL of a full 1536x1872 atlas for `id`, or null where canvas is unavailable. */
export function placeholderAtlas(id: string): string | null {
  const hit = cache.get(id);
  if (hit) return hit;
  if (typeof document === "undefined") return null;
  const canvas = document.createElement("canvas");
  canvas.width = ATLAS_W;
  canvas.height = ATLAS_H;
  let ctx: CanvasRenderingContext2D | null = null;
  try {
    ctx = canvas.getContext("2d");
  } catch {
    ctx = null;
  }
  if (!ctx) return null;
  const pal = paletteFor(id);
  const colors: Record<number, string> = {
    [BODY]: pal.body,
    [DARK]: pal.dark,
    [BELLY]: pal.belly,
    [BEAK]: pal.beak,
    [EYE]: "#17191d",
    [OUTLINE]: "#23262b",
    [SPARK]: "#ffcf4a",
    [DROP]: "#5aa9ff",
    [MOTE]: "#8a939e",
  };
  (Object.keys(ROW_SPECS) as RowName[]).forEach((name) => {
    const { row, durations } = ROW_SPECS[name];
    durations.forEach((_, frame) => {
      const g = drawGrid(pose(name, frame));
      for (let i = 0; i < g.length; i++) {
        if (g[i] === EMPTY) continue;
        ctx!.fillStyle = colors[g[i]];
        ctx!.fillRect(frame * CELL_W + (i % GW) * P, row * CELL_H + Math.floor(i / GW) * P, P, P);
      }
    });
  });
  const url = canvas.toDataURL("image/png");
  cache.set(id, url);
  return url;
}
