import { describe, expect, it } from "vitest";
import { makeSnapshot, makeThread } from "../test/fixtures";
import { relativeTime } from "./time";
import { sortThreads, stackBubbles, stackCards, threadForOpen, threadLine } from "./threads";
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

describe("stackCards", () => {
  const threads = Array.from({ length: 4 }, (_, i) => makeThread({ sessionId: `t${i}`, updatedAt: 10 - i }));
  const approval = (sessionId: string) => ({ sessionId, id: `a-${sessionId}` });

  it("matches stackBubbles without approvals", () => {
    const { approvals, threads: shown, more } = stackCards([], threads, false);
    expect(approvals).toEqual([]);
    expect(shown.map((t) => t.sessionId)).toEqual(stackBubbles(threads, false).shown.map((t) => t.sessionId));
    expect(more).toBe(1);
  });

  it("puts approvals first, caps them at two and fills the rest with threads", () => {
    const r = stackCards([approval("x"), approval("y"), approval("z")], threads, false);
    expect(r.approvals.map((a) => a.id)).toEqual(["a-x", "a-y"]);
    expect(r.threads.map((t) => t.sessionId)).toEqual(["t0"]);
    expect(r.more).toBe(1 + 3);
    const one = stackCards([approval("x")], threads, false);
    expect(one.threads.map((t) => t.sessionId)).toEqual(["t0", "t1"]);
  });

  it("leaves out threads whose session has an approval card", () => {
    const r = stackCards([approval("t0")], threads, true);
    expect(r.threads.map((t) => t.sessionId)).toEqual(["t1", "t2", "t3"]);
    expect(r.more).toBe(0);
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

  it("leaves identifiers, file names and arithmetic alone", () => {
    const line = (excerpt: string) => threadLine(makeThread({ status: "ready", excerpt }));
    expect(line("Renamed `get_user_id` to `fetch_user_id`.")).toBe("Renamed get_user_id to fetch_user_id.");
    expect(line("Fixed src/my_file_name.rs and 2*3*4 math")).toBe("Fixed src/my_file_name.rs and 2*3*4 math");
    expect(line("**Done.** Tests pass in `snake_case_module`, *all* of them")).toBe(
      "Done. Tests pass in snake_case_module, all of them",
    );
    expect(line("Kept `**kwargs` and `a_b_c` as they were")).toBe("Kept **kwargs and a_b_c as they were");
    expect(line("a * b * c, _emphasis_ and ~~gone~~")).toBe("a * b * c, emphasis and gone");
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
