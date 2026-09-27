import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { relativeTime, useNow } from "./time";

function Clock() {
  return <span data-testid="now">{useNow()}</span>;
}

const shown = () => Number(screen.getByTestId("now").textContent);

describe("relativeTime", () => {
  it("rounds to a compact label", () => {
    expect(relativeTime(0, 10_000)).toBe("now");
    expect(relativeTime(0, 120_000)).toBe("2m");
    expect(relativeTime(0, 3 * 3_600_000)).toBe("3h");
    expect(relativeTime(0, 4 * 86_400_000)).toBe("4d");
  });
});

describe("useNow", () => {
  let visibility: DocumentVisibilityState = "visible";
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"] });
    visibility = "visible";
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => visibility });
  });
  afterEach(() => {
    vi.useRealTimers();
    delete (document as { visibilityState?: unknown }).visibilityState;
  });

  it("ticks every 30 s", () => {
    render(<Clock />);
    const first = shown();
    act(() => void vi.advanceTimersByTime(30_000));
    expect(shown()).toBe(first + 30_000);
  });

  it("catches up as soon as a hidden page is shown again", () => {
    // A hidden page's timers are throttled (design D17), so relative times would lag after a show.
    render(<Clock />);
    const first = shown();
    visibility = "hidden";
    act(() => void document.dispatchEvent(new Event("visibilitychange")));
    vi.setSystemTime(first + 10_000); // time passes without the interval firing
    expect(shown()).toBe(first);
    visibility = "visible";
    act(() => void document.dispatchEvent(new Event("visibilitychange")));
    expect(shown()).toBe(first + 10_000);
  });
});
