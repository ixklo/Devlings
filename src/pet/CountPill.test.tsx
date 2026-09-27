import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { makeApproval, makeThread } from "../test/fixtures";
import { CountPill, hiddenCount } from "./CountPill";

describe("hiddenCount", () => {
  it("is nothing when nothing is hidden", () => {
    expect(hiddenCount([], [])).toBeNull();
  });

  it("counts the threads, in a neutral tone while nothing needs attention", () => {
    const threads = [makeThread({ status: "running" }), makeThread({ status: "ready" })];
    expect(hiddenCount(threads, [])).toEqual({ threads: 2, tone: "neutral", count: 2 });
  });

  it("waits when a thread needs input, counting only those", () => {
    const threads = [makeThread({ status: "needs_input" }), makeThread({ status: "blocked" }), makeThread()];
    expect(hiddenCount(threads, [])).toEqual({ threads: 3, tone: "wait", count: 1 });
  });

  it("counts pending permission requests as needing you too", () => {
    const threads = [makeThread({ status: "running" })];
    expect(hiddenCount(threads, [makeApproval()])).toEqual({ threads: 1, tone: "wait", count: 1 });
  });

  it("counts a session once when its request stands in for its needs-input thread", () => {
    const waiting = makeThread({ sessionId: "s-wait", status: "needs_input" });
    const other = makeThread({ status: "needs_input" });
    const approval = makeApproval({ sessionId: "s-wait" });
    expect(hiddenCount([waiting, other], [approval])).toEqual({ threads: 2, tone: "wait", count: 2 });
  });

  it("still counts a request whose session isn't on the cards", () => {
    expect(hiddenCount([], [makeApproval()])).toEqual({ threads: 0, tone: "wait", count: 1 });
  });

  it("shows blocked threads when nothing needs input", () => {
    const threads = [makeThread({ status: "blocked" }), makeThread({ status: "blocked" }), makeThread({ status: "ready" })];
    expect(hiddenCount(threads, [])).toEqual({ threads: 3, tone: "err", count: 2 });
  });
});

describe("CountPill", () => {
  const pill = (name: RegExp | string) => screen.getByRole("button", { name });

  it("says how many threads are hidden", () => {
    const { rerender } = render(<CountPill hidden={{ threads: 2, tone: "neutral", count: 2 }} onShow={() => {}} />);
    expect(pill("2 threads hidden. Show threads.")).toHaveTextContent(/^2 threads$/);
    rerender(<CountPill hidden={{ threads: 1, tone: "neutral", count: 1 }} onShow={() => {}} />);
    expect(pill("1 thread hidden. Show threads.")).toHaveTextContent(/^1 thread$/);
  });

  it("says how many need you, in amber with a dot", () => {
    const { rerender } = render(<CountPill hidden={{ threads: 2, tone: "wait", count: 1 }} onShow={() => {}} />);
    const button = pill("2 threads hidden, 1 needs you. Show threads.");
    expect(button).toHaveTextContent(/^1 needs you$/);
    expect(button).toHaveClass("is-wait");
    expect(button.querySelector(".count-pill-dot")).not.toBeNull();
    rerender(<CountPill hidden={{ threads: 3, tone: "wait", count: 2 }} onShow={() => {}} />);
    expect(pill("3 threads hidden, 2 need you. Show threads.")).toHaveTextContent(/^2 need you$/);
  });

  it("names a lone request without claiming hidden threads", () => {
    render(<CountPill hidden={{ threads: 0, tone: "wait", count: 1 }} onShow={() => {}} />);
    expect(pill("1 needs you. Show threads.")).toHaveTextContent(/^1 needs you$/);
  });

  it("says how many are blocked, in the error tone, like the cards' own status", () => {
    render(<CountPill hidden={{ threads: 3, tone: "err", count: 2 }} onShow={() => {}} />);
    const button = pill("3 threads hidden, 2 blocked. Show threads.");
    expect(button).toHaveTextContent(/^2 blocked$/);
    expect(button).toHaveClass("is-err");
  });

  it("is a collapsed disclosure the overlay can click through to", () => {
    render(<CountPill hidden={{ threads: 2, tone: "neutral", count: 2 }} onShow={() => {}} />);
    const button = pill(/Show threads/);
    expect(button.tagName).toBe("BUTTON");
    expect(button).toHaveAttribute("type", "button");
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(button).toHaveAttribute("data-hit");
    expect(button).toHaveClass("more-pill", "count-pill", "is-neutral");
    expect(button.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("shows the threads on click, Enter and Space, saying whether it held focus", async () => {
    const onShow = vi.fn();
    render(<CountPill hidden={{ threads: 2, tone: "neutral", count: 2 }} onShow={onShow} />);
    const button = pill(/Show threads/);
    button.focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    expect(onShow).toHaveBeenCalledTimes(2);
    expect(onShow).toHaveBeenLastCalledWith(true);
    button.blur();
    await userEvent.pointer({ keys: "[MouseLeft]", target: button });
    expect(onShow).toHaveBeenCalledTimes(3);
  });
});
