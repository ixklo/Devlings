// Codex-compatible pet atlas: 1536x1872, 8 columns x 9 rows of 192x208 cells.
// Spec section 1 has the row table; the durations below must match it exactly.

export const CELL_W = 192;
export const CELL_H = 208;
export const COLUMNS = 8;
export const ROWS = 9;
export const ATLAS_W = CELL_W * COLUMNS; // 1536
export const ATLAS_H = CELL_H * ROWS; // 1872

export type RowName =
  | "idle"
  | "running-right"
  | "running-left"
  | "waving"
  | "jumping"
  | "failed"
  | "waiting"
  | "running"
  | "review";

const rep = (ms: number, n: number) => Array.from({ length: n }, () => ms);

export const ROW_SPECS: Record<RowName, { row: number; durations: readonly number[] }> = {
  idle: { row: 0, durations: [280, 110, 110, 140, 140, 320] },
  "running-right": { row: 1, durations: [...rep(120, 7), 220] },
  "running-left": { row: 2, durations: [...rep(120, 7), 220] },
  waving: { row: 3, durations: [140, 140, 140, 280] },
  jumping: { row: 4, durations: [140, 140, 140, 140, 280] },
  failed: { row: 5, durations: [...rep(140, 7), 240] },
  waiting: { row: 6, durations: [...rep(150, 5), 260] },
  running: { row: 7, durations: [...rep(120, 5), 220] },
  review: { row: 8, durations: [...rep(150, 5), 280] },
};

export interface FrameAt {
  /** Column of the frame to show. */
  frame: number;
  /** A one-shot clip has played through; it rests on its last frame. */
  done: boolean;
  /** Milliseconds until the frame changes; Infinity once a one-shot is done. */
  nextIn: number;
}

/**
 * Which frame of a clip is showing `elapsed` ms after it started.
 * Looping clips wrap around; one-shot clips stop on their last frame.
 */
export function frameAt(durations: readonly number[], elapsed: number, loop: boolean): FrameAt {
  const total = durations.reduce((a, b) => a + b, 0);
  if (durations.length === 0 || total <= 0) return { frame: 0, done: true, nextIn: Infinity };
  let t = Math.max(0, elapsed);
  if (loop) t %= total;
  else if (t >= total) return { frame: durations.length - 1, done: true, nextIn: Infinity };
  for (let i = 0; i < durations.length; i++) {
    if (t < durations[i]) return { frame: i, done: false, nextIn: durations[i] - t };
    t -= durations[i];
  }
  // Unreachable for finite input; keeps the type checker honest.
  return { frame: 0, done: false, nextIn: durations[0] };
}

/** CSS `background-position` that shows one cell at scale `s`. */
export function cellPosition(row: number, frame: number, s: number): string {
  return `${-frame * CELL_W * s}px ${-row * CELL_H * s}px`;
}

/** A cell's box and the sheet's size at scale `s`: everything but which cell shows. */
export function cellBox(s: number) {
  return { width: CELL_W * s, height: CELL_H * s, backgroundSize: `${ATLAS_W * s}px ${ATLAS_H * s}px` };
}

/** CSS background geometry for one cell at scale `s`. */
export function cellStyle(row: number, frame: number, s: number) {
  return { ...cellBox(s), backgroundPosition: cellPosition(row, frame, s) };
}
