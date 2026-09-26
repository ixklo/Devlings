import type { ThreadInfo, ThreadStatus } from "./types";

const PRIORITY: Record<ThreadStatus, number> = { needs_input: 0, blocked: 1, ready: 2, running: 3, idle: 4 };

/** Priority order from the spec (needs_input, blocked, ready, running), newest first within a status. */
export function sortThreads(threads: ThreadInfo[]): ThreadInfo[] {
  return [...threads].sort((a, b) => PRIORITY[a.status] - PRIORITY[b.status] || b.updatedAt - a.updatedAt);
}

export const MAX_BUBBLES = 3;

/** The cards to draw, in priority order, and how many sit behind the "+N more" pill. */
export function stackBubbles(threads: ThreadInfo[], expanded: boolean, max = MAX_BUBBLES) {
  const sorted = sortThreads(threads);
  if (expanded || sorted.length <= max) return { shown: sorted, more: 0 };
  return { shown: sorted.slice(0, max), more: sorted.length - max };
}

export const STATUS_TEXT: Record<ThreadStatus, string> = {
  running: "Working",
  needs_input: "Needs you",
  ready: "Done",
  blocked: "Blocked",
  idle: "Idle",
};

/** The one line under the project name on a card. */
export function threadLine(t: ThreadInfo): string {
  if (t.status === "ready" || t.status === "blocked") return t.excerpt ?? t.label ?? STATUS_TEXT[t.status];
  if (t.status === "needs_input") return t.label ?? "Waiting for your approval";
  return t.label ?? t.excerpt ?? (t.status === "running" ? "Working…" : "Idle");
}
