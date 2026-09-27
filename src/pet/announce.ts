import { useEffect, useRef, useState } from "react";
import type { Snapshot, ThreadStatus } from "../shared/types";

const SAID: Partial<Record<ThreadStatus, string>> = {
  running: "working",
  needs_input: "needs input",
  ready: "done",
  blocked: "blocked",
};

/**
 * What changed between two snapshots, in short phrases for the pet window's live region, e.g.
 * "api-server: needs input" or "Perch 1.0.1 is ready to install". Only status changes count, each
 * once: a label or excerpt changing while a session works (streaming) never does, and neither does
 * the first snapshot, so launching Perch doesn't read out every card.
 */
export function announcements(prev: Snapshot | null, next: Snapshot): string[] {
  if (!prev) return [];
  const out: string[] = [];
  if (next.setup.needsSetup && !prev.setup.needsSetup) out.push("Perch needs setup");

  const known = new Set(prev.approvals.map((a) => a.id));
  const asked = new Set<string>();
  for (const a of next.approvals) {
    if (known.has(a.id)) continue;
    out.push(`${a.projectName}: ${a.toolName} permission request`);
    asked.add(a.sessionId);
  }

  const before = new Map(prev.threads.map((t) => [t.sessionId, t.status]));
  for (const t of next.threads) {
    if (before.get(t.sessionId) === t.status) continue;
    // The permission request above already says why this session needs you.
    if (t.status === "needs_input" && asked.has(t.sessionId)) continue;
    const word = t.status === "ready" && t.source === "ask" ? "reply finished" : SAID[t.status];
    if (word) out.push(`${t.projectName}: ${word}`);
  }

  const u = next.update;
  const was = prev.update;
  if (u.state === "ready" && u.version && !(was.state === "ready" && was.version === u.version)) {
    out.push(`Perch ${u.version} is ready to install`);
  }
  return out;
}

/**
 * The text for the pet window's single polite live region. Each snapshot's changes are joined into
 * one message; a trailing no-break space alternates so the same words said twice in a row (a
 * session needing input again later) still count as a change for screen readers.
 */
export function useAnnouncements(snap: Snapshot | null): string {
  const prev = useRef<Snapshot | null>(null);
  const flip = useRef(false);
  const [text, setText] = useState("");
  useEffect(() => {
    if (!snap) return;
    const lines = announcements(prev.current, snap);
    prev.current = snap;
    if (!lines.length) return;
    flip.current = !flip.current;
    setText(lines.join(". ") + (flip.current ? " " : ""));
  }, [snap]);
  return text;
}
