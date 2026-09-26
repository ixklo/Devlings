import { describe, expect, it } from "vitest";
import { makeThread } from "../test/fixtures";
import { relativeTime } from "./time";
import { sortThreads, stackBubbles, threadLine } from "./threads";

describe("sortThreads", () => {
  it("orders by needs_input, blocked, ready, running, then newest", () => {
    const threads = [
      makeThread({ sessionId: "run-old", status: "running", updatedAt: 1 }),
      makeThread({ sessionId: "ready", status: "ready", updatedAt: 5 }),
      makeThread({ sessionId: "run-new", status: "running", updatedAt: 9 }),
      makeThread({ sessionId: "needs", status: "needs_input", updatedAt: 2 }),
      makeThread({ sessionId: "blocked", status: "blocked", updatedAt: 3 }),
    ];
    expect(sortThreads(threads).map((t) => t.sessionId)).toEqual(["needs", "blocked", "ready", "run-new", "run-old"]);
  });
});

describe("stackBubbles", () => {
  const five = Array.from({ length: 5 }, (_, i) => makeThread({ sessionId: `t${i}`, updatedAt: 10 - i }));

  it("shows three and counts the rest", () => {
    const { shown, more } = stackBubbles(five, false);
    expect(shown.map((t) => t.sessionId)).toEqual(["t0", "t1", "t2"]);
    expect(more).toBe(2);
  });

  it("shows everything when expanded, or when there are three or fewer", () => {
    expect(stackBubbles(five, true)).toMatchObject({ more: 0 });
    expect(stackBubbles(five, true).shown).toHaveLength(5);
    expect(stackBubbles(five.slice(0, 3), false)).toMatchObject({ more: 0 });
  });
});

describe("threadLine", () => {
  it("prefers the excerpt for finished threads and the label for live ones", () => {
    expect(threadLine(makeThread({ status: "ready", label: "Done", excerpt: "Fixed it" }))).toBe("Fixed it");
    expect(threadLine(makeThread({ status: "running", label: "Editing a.ts", excerpt: "old" }))).toBe("Editing a.ts");
    expect(threadLine(makeThread({ status: "needs_input", label: null }))).toBe("Waiting for your approval");
  });

  it("shows Markdown excerpts as plain text", () => {
    const excerpt = "1. **Say what it holds:** `unpaidInvoices` beats\n## Heading\n- see [the docs](https://x.y)";
    expect(threadLine(makeThread({ status: "ready", excerpt }))).toBe(
      "Say what it holds: unpaidInvoices beats Heading see the docs",
    );
  });
});

describe("relativeTime", () => {
  it("is compact", () => {
    const now = 10 * 24 * 3_600_000;
    expect(relativeTime(now - 10_000, now)).toBe("now");
    expect(relativeTime(now - 2 * 60_000, now)).toBe("2m");
    expect(relativeTime(now - 3 * 3_600_000, now)).toBe("3h");
    expect(relativeTime(now - 2 * 86_400_000, now)).toBe("2d");
  });
});
