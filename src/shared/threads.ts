import { projectName } from "./paths";
import type { Snapshot, ThreadInfo, ThreadOpen, ThreadStatus } from "./types";

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

/**
 * The thread a `pet-open {view: "thread"}` request (a notification click) is about: the listed one if it's still on
 * the cards, else one rebuilt from the request, so the click can do exactly what the card would. Null when there's
 * nothing to open.
 */
export function threadForOpen(o: ThreadOpen, snap: Snapshot | null): ThreadInfo | null {
  const live = snap?.threads.find((t) => t.sessionId === o.sessionId);
  if (live) return live;
  const askProject = snap?.projects.find((p) => p.askSessionId === o.sessionId)?.path;
  const source = o.source ?? (askProject !== undefined ? "ask" : undefined);
  const project = o.project ?? askProject;
  if (source === undefined || project === undefined) return null;
  // An Ask chat needs its folder; a Watch session without one is still marked seen.
  if (source === "ask" && !project) return null;
  return {
    sessionId: o.sessionId,
    project,
    projectName: project ? projectName(project) : "Claude Code",
    source,
    status: "idle",
    label: null,
    excerpt: null,
    updatedAt: 0,
    unread: false,
  };
}
