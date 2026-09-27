import type { PendingApproval, ThreadInfo } from "../shared/types";

/** What collapsing the cards hides: the thread count, and the one number worth the count's tone. */
export interface HiddenCount {
  threads: number;
  /** Needs input (threads or permission requests) > blocked > neutral. */
  tone: "neutral" | "wait" | "err";
  /** Threads (neutral), what needs you (wait) or blocked threads (err). */
  count: number;
}

/**
 * Null when collapsing hides nothing. A request stands in for its session's thread on the cards
 * (`stackCards`), so a session waiting on one counts once.
 */
export function hiddenCount(threads: ThreadInfo[], approvals: PendingApproval[]): HiddenCount | null {
  if (!threads.length && !approvals.length) return null;
  const asking = new Set(approvals.map((a) => a.sessionId));
  const needs = approvals.length + threads.filter((t) => t.status === "needs_input" && !asking.has(t.sessionId)).length;
  if (needs) return { threads: threads.length, tone: "wait", count: needs };
  const blocked = threads.filter((t) => t.status === "blocked").length;
  if (blocked) return { threads: threads.length, tone: "err", count: blocked };
  return { threads: threads.length, tone: "neutral", count: threads.length };
}

/**
 * The collapsed chevron's name and tooltip, e.g. "Show threads, 2 hidden, 1 needs you". "Needs you"
 * and "blocked" match the cards' own status text.
 */
export function showThreadsLabel({ threads, tone, count }: HiddenCount): string {
  const parts = ["Show threads"];
  if (threads) parts.push(`${threads} hidden`);
  if (tone === "wait") parts.push(`${count} ${count === 1 ? "needs" : "need"} you`);
  if (tone === "err") parts.push(`${count} blocked`);
  return parts.join(", ");
}
