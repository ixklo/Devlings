import { useLayoutEffect, useRef, type ReactNode } from "react";
import { IconAlert, IconChevronRight } from "../shared/icons";
import { stackCards } from "../shared/threads";
import type { PendingApproval, ThreadInfo } from "../shared/types";
import { ApprovalCard, ApprovalGhost, ApprovalsIntroCard } from "./ApprovalCard";
import { CountPill, type HiddenCount } from "./CountPill";
import { ThreadCard } from "./ThreadCard";
import { UpdateCard } from "./UpdateCard";
import { useApprovalGhosts, type ApprovalItem } from "./useApprovalGhosts";

interface SetupProps {
  detail: string;
  tail?: boolean;
  onOpen: () => void;
}

/** Shown instead of nothing when Devlings can't work yet; opens the settings window. */
export function SetupCard({ detail, tail, onOpen }: SetupProps) {
  return (
    <div className={`card thread-card setup-card${tail ? " has-tail" : ""}`} data-hit="" role="group" aria-label="Devlings: needs setup">
      <button type="button" className="thread-card-main" onClick={onOpen}>
        <span className="status status-setup" aria-hidden="true">
          <IconAlert size={16} />
        </span>
        <span className="thread-card-body">
          <span className="thread-card-top">
            <span className="thread-card-project">Finish setting up Devlings</span>
          </span>
          <span className="thread-card-line">{detail}</span>
        </span>
        <IconChevronRight className="setup-card-go" size={14} />
      </button>
    </div>
  );
}

const ENTER: Keyframe[] = [
  { opacity: 0, transform: "translateY(6px) scale(0.97)" },
  { opacity: 1, transform: "none" },
];

/**
 * Plays the entrance once, on mount. (A CSS animation would replay whenever
 * React moves the node, e.g. when a thread changes priority.)
 */
function BubbleSlot({ index, children }: { index: number; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el?.animate || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) return;
    el.animate(ENTER, { duration: 200, delay: index * 30, easing: "cubic-bezier(0.22, 1, 0.36, 1)", fill: "backwards" });
    // Mount only.
  }, []);
  return (
    <div className="bubble-slot" ref={ref}>
      {children}
    </div>
  );
}

interface Props {
  threads: ThreadInfo[];
  now: number;
  expanded: boolean;
  onToggleExpanded: () => void;
  onOpenThread: (thread: ThreadInfo) => void;
  setup?: { detail: string; onOpen: () => void } | null;
  /** A downloaded update waiting for a restart; shown above the threads. */
  update?: { version: string; onRestart: () => Promise<void>; onLater: () => void } | null;
  /** Permission requests the user can answer, oldest first; shown nearest the pet. */
  approvals?: PendingApproval[];
  /** The configured hold, for the approval cards' countdown bars. */
  holdMs?: number;
  /** Collapsed cards: approvals aren't drawn (and don't leave placeholders). */
  approvalsHidden?: boolean;
  /** Changes when the stage is laid out differently (flipped below the pet, shifted): re-arms approval buttons. */
  layoutKey?: unknown;
  /** The one-time "answer permission prompts" intro, shown at the top. */
  intro?: { petName: string } | null;
  /** Collapsed cards: the pill that stands in for them, in their place. */
  folded?: { hidden: HiddenCount; onShow: (hadFocus: boolean) => void } | null;
}

type Stacked = ApprovalItem & { sessionId: string };

const itemKey = (it: ApprovalItem) => `approval-${it.kind === "live" ? it.approval.id : it.id}`;

/**
 * Cards above the pet, nearest first: setup, permission requests, then threads
 * in priority order. The nearest card carries the tail; beyond three cards, a
 * "+N more" pill expands the stack. Collapsed, the requests and threads fold
 * into one count pill in their place.
 */
export function BubbleStack({
  threads,
  now,
  expanded,
  onToggleExpanded,
  onOpenThread,
  setup,
  update,
  approvals = [],
  holdMs = 60_000,
  approvalsHidden = false,
  layoutKey,
  intro,
  folded,
}: Props) {
  const ghosts = useApprovalGhosts(approvals);
  // A placeholder keeps a gone request's place (and never hides a thread card).
  const stackable: Stacked[] = approvalsHidden
    ? []
    : ghosts.items.map((it) => ({ ...it, sessionId: it.kind === "live" ? it.approval.sessionId : "" }));
  const shown = stackCards(stackable, threads, expanded);
  const { more } = shown;
  const collapsible = expanded && stackCards(stackable, threads, false).more > 0;
  const first = setup
    ? "setup"
    : shown.approvals.length
      ? "approval"
      : shown.threads.length
        ? "thread"
        : folded
          ? "folded"
          : update
            ? "update"
            : "intro";
  return (
    <div className="bubbles" role="region" aria-label="Claude Code sessions">
      {setup && <SetupCard detail={setup.detail} onOpen={setup.onOpen} tail />}
      {shown.approvals.map((it, i) => (
        // Same key for a card and the placeholder that replaces it, so the slot doesn't replay its entrance.
        <BubbleSlot key={itemKey(it)} index={i}>
          {it.kind === "live" ? (
            <ApprovalCard
              approval={it.approval}
              holdMs={holdMs}
              tail={first === "approval" && i === 0}
              layoutKey={layoutKey}
              onHeight={(h) => ghosts.onHeight(it.approval.id, h)}
              onAnswered={() => ghosts.onAnswered(it.approval.id)}
            />
          ) : (
            <ApprovalGhost text={it.text} height={it.height} tail={first === "approval" && i === 0} />
          )}
        </BubbleSlot>
      ))}
      {shown.threads.map((t, i) => (
        <BubbleSlot key={t.sessionId} index={shown.approvals.length + i}>
          <ThreadCard thread={t} now={now} tail={first === "thread" && i === 0} onOpenThread={onOpenThread} />
        </BubbleSlot>
      ))}
      {more > 0 && (
        <button type="button" className="more-pill" data-hit="" onClick={onToggleExpanded} aria-expanded={false}>
          +{more} more
        </button>
      )}
      {collapsible && (
        <button type="button" className="more-pill" data-hit="" onClick={onToggleExpanded} aria-expanded={true}>
          Show less
        </button>
      )}
      {folded && (
        <BubbleSlot key="count-pill" index={shown.approvals.length + shown.threads.length}>
          <CountPill hidden={folded.hidden} onShow={folded.onShow} />
        </BubbleSlot>
      )}
      {update && (
        <BubbleSlot key={`update-${update.version}`} index={shown.approvals.length + shown.threads.length}>
          <UpdateCard
            version={update.version}
            tail={first === "update"}
            onRestart={update.onRestart}
            onLater={update.onLater}
          />
        </BubbleSlot>
      )}
      {intro && (
        <BubbleSlot key="approvals-intro" index={shown.approvals.length + shown.threads.length + 1}>
          <ApprovalsIntroCard petName={intro.petName} tail={first === "intro"} />
        </BubbleSlot>
      )}
    </div>
  );
}
