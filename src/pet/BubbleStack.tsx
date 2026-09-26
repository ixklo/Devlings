import { useLayoutEffect, useRef, type ReactNode } from "react";
import { IconAlert, IconChevronRight } from "../shared/icons";
import { MAX_BUBBLES, stackBubbles } from "../shared/threads";
import type { ThreadInfo } from "../shared/types";
import { ThreadCard } from "./ThreadCard";

interface SetupProps {
  detail: string;
  tail?: boolean;
  onOpen: () => void;
}

/** Shown instead of nothing when Perch can't work yet; opens the settings window. */
export function SetupCard({ detail, tail, onOpen }: SetupProps) {
  return (
    <div className={`card thread-card setup-card${tail ? " has-tail" : ""}`} data-hit="">
      <button type="button" className="thread-card-main" onClick={onOpen}>
        <span className="status status-setup" aria-hidden="true">
          <IconAlert size={16} />
        </span>
        <span className="thread-card-body">
          <span className="thread-card-top">
            <span className="thread-card-project">Finish setting up Perch</span>
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
}

/**
 * Thread cards above the pet. The first card in priority order sits nearest
 * the pet and carries the tail; beyond three, a "+N more" pill expands the stack.
 */
export function BubbleStack({ threads, now, expanded, onToggleExpanded, onOpenThread, setup }: Props) {
  const { shown, more } = stackBubbles(threads, expanded);
  const collapsible = expanded && threads.length > MAX_BUBBLES;
  return (
    <div className="bubbles" aria-label="Claude Code threads">
      {setup && <SetupCard detail={setup.detail} onOpen={setup.onOpen} tail />}
      {shown.map((t, i) => (
        <BubbleSlot key={t.sessionId} index={i}>
          <ThreadCard thread={t} now={now} tail={!setup && i === 0} onOpenThread={onOpenThread} />
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
    </div>
  );
}
