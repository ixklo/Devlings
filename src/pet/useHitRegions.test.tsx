import { act, render } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HitRect } from "../shared/types";
import { collectHitRects, HIT_THROTTLE_MS, useHitRegions } from "./useHitRegions";

function rect(x: number, y: number, w: number, h: number): DOMRect {
  return { x, y, left: x, top: y, width: w, height: h, right: x + w, bottom: y + h, toJSON: () => ({}) } as DOMRect;
}

function box(el: HTMLElement | null, r: DOMRect) {
  if (el) el.getBoundingClientRect = () => r;
}

describe("collectHitRects", () => {
  it("reports rounded rects for visible [data-hit] elements only", () => {
    document.body.innerHTML = `<div data-hit id="a"></div><div data-hit id="b"></div><div id="c"></div>`;
    box(document.getElementById("a"), rect(10.4, 20.6, 100.2, 40));
    box(document.getElementById("b"), rect(0, 0, 0, 0));
    box(document.getElementById("c"), rect(1, 1, 50, 50));
    expect(collectHitRects()).toEqual([{ x: 10, y: 21, w: 100, h: 40 }]);
    document.body.innerHTML = "";
  });

  it("names rects whose data-hit has a value, so the backend can report hover", () => {
    document.body.innerHTML = `<div data-hit="pet" id="p"></div><div data-hit="" id="c"></div>`;
    box(document.getElementById("p"), rect(0, 500, 100, 100));
    box(document.getElementById("c"), rect(0, 0, 50, 50));
    expect(collectHitRects()).toEqual([
      { x: 0, y: 500, w: 100, h: 100, id: "pet" },
      { x: 0, y: 0, w: 50, h: 50 },
    ]);
    document.body.innerHTML = "";
  });
});

function Overlay({ send }: { send: (r: HitRect[]) => void }) {
  const [open, setOpen] = useState(false);
  const refresh = useHitRegions(send);
  return (
    <div>
      <button data-hit="" ref={(el) => box(el, rect(0, 500, 100, 100))} onClick={() => setOpen(true)} onFocus={refresh}>
        pet
      </button>
      {open && <div data-hit="" ref={(el) => box(el, rect(20, 100, 340, 200))} />}
    </div>
  );
}

describe("useHitRegions", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("sends on mount and again when an interactive element appears, throttled", async () => {
    const send = vi.fn();
    const { getByText } = render(<Overlay send={send} />);
    act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS));
    expect(send).toHaveBeenLastCalledWith([{ x: 0, y: 500, w: 100, h: 100 }]);

    act(() => getByText("pet").click());
    // MutationObserver callbacks are microtasks; let them run, then pass the throttle window.
    await act(async () => {
      await Promise.resolve();
    });
    act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS));
    expect(send).toHaveBeenLastCalledWith([
      { x: 0, y: 500, w: 100, h: 100 },
      { x: 20, y: 100, w: 340, h: 200 },
    ]);
  });

  it("re-reports as soon as a hidden page is shown again", async () => {
    // A hidden window hides its page (design D17), whose timers are throttled; re-check on show.
    const send = vi.fn();
    const { container } = render(<Overlay send={send} />);
    act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS));
    const count = send.mock.calls.length;
    box(container.querySelector("button"), rect(0, 480, 100, 120)); // moved without any DOM change
    act(() => void document.dispatchEvent(new Event("visibilitychange")));
    act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS));
    expect(send.mock.calls.length).toBe(count + 1);
    expect(send).toHaveBeenLastCalledWith([{ x: 0, y: 480, w: 100, h: 120 }]);
  });

  it("skips sending an unchanged set", () => {
    const send = vi.fn();
    const { getByText } = render(<Overlay send={send} />);
    act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS));
    const count = send.mock.calls.length;
    act(() => getByText("pet").focus());
    act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS * 2));
    expect(send.mock.calls.length).toBe(count);
  });
});
