import { describe, expect, it } from "vitest";
import { ATLAS_H, ATLAS_W, CELL_H, CELL_W, cellStyle, frameAt, ROW_SPECS } from "./atlas";

describe("atlas contract", () => {
  it("is 1536x1872 in 192x208 cells", () => {
    expect([ATLAS_W, ATLAS_H, CELL_W, CELL_H]).toEqual([1536, 1872, 192, 208]);
  });

  it("uses the exact per-row frame durations from the spec", () => {
    const table = Object.fromEntries(Object.entries(ROW_SPECS).map(([k, v]) => [k, [v.row, [...v.durations]]]));
    expect(table).toEqual({
      idle: [0, [280, 110, 110, 140, 140, 320]],
      "running-right": [1, [120, 120, 120, 120, 120, 120, 120, 220]],
      "running-left": [2, [120, 120, 120, 120, 120, 120, 120, 220]],
      waving: [3, [140, 140, 140, 280]],
      jumping: [4, [140, 140, 140, 140, 280]],
      failed: [5, [140, 140, 140, 140, 140, 140, 140, 240]],
      waiting: [6, [150, 150, 150, 150, 150, 260]],
      running: [7, [120, 120, 120, 120, 120, 220]],
      review: [8, [150, 150, 150, 150, 150, 280]],
    });
  });
});

describe("frameAt", () => {
  const idle = ROW_SPECS.idle.durations; // 280,110,110,140,140,320 = 1100

  it("walks the frames by their own durations", () => {
    expect(frameAt(idle, 0, true)).toEqual({ frame: 0, done: false, nextIn: 280 });
    expect(frameAt(idle, 279, true).frame).toBe(0);
    expect(frameAt(idle, 280, true)).toEqual({ frame: 1, done: false, nextIn: 110 });
    expect(frameAt(idle, 450, true)).toEqual({ frame: 2, done: false, nextIn: 50 });
    expect(frameAt(idle, 500, true)).toEqual({ frame: 3, done: false, nextIn: 140 });
    expect(frameAt(idle, 1099, true).frame).toBe(5);
  });

  it("wraps looping clips", () => {
    expect(frameAt(idle, 1100, true)).toEqual({ frame: 0, done: false, nextIn: 280 });
    expect(frameAt(idle, 1100 * 3 + 300, true).frame).toBe(1);
  });

  it("stops one-shot clips on their last frame", () => {
    const wave = ROW_SPECS.waving.durations; // 140,140,140,280 = 700
    expect(frameAt(wave, 699, false)).toEqual({ frame: 3, done: false, nextIn: 1 });
    expect(frameAt(wave, 700, false)).toEqual({ frame: 3, done: true, nextIn: Infinity });
    expect(frameAt(wave, 5000, false).done).toBe(true);
  });

  it("treats negative time as the start", () => {
    expect(frameAt(idle, -50, true).frame).toBe(0);
  });
});

describe("cellStyle", () => {
  it("scales the sheet and offsets to the cell", () => {
    expect(cellStyle(8, 2, 0.5)).toEqual({
      width: 96,
      height: 104,
      backgroundSize: "768px 936px",
      backgroundPosition: "-192px -832px",
    });
  });
});
