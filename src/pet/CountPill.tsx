import type { MouseEvent } from "react";
import { IconChevronUp } from "../shared/icons";
import type { PendingApproval, ThreadInfo } from "../shared/types";

/** What collapsing the cards hides: the thread count, and the one number worth the pill's tone. */
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

const threadsText = (n: number) => `${n} ${n === 1 ? "thread" : "threads"}`;

/** The words on the pill; "needs you" and "blocked" match the cards' own status text. */
function pillText({ tone, count }: HiddenCount): string {
  if (tone === "wait") return `${count} ${count === 1 ? "needs" : "need"} you`;
  if (tone === "err") return `${count} blocked`;
  return threadsText(count);
}

function pillLabel(hidden: HiddenCount): string {
  const parts = hidden.threads ? [`${threadsText(hidden.threads)} hidden`] : [];
  if (hidden.tone !== "neutral") parts.push(pillText(hidden));
  return `${parts.join(", ")}. Show threads.`;
}

interface Props {
  hidden: HiddenCount;
  /** Shows the threads; `hadFocus` says whether the pill held focus as it goes. */
  onShow: (hadFocus: boolean) => void;
}

/** Collapsed cards fold into this pill, in their place (never on the pet); clicking it shows them again. */
export function CountPill({ hidden, onShow }: Props) {
  return (
    <button
      type="button"
      className={`more-pill count-pill is-${hidden.tone}`}
      data-hit=""
      aria-expanded={false}
      aria-label={pillLabel(hidden)}
      title="Show threads"
      onClick={(e: MouseEvent<HTMLButtonElement>) => onShow(document.activeElement === e.currentTarget)}
    >
      {hidden.tone === "wait" && <span className="count-pill-dot" aria-hidden="true" />}
      {pillText(hidden)}
      <IconChevronUp size={13} strokeWidth={2.2} />
    </button>
  );
}
