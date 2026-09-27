import { describe, expect, it } from "vitest";
import { ROW_SPECS } from "./atlas";
import { clockRuns, REST_MAX_MS, REST_MIN_MS, restingFrameAt, restLength } from "./spriteClock";

const IDLE = ROW_SPECS.idle.durations; // 280,110,110,140,140,320 = 1100 ms
const PLAY = IDLE.reduce((a, b) => a + b, 0);

describe("restLength", () => {
  it("rests 4 to 8 seconds, spread by the random draw", () => {
    expect(restLength(0)).toBe(REST_MIN_MS);
    expect(restLength(0.5)).toBe(6000);
    expect(restLength(0.999999)).toBeLessThanOrEqual(REST_MAX_MS);
    expect(REST_MIN_MS).toBe(4000);
    expect(REST_MAX_MS).toBe(8000);
  });

  it("stays in range for a bad draw", () => {
    for (const r of [-1, 2, Number.NaN, Number.POSITIVE_INFINITY]) {
      const ms = restLength(r);
      expect(ms).toBeGreaterThanOrEqual(REST_MIN_MS);
      expect(ms).toBeLessThanOrEqual(REST_MAX_MS);
    }
  });
});

describe("restingFrameAt", () => {
  it("plays the row once on the table's schedule", () => {
    expect(restingFrameAt(IDLE, 0, 5000)).toEqual({ frame: 0, nextIn: 280, cycleOver: false });
    expect(restingFrameAt(IDLE, 280, 5000)).toEqual({ frame: 1, nextIn: 110, cycleOver: false });
    expect(restingFrameAt(IDLE, 700, 5000)).toMatchObject({ frame: 4, nextIn: 80 });
    expect(restingFrameAt(IDLE, PLAY - 1, 5000)).toMatchObject({ frame: 5, nextIn: 1 });
  });

  it("then holds frame 0 for the whole rest with a single wake-up", () => {
    expect(restingFrameAt(IDLE, PLAY, 5000)).toEqual({ frame: 0, nextIn: 5000, cycleOver: false });
    expect(restingFrameAt(IDLE, PLAY + 4000, 5000)).toEqual({ frame: 0, nextIn: 1000, cycleOver: false });
  });

  it("reports the end of the cycle once play plus rest have passed", () => {
    expect(restingFrameAt(IDLE, PLAY + 5000, 5000)).toEqual({ frame: 0, nextIn: 0, cycleOver: true });
    // Long after (the page was hidden for an hour): still just "over", never a stale mid-loop frame.
    expect(restingFrameAt(IDLE, 3_600_000, 5000)).toEqual({ frame: 0, nextIn: 0, cycleOver: true });
  });

  it("treats a negative elapsed time as the start", () => {
    expect(restingFrameAt(IDLE, -50, 5000)).toEqual({ frame: 0, nextIn: 280, cycleOver: false });
  });
});

describe("clockRuns", () => {
  it("runs only for a moving clip on a visible page that hasn't finished", () => {
    expect(clockRuns({ still: false }, false, false)).toBe(true);
    expect(clockRuns({ still: true }, false, false)).toBe(false); // reduced motion: a still frame
    expect(clockRuns({ still: false }, true, false)).toBe(false); // page (or window) hidden
    expect(clockRuns({ still: false }, false, true)).toBe(false); // a one-shot that played through
  });
});
