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

/** Approval cards are tall; at most this many show before "+N more". */
export const MAX_APPROVAL_CARDS = 2;

/**
 * What the bubble stack draws: approvals first (oldest nearest the pet), then
 * threads in priority order, at most MAX_BUBBLES cards in all unless expanded.
 * A thread whose session has an approval card is left out; the card stands in for it.
 */
export function stackCards<A extends { sessionId: string }>(approvals: A[], threads: ThreadInfo[], expanded: boolean) {
  const waiting = new Set(approvals.map((a) => a.sessionId));
  const sorted = sortThreads(threads.filter((t) => !waiting.has(t.sessionId)));
  if (expanded) return { approvals, threads: sorted, more: 0 };
  const shownApprovals = approvals.slice(0, MAX_APPROVAL_CARDS);
  const shownThreads = sorted.slice(0, Math.max(0, MAX_BUBBLES - shownApprovals.length));
  return {
    approvals: shownApprovals,
    threads: shownThreads,
    more: approvals.length - shownApprovals.length + sorted.length - shownThreads.length,
  };
}

export const STATUS_TEXT: Record<ThreadStatus, string> = {
  running: "Working",
  needs_input: "Needs you",
  ready: "Done",
  blocked: "Blocked",
  idle: "Idle",
};

/** Flattens a Markdown snippet to one readable line for a card. */
export function plainLine(md: string): string {
  return md
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/`+([^`]*)`+/g, "$1")
    .replace(/(\*\*|__|\*|_|~~)(\S(?:.*?\S)?)\1/g, "$2")
    .split(/\r?\n/)
    .map((l) => l.replace(/^\s*(#{1,6}\s+|>\s*|[-*+]\s+|\d+[.)]\s+)/, "").trim())
    .filter(Boolean)
    .join(" ");
}

/** The one line under the project name on a card. */
export function threadLine(t: ThreadInfo): string {
  if (t.status === "ready" || t.status === "blocked") {
    return t.excerpt ? plainLine(t.excerpt) : (t.label ?? STATUS_TEXT[t.status]);
  }
  if (t.status === "needs_input") return t.label ?? "Waiting for your approval";
  return t.label ?? t.excerpt ?? (t.status === "running" ? "Working…" : "Idle");
}
