import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Clip } from "./petAnimation";
import { useSpriteClip } from "./useSpriteClip";

function Probe({ clip, onDone }: { clip: Clip; onDone?: () => void }) {
  const frame = useSpriteClip(clip, onDone);
  return <span data-testid="f">{frame}</span>;
}

const frame = (c: HTMLElement) => Number(c.querySelector("[data-testid=f]")!.textContent);

describe("useSpriteClip", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("advances a looping clip on the table's schedule", () => {
    const { container } = render(<Probe clip={{ name: "idle", loop: true, still: false, key: "idle" }} />);
    expect(frame(container)).toBe(0);
    act(() => void vi.advanceTimersByTime(280));
    expect(frame(container)).toBe(1);
    act(() => void vi.advanceTimersByTime(110));
    expect(frame(container)).toBe(2);
    act(() => void vi.advanceTimersByTime(110 + 140 + 140 + 320));
    expect(frame(container)).toBe(0);
  });

  it("finishes a one-shot clip once and reports it", () => {
    const onDone = vi.fn();
    const { container } = render(
      <Probe clip={{ name: "waving", loop: false, still: false, key: "wave-1" }} onDone={onDone} />,
    );
    act(() => void vi.advanceTimersByTime(700));
    expect(frame(container)).toBe(3);
    expect(onDone).toHaveBeenCalledTimes(1);
    act(() => void vi.advanceTimersByTime(5000));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it("holds frame 0 when still", () => {
    const { container } = render(<Probe clip={{ name: "running", loop: false, still: true, key: "still" }} />);
    act(() => void vi.advanceTimersByTime(2000));
    expect(frame(container)).toBe(0);
  });
});
