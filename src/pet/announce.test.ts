import { describe, expect, it } from "vitest";
import { makeApproval, makeSetup, makeSnapshot, makeThread } from "../test/fixtures";
import { announcements } from "./announce";

describe("announcements (the pet window's live region)", () => {
  it("says nothing for the first snapshot, so launching Perch doesn't read out every card", () => {
    expect(announcements(null, makeSnapshot({ threads: [makeThread({ status: "needs_input" })] }))).toEqual([]);
  });

  it("announces a thread's status change once", () => {
    const t = makeThread({ sessionId: "s", projectName: "api-server", status: "running" });
    const a = makeSnapshot({ threads: [t] });
    const b = makeSnapshot({ threads: [{ ...t, status: "needs_input" }] });
    expect(announcements(a, b)).toEqual(["api-server: needs input"]);
    // The same status again (a new label, a new time) says nothing.
    const c = makeSnapshot({ threads: [{ ...t, status: "needs_input", label: "Still waiting", updatedAt: 9 }] });
    expect(announcements(b, c)).toEqual([]);
  });

  it("never reads streaming progress: label and excerpt changes while working say nothing", () => {
    const t = makeThread({ sessionId: "s", status: "running", label: "Reading a.ts" });
    const a = makeSnapshot({ threads: [t] });
    const b = makeSnapshot({ threads: [{ ...t, label: "Editing b.ts", excerpt: "Half a sentence" }] });
    expect(announcements(a, b)).toEqual([]);
  });

  it("announces a new session starting work, a finished Ask reply, a finished watched session and a blocked one", () => {
    const ask = makeThread({ sessionId: "ask", projectName: "app", source: "ask", status: "running" });
    const watch = makeThread({ sessionId: "w", projectName: "web", status: "running" });
    const bad = makeThread({ sessionId: "b", projectName: "db", status: "running" });
    const a = makeSnapshot({ threads: [ask, watch, bad] });
    const fresh = makeThread({ sessionId: "n", projectName: "docs", status: "running" });
    const b = makeSnapshot({
      threads: [{ ...ask, status: "ready" }, { ...watch, status: "ready" }, { ...bad, status: "blocked" }, fresh],
    });
    expect(announcements(a, b)).toEqual(["app: reply finished", "web: done", "db: blocked", "docs: working"]);
  });

  it("says nothing when a thread goes idle (viewed) or disappears", () => {
    const t = makeThread({ sessionId: "s", status: "ready" });
    const a = makeSnapshot({ threads: [t] });
    expect(announcements(a, makeSnapshot({ threads: [{ ...t, status: "idle" }] }))).toEqual([]);
    expect(announcements(a, makeSnapshot({ threads: [] }))).toEqual([]);
  });

  it("announces a new permission request instead of a bare 'needs input' for its session", () => {
    const t = makeThread({ sessionId: "s", projectName: "app", status: "running" });
    const req = makeApproval({ sessionId: "s", projectName: "app", toolName: "Bash" });
    const a = makeSnapshot({ threads: [t] });
    const b = makeSnapshot({ threads: [{ ...t, status: "needs_input" }], approvals: [req] });
    expect(announcements(a, b)).toEqual(["app: Bash permission request"]);
    // Still the same request: nothing new.
    expect(announcements(b, makeSnapshot({ threads: [{ ...t, status: "needs_input" }], approvals: [req] }))).toEqual([]);
  });

  it("announces an update once it's ready to install, once per version", () => {
    const a = makeSnapshot({ update: { state: "downloading", version: "1.0.1", progress: 90 } });
    const b = makeSnapshot({ update: { state: "ready", version: "1.0.1" } });
    expect(announcements(a, b)).toEqual(["Perch 1.0.1 is ready to install"]);
    expect(announcements(b, makeSnapshot({ update: { state: "ready", version: "1.0.1" } }))).toEqual([]);
    expect(announcements(b, makeSnapshot({ update: { state: "ready", version: "1.0.2" } }))).toEqual([
      "Perch 1.0.2 is ready to install",
    ]);
  });

  it("announces when Perch starts needing setup", () => {
    const a = makeSnapshot();
    const b = makeSnapshot({ setup: makeSetup({ needsSetup: true }) });
    expect(announcements(a, b)).toEqual(["Perch needs setup"]);
    expect(announcements(b, b)).toEqual([]);
  });
});
