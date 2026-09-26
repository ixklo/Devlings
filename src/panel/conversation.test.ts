import { describe, expect, it } from "vitest";
import type { ChatTurn, PetEvent } from "../shared/types";
import { applyAskEvent } from "./conversation";

const ev = (kind: PetEvent["kind"], extra: Partial<PetEvent> = {}): PetEvent => ({
  sessionId: "s",
  project: "p",
  source: "ask",
  kind,
  at: 0,
  ...extra,
});

const run = (events: PetEvent[], start: ChatTurn[] = [{ role: "user", text: "hi" }]) =>
  events.reduce(applyAskEvent, start);

describe("applyAskEvent", () => {
  it("streams deltas into one pending turn and finalizes on done", () => {
    const mid = run([ev("reply_delta", { text: "ban" }), ev("reply_delta", { text: "ana" })]);
    expect(mid).toEqual([{ role: "user", text: "hi" }, { role: "assistant", text: "banana", pending: true }]);
    const end = applyAskEvent(mid, ev("done", { label: "Done", text: "banana" }));
    expect(end[1]).toEqual({ role: "assistant", text: "banana" });
  });

  it("uses the final text when no deltas arrived", () => {
    expect(run([ev("done", { text: "ok" })])[1]).toEqual({ role: "assistant", text: "ok" });
  });

  it("starts a new assistant turn after a tool step", () => {
    const t = run([
      ev("reply_delta", { text: "Let me look." }),
      ev("step", { label: "Reading a.ts" }),
      ev("reply_delta", { text: "Found it." }),
      ev("done", { text: "Found it." }),
    ]);
    expect(t.map((x) => x.text)).toEqual(["hi", "Let me look.", "Found it."]);
    expect(t.every((x) => !x.pending)).toBe(true);
  });

  it("adds notes for blocked, failed, and stopped", () => {
    const t = run([
      ev("blocked", { label: "Blocked: Write" }),
      ev("failed", { label: "Plan limit reached", text: "Resets in 1h 0m." }),
      ev("ended", { label: "Stopped" }),
    ]);
    expect(t.slice(1)).toEqual([
      { role: "note", text: "Blocked: Write" },
      { role: "note", text: "Plan limit reached — Resets in 1h 0m." },
      { role: "note", text: "Stopped" },
    ]);
  });

  it("ignores events that don't change the conversation", () => {
    const start: ChatTurn[] = [{ role: "user", text: "hi" }];
    expect(run([ev("started"), ev("prompt"), ev("needs_you")], start)).toBe(start);
  });
});
