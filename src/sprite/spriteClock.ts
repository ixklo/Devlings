import { frameAt } from "./atlas";

// The idle row "breathes": it plays once, then the pet holds still on frame 0 for a few seconds
// before playing it again. A sprite sitting idle on the desktop for hours then wakes the page
// about once a second instead of on every frame. Working, failed and review do the same once they
// have lasted a while (`RestPolicy` in petAnimation.ts); waiting, the drag run and the one-shots
// keep playing back to back.

/** Shortest and longest rest between two plays of the idle row, in ms. */
export const REST_MIN_MS = 4000;
export const REST_MAX_MS = 8000;

/** The rest after one play, from a draw in [0, 1): 4 to 8 s by default, so it never looks mechanical. */
export function restLength(random: number, minMs = REST_MIN_MS, maxMs = REST_MAX_MS): number {
  const r = Number.isFinite(random) ? Math.min(Math.max(random, 0), 1) : 0.5;
  return Math.round(minMs + r * (maxMs - minMs));
}

export interface RestingFrame {
  /** Column of the frame to show. */
  frame: number;
  /** Milliseconds until the frame changes (the whole rest, while resting). */
  nextIn: number;
  /** Play and rest are both over: start the next cycle now. */
  cycleOver: boolean;
}

/**
 * Which frame of a resting loop shows `elapsed` ms into its current cycle: the row plays once on
 * its own schedule, then frame 0 holds for `restMs`.
 */
export function restingFrameAt(durations: readonly number[], elapsed: number, restMs: number): RestingFrame {
  const play = durations.reduce((a, b) => a + b, 0);
  const t = Math.max(0, elapsed);
  if (t >= play + restMs) return { frame: 0, nextIn: 0, cycleOver: true };
  if (t >= play) return { frame: 0, nextIn: play + restMs - t, cycleOver: false };
  const f = frameAt(durations, t, false);
  return { frame: f.frame, nextIn: f.nextIn, cycleOver: false };
}

/**
 * Whether a sprite's frame timer should be running at all. It never runs for a still clip
 * (reduced motion), while the page is hidden (a hidden window hides its page, see design D17),
 * or once a one-shot has played through.
 */
export function clockRuns(clip: { still: boolean }, pageHidden: boolean, done: boolean): boolean {
  return !clip.still && !pageHidden && !done;
}
