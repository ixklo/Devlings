import { describe, expect, it } from "vitest";
import type { PetState } from "../shared/types";
import { makeThread } from "../test/fixtures";
import {
  animReducer,
  becameReady,
  dragDirection,
  FAILED_REST,
  IDLE_REST,
  initialAnim,
  resolveClip,
  REVIEW_REST,
  rowForState,
  WAVE_COOLDOWN_MS,
  WORKING_REST,
  type AnimAction,
  type AnimState,
} from "./petAnimation";

const run = (actions: AnimAction[], start: AnimState = initialAnim) => actions.reduce(animReducer, start);

describe("rowForState", () => {
  it("maps each pet state to its row", () => {
    const states: PetState[] = ["idle", "running", "needs_input", "blocked", "ready", "setup"];
    expect(states.map(rowForState)).toEqual(["idle", "running", "waiting", "failed", "review", "waiting"]);
  });
});

describe("resolveClip", () => {
  it("loops the state row with no overrides", () => {
    expect(resolveClip("running", initialAnim, false)).toMatchObject({ name: "running", loop: true, still: false });
  });

  it("idle rests from the start; working, failed and done settle after a while; waiting never rests", () => {
    expect(resolveClip("idle", initialAnim, false)).toMatchObject({ name: "idle", loop: true, rest: IDLE_REST });
    expect(IDLE_REST.afterMs).toBe(0);
    expect(resolveClip("running", initialAnim, false).rest).toBe(WORKING_REST);
    expect(resolveClip("blocked", initialAnim, false).rest).toBe(FAILED_REST);
    expect(resolveClip("ready", initialAnim, false).rest).toBe(REVIEW_REST);
    for (const policy of [WORKING_REST, FAILED_REST, REVIEW_REST]) expect(policy.afterMs).toBeGreaterThan(0);
    // "Needs you" keeps moving: it's the one state meant to keep catching the eye.
    const waiting: PetState[] = ["needs_input", "setup"];
    for (const state of waiting) expect(resolveClip(state, initialAnim, false).rest).toBeUndefined();
    // Overrides and the reduced-motion still frame never rest.
    expect(resolveClip("idle", run([{ type: "hover", now: 10_000 }]), false).rest).toBeFalsy();
    expect(resolveClip("idle", run([{ type: "drag", dir: "left" }]), false).rest).toBeFalsy();
    expect(resolveClip("idle", initialAnim, true).rest).toBeFalsy();
  });

  it("waves once on hover, then returns to the state row", () => {
    const waving = run([{ type: "hover", now: 10_000 }]);
    expect(resolveClip("idle", waving, false)).toMatchObject({ name: "waving", loop: false });
    const back = animReducer(waving, { type: "clipDone" });
    expect(resolveClip("idle", back, false)).toMatchObject({ name: "idle", loop: true });
  });

  it("waves at most once per 4 seconds", () => {
    const first = run([{ type: "hover", now: 10_000 }, { type: "clipDone" }]);
    const tooSoon = animReducer(first, { type: "hover", now: 10_000 + WAVE_COOLDOWN_MS - 1 });
    expect(tooSoon.oneShot).toBeNull();
    const later = animReducer(first, { type: "hover", now: 10_000 + WAVE_COOLDOWN_MS });
    expect(later.oneShot).toBe("waving");
  });

  it("jumps once when a thread gets ready, then plays review", () => {
    const jumping = run([{ type: "ready" }]);
    expect(resolveClip("ready", jumping, false)).toMatchObject({ name: "jumping", loop: false });
    expect(resolveClip("ready", animReducer(jumping, { type: "clipDone" }), false)).toMatchObject({
      name: "review",
      loop: true,
    });
  });

  it("runs right or left while dragging, over any other clip", () => {
    const right = run([{ type: "ready" }, { type: "drag", dir: "right" }]);
    expect(resolveClip("needs_input", right, false)).toMatchObject({ name: "running-right", loop: true });
    const left = animReducer(right, { type: "drag", dir: "left" });
    expect(resolveClip("needs_input", left, false).name).toBe("running-left");
    const settled = animReducer(left, { type: "dragEnd" });
    expect(resolveClip("needs_input", settled, false).name).not.toMatch(/^running-/);
  });

  it("does not wave during a drag", () => {
    const dragging = run([{ type: "drag", dir: "left" }, { type: "hover", now: 99_000 }]);
    expect(dragging.oneShot).toBeNull();
  });

  it("restarts a replayed one-shot with a new key", () => {
    const a = resolveClip("ready", run([{ type: "ready" }]), false).key;
    const b = resolveClip("ready", run([{ type: "ready" }, { type: "clipDone" }, { type: "ready" }]), false).key;
    expect(a).not.toBe(b);
  });

  it("holds frame 0 of the state row for reduced motion, ignoring overrides", () => {
    const busy = run([{ type: "drag", dir: "right" }]);
    expect(resolveClip("blocked", busy, true)).toMatchObject({ name: "failed", still: true });
  });
});

describe("becameReady", () => {
  it("fires only for threads that are newly ready", () => {
    const running = makeThread({ sessionId: "a", status: "running" });
    const ready = { ...running, status: "ready" as const };
    expect(becameReady(null, [ready])).toBe(false);
    expect(becameReady([running], [ready])).toBe(true);
    expect(becameReady([ready], [ready])).toBe(false);
    expect(becameReady([ready], [])).toBe(false);
  });
});

describe("dragDirection", () => {
  it("follows the horizontal delta", () => {
    expect(dragDirection(3)).toBe("right");
    expect(dragDirection(-1)).toBe("left");
    expect(dragDirection(0)).toBeNull();
  });
});
