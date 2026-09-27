import { act, render, screen } from "@testing-library/react";
import { Profiler, useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CELL_H, CELL_W } from "./atlas";
import type { Clip } from "./petAnimation";
import { SpriteView } from "./SpriteView";
import { useSpriteClip } from "./useSpriteClip";

function Probe({ clip, scale = 1, onDone }: { clip: Clip; scale?: number; onDone?: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useSpriteClip(ref, clip, scale, onDone);
  return <div ref={ref} data-testid="s" />;
}

const el = () => screen.getByTestId("s");
const frame = () => Number(el().dataset.frame);
const advance = (ms: number) => act(() => void vi.advanceTimersByTime(ms));

let visibility: DocumentVisibilityState = "visible";
function setVisibility(v: DocumentVisibilityState) {
  visibility = v;
  act(() => void document.dispatchEvent(new Event("visibilitychange")));
}

const IDLE_LOOP: Clip = { name: "idle", loop: true, still: false, key: "idle" };
const IDLE_REST: Clip = { name: "idle", loop: true, still: false, rest: { afterMs: 0, minMs: 4000, maxMs: 8000 }, key: "idle" };
const RUNNING_SETTLES: Clip = {
  name: "running",
  loop: true,
  still: false,
  rest: { afterMs: 2000, minMs: 1000, maxMs: 1000 },
  key: "running",
};
const RUNNING: Clip = { name: "running", loop: true, still: false, key: "running" };

describe("useSpriteClip", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    visibility = "visible";
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => visibility });
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    delete (document as { visibilityState?: unknown }).visibilityState;
  });

  it("advances a looping clip on the table's schedule", () => {
    render(<Probe clip={IDLE_LOOP} />);
    expect(frame()).toBe(0);
    advance(280);
    expect(frame()).toBe(1);
    advance(110);
    expect(frame()).toBe(2);
    advance(110 + 140 + 140 + 320);
    expect(frame()).toBe(0);
  });

  it("moves the cell by background-position on the row of the clip", () => {
    render(<Probe clip={RUNNING} scale={0.5} />);
    expect(el().style.backgroundPosition).toBe(`0px ${-7 * CELL_H * 0.5}px`);
    advance(120);
    expect(el().style.backgroundPosition).toBe(`${-CELL_W * 0.5}px ${-7 * CELL_H * 0.5}px`);
  });

  it("finishes a one-shot clip once and reports it", () => {
    const onDone = vi.fn();
    render(<Probe clip={{ name: "waving", loop: false, still: false, key: "wave-1" }} onDone={onDone} />);
    advance(700);
    expect(frame()).toBe(3);
    expect(onDone).toHaveBeenCalledTimes(1);
    advance(5000);
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("holds frame 0 with no timer at all when still (reduced motion)", () => {
    render(<Probe clip={{ name: "running", loop: false, still: true, key: "still-running" }} />);
    expect(frame()).toBe(0);
    expect(el().style.backgroundPosition).toBe(`0px ${-7 * CELL_H}px`);
    expect(vi.getTimerCount()).toBe(0);
    advance(2000);
    expect(frame()).toBe(0);
  });

  it("plays the idle loop once, then rests on frame 0 for 4-8 s before playing again", () => {
    vi.spyOn(Math, "random").mockReturnValue(0); // the shortest rest, 4 s
    render(<Probe clip={IDLE_REST} />);
    advance(280 + 110 + 110 + 140 + 140);
    expect(frame()).toBe(5);
    advance(320);
    expect(frame()).toBe(0); // resting
    advance(3999);
    expect(frame()).toBe(0);
    expect(vi.getTimerCount()).toBe(1); // one timer for the whole rest, not a tick per frame
    advance(1); // rest over: the next cycle starts on frame 0 ...
    expect(frame()).toBe(0);
    advance(280); // ... and plays again
    expect(frame()).toBe(1);
  });

  it("plays an active row back to back at first, then finishes the play in progress and rests between plays", () => {
    render(<Probe clip={RUNNING_SETTLES} />); // the row is 5 x 120 ms + 220 ms = 820 ms
    advance(940); // second play, no rest before it
    expect(frame()).toBe(1);
    advance(1060); // 2 s in: settling starts, but the third play (from 1640 ms) carries on
    expect(frame()).toBe(3);
    advance(460); // that play ends at 2460 ms: rest on frame 0
    expect(frame()).toBe(0);
    advance(999);
    expect(frame()).toBe(0);
    expect(vi.getTimerCount()).toBe(1);
    advance(1); // rest over, the next play starts
    advance(120);
    expect(frame()).toBe(1);
  });

  it("stops its timer while the page is hidden and picks up again when shown", () => {
    render(<Probe clip={RUNNING} />);
    expect(vi.getTimerCount()).toBe(1);
    setVisibility("hidden");
    expect(vi.getTimerCount()).toBe(0);
    const held = frame();
    advance(10_000);
    expect(frame()).toBe(held);
    setVisibility("visible");
    expect(vi.getTimerCount()).toBe(1);
    advance(120);
    expect(frame()).not.toBe(held);
  });

  it("starts stopped when mounted in a hidden page", () => {
    visibility = "hidden";
    render(<Probe clip={RUNNING} />);
    expect(frame()).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
    setVisibility("visible");
    expect(vi.getTimerCount()).toBe(1);
  });

  it("finishes a one-shot that ran out while hidden as soon as the page is shown", () => {
    const onDone = vi.fn();
    render(<Probe clip={{ name: "jumping", loop: false, still: false, key: "jump-1" }} onDone={onDone} />);
    setVisibility("hidden");
    advance(5000);
    expect(onDone).not.toHaveBeenCalled();
    setVisibility("visible");
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(frame()).toBe(4);
  });

  it("clears its timer and listener on unmount", () => {
    const { unmount } = render(<Probe clip={RUNNING} />);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
    setVisibility("hidden");
    setVisibility("visible");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("repositions the showing frame for a new scale without restarting the clip", () => {
    const { rerender } = render(<Probe clip={RUNNING} scale={1} />);
    advance(240);
    expect(frame()).toBe(2);
    rerender(<Probe clip={RUNNING} scale={0.5} />);
    expect(frame()).toBe(2);
    expect(el().style.backgroundPosition).toBe(`${-2 * CELL_W * 0.5}px ${-7 * CELL_H * 0.5}px`);
  });

  it("restarts from frame 0 of the new row when the clip changes", () => {
    const { rerender } = render(<Probe clip={RUNNING} />);
    advance(240);
    rerender(<Probe clip={{ name: "waiting", loop: true, still: false, key: "waiting" }} />);
    expect(frame()).toBe(0);
    expect(el().style.backgroundPosition).toBe(`0px ${-6 * CELL_H}px`);
  });
});

describe("SpriteView", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("advances frames without re-rendering React", () => {
    const commits = vi.fn();
    const { container } = render(
      <Profiler id="sprite" onRender={commits}>
        <SpriteView src="data:image/png;base64,AAAA" scale={1} clip={RUNNING} />
      </Profiler>,
    );
    const sprite = container.querySelector(".sprite") as HTMLElement;
    const seen = new Set<string>();
    for (let i = 0; i < 12; i++) {
      seen.add(sprite.style.backgroundPosition);
      advance(120);
    }
    expect(seen.size).toBe(6); // every frame of the running row showed ...
    expect(commits).toHaveBeenCalledTimes(1); // ... from the mount's single render
  });
});
