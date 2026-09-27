import type { PetState, ThreadInfo } from "../shared/types";
import type { RowName } from "./atlas";

// Spec section 2: which row plays for the pet's overall state, plus the
// interaction overrides (hover wave, drag run, jump when a thread gets ready).

export const WAVE_COOLDOWN_MS = 4000;
export const DRAG_SETTLE_MS = 150;

export function rowForState(state: PetState): RowName {
  switch (state) {
    case "running":
      return "running";
    case "needs_input":
    case "setup":
      return "waiting";
    case "blocked":
      return "failed";
    case "ready":
      return "review";
    case "idle":
    default:
      return "idle";
  }
}

export type DragDir = "right" | "left";

export interface AnimState {
  /** A clip that plays once and then hands back to the state row. */
  oneShot: "waving" | "jumping" | null;
  /** Bumped on every one-shot so replaying the same clip restarts it. */
  oneShotId: number;
  drag: DragDir | null;
  lastWaveAt: number;
}

export const initialAnim: AnimState = { oneShot: null, oneShotId: 0, drag: null, lastWaveAt: -Infinity };

export type AnimAction =
  | { type: "hover"; now: number }
  | { type: "ready" }
  | { type: "drag"; dir: DragDir }
  | { type: "dragEnd" }
  | { type: "clipDone" };

export function animReducer(s: AnimState, a: AnimAction): AnimState {
  switch (a.type) {
    case "hover":
      // Waves at most once per 4 s, and never interrupts a drag or a jump.
      if (s.drag || s.oneShot || a.now - s.lastWaveAt < WAVE_COOLDOWN_MS) return s;
      return { ...s, oneShot: "waving", oneShotId: s.oneShotId + 1, lastWaveAt: a.now };
    case "ready":
      return { ...s, oneShot: "jumping", oneShotId: s.oneShotId + 1 };
    case "drag":
      return s.drag === a.dir ? s : { ...s, drag: a.dir };
    case "dragEnd":
      return s.drag ? { ...s, drag: null } : s;
    case "clipDone":
      return s.oneShot ? { ...s, oneShot: null } : s;
  }
}

/**
 * How a looping row rests: after `afterMs` of playing back to back it finishes the play in progress, then holds
 * frame 0 for `minMs` to `maxMs` between plays (see `spriteClock.ts`). Motion in the corner of the eye is hard to
 * ignore, so a state that lasts keeps moving less once it has been noticed (design v1.1 §2a).
 */
export interface RestPolicy {
  afterMs: number;
  minMs: number;
  maxMs: number;
}

/** Idle breathes from the start. */
export const IDLE_REST: RestPolicy = { afterMs: 0, minMs: 4000, maxMs: 8000 };
/** Working: a minute of typing, then short pauses, so a long run isn't constant motion. */
export const WORKING_REST: RestPolicy = { afterMs: 60_000, minMs: 1500, maxMs: 3000 };
/** Failed: a few seconds of storm, then the cloud holds still and only stirs now and then. */
export const FAILED_REST: RestPolicy = { afterMs: 8000, minMs: 6000, maxMs: 10_000 };
/** Done: reviewing settles too. */
export const REVIEW_REST: RestPolicy = { afterMs: 20_000, minMs: 4000, maxMs: 8000 };

// "Waiting" never rests: it's the one state meant to keep catching the eye until it's answered.
const REST_FOR: Partial<Record<RowName, RestPolicy>> = {
  idle: IDLE_REST,
  running: WORKING_REST,
  failed: FAILED_REST,
  review: REVIEW_REST,
};

export interface Clip {
  name: RowName;
  loop: boolean;
  /** Reduced motion: hold frame 0. */
  still: boolean;
  /** Resting between plays; without it the row loops back to back. */
  rest?: RestPolicy;
  /** Changes whenever the clip should restart from frame 0. */
  key: string;
}

export function resolveClip(state: PetState, anim: AnimState, reducedMotion: boolean): Clip {
  const base = rowForState(state);
  if (reducedMotion) return { name: base, loop: false, still: true, key: `still-${base}` };
  if (anim.drag) {
    const name: RowName = anim.drag === "right" ? "running-right" : "running-left";
    return { name, loop: true, still: false, key: name };
  }
  if (anim.oneShot) return { name: anim.oneShot, loop: false, still: false, key: `${anim.oneShot}-${anim.oneShotId}` };
  return { name: base, loop: true, still: false, rest: REST_FOR[base], key: base };
}

/** True when some thread is ready now that wasn't ready in the previous snapshot. */
export function becameReady(prev: ThreadInfo[] | null, next: ThreadInfo[]): boolean {
  if (!prev) return false;
  const wasReady = new Set(prev.filter((t) => t.status === "ready").map((t) => t.sessionId));
  return next.some((t) => t.status === "ready" && !wasReady.has(t.sessionId));
}

/** Direction of a window move, or null when it moved only vertically. */
export function dragDirection(dx: number): DragDir | null {
  if (dx > 0) return "right";
  if (dx < 0) return "left";
  return null;
}
