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

export interface Clip {
  name: RowName;
  loop: boolean;
  /** Reduced motion: hold frame 0. */
  still: boolean;
  /**
   * A loop that rests on frame 0 for 4-8 s between plays (the idle row only; see `spriteClock.ts`).
   * Active rows loop back to back.
   */
  rest?: boolean;
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
  return { name: base, loop: true, still: false, rest: base === "idle", key: base };
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
