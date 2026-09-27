import { describe, expect, it } from "vitest";
import { makeSnapshot, makeThread } from "../test/fixtures";
import { relativeTime } from "./time";
import { sortThreads, stackBubbles, threadForOpen, threadLine } from "./threads";
import type { ThreadInfo } from "./types";

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

describe("threadForOpen (a notification click, v1.0 S1)", () => {
  const snap = (threads: ThreadInfo[] = [], askSessionId: string | null = null) =>
    makeSnapshot({
      threads,
      projects: [
        {
          path: "C:\\code\\app",
          name: "app",
          lastSeen: 2,
          permissionMode: "edit_files",
          askSessionId,
          transcriptPath: null,
          trusted: false,
        },
      ],
    });

  it("uses the live thread when it's still listed, so the click does what its card does", () => {
    const live = makeThread({ sessionId: "w1", source: "watch", project: "C:\\code\\api", status: "ready" });
    expect(threadForOpen({ view: "thread", sessionId: "w1", project: "C:\\elsewhere", source: "ask" }, snap([live]))).toBe(live);
  });

  it("rebuilds a thread that has left the cards from the request", () => {
    const t = threadForOpen({ view: "thread", sessionId: "w2", project: "C:\\code\\api", source: "watch" }, snap());
    expect(t).toMatchObject({ sessionId: "w2", project: "C:\\code\\api", projectName: "api", source: "watch" });
    const ask = threadForOpen({ view: "thread", sessionId: "a1", project: "C:\\code\\app", source: "ask" }, snap());
    expect(ask).toMatchObject({ sessionId: "a1", project: "C:\\code\\app", source: "ask" });
  });

  it("falls back to the project whose Ask conversation it is when the request has no project", () => {
    expect(threadForOpen({ view: "thread", sessionId: "a1" }, snap([], "a1"))).toMatchObject({
      sessionId: "a1",
      project: "C:\\code\\app",
      source: "ask",
    });
  });

  it("keeps a Watch session without a folder, which the card only marks as seen", () => {
    expect(threadForOpen({ view: "thread", sessionId: "w3", project: "", source: "watch" }, snap())).toMatchObject({
      sessionId: "w3",
      project: "",
      source: "watch",
    });
  });

  it("gives up when there's nothing to open", () => {
    expect(threadForOpen({ view: "thread", sessionId: "gone" }, snap())).toBeNull();
    expect(threadForOpen({ view: "thread", sessionId: "a2", project: "", source: "ask" }, snap())).toBeNull();
    expect(threadForOpen({ view: "thread", sessionId: "x", project: "C:\\code\\app", source: "ask" }, null)).toMatchObject({
      sessionId: "x",
    });
  });
});
