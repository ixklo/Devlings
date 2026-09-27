import { describe, expect, it } from "vitest";
import { makeApproval, makeThread } from "../test/fixtures";
import { hiddenCount, showThreadsLabel } from "./hiddenCount";

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

describe("showThreadsLabel", () => {
  it("says how many threads are hidden", () => {
    expect(showThreadsLabel({ threads: 2, tone: "neutral", count: 2 })).toBe("Show threads, 2 hidden");
    expect(showThreadsLabel({ threads: 1, tone: "neutral", count: 1 })).toBe("Show threads, 1 hidden");
  });

  it("adds how many need you, in the cards' own words", () => {
    expect(showThreadsLabel({ threads: 2, tone: "wait", count: 1 })).toBe("Show threads, 2 hidden, 1 needs you");
    expect(showThreadsLabel({ threads: 3, tone: "wait", count: 2 })).toBe("Show threads, 3 hidden, 2 need you");
  });

  it("names a lone request without claiming hidden threads", () => {
    expect(showThreadsLabel({ threads: 0, tone: "wait", count: 1 })).toBe("Show threads, 1 needs you");
  });

  it("adds how many are blocked", () => {
    expect(showThreadsLabel({ threads: 3, tone: "err", count: 2 })).toBe("Show threads, 3 hidden, 2 blocked");
  });
});
