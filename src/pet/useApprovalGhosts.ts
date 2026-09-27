import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { PendingApproval } from "../shared/types";

/** How long a removed request's place is kept, so the cards around it don't slide under the cursor. */
export const GHOST_MS = 1200;

export type ApprovalItem =
  | { kind: "live"; approval: PendingApproval }
  | { kind: "ghost"; id: string; text: string; height: number };

interface Ghost {
  id: string;
  text: string;
  height: number;
  index: number;
  until: number;
}

/**
 * The approvals to draw, with a short-lived placeholder ("Answered", or "No
 * longer waiting" when it expired or was answered in Claude Code) where a
 * request just left, the same height as its card.
 */
export function useApprovalGhosts(approvals: PendingApproval[]) {
  const heights = useRef(new Map<string, number>());
  const answered = useRef(new Set<string>());
  const prevIds = useRef<string[]>(approvals.map((a) => a.id));
  const [ghosts, setGhosts] = useState<Ghost[]>([]);
  const ids = approvals.map((a) => a.id).join("\n");

  useLayoutEffect(() => {
    const current = new Set(approvals.map((a) => a.id));
    const gone = prevIds.current.map((id, index) => ({ id, index })).filter((g) => !current.has(g.id));
    prevIds.current = approvals.map((a) => a.id);
    if (!gone.length) return;
    const until = Date.now() + GHOST_MS;
    setGhosts((old) => [
      ...old.filter((g) => !current.has(g.id)),
      ...gone.map((g) => ({
        id: g.id,
        index: g.index,
        height: heights.current.get(g.id) ?? 0,
        text: answered.current.has(g.id) ? "Answered" : "No longer waiting",
        until,
      })),
    ]);
    // Only the set of ids matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ids]);

  useEffect(() => {
    if (!ghosts.length) return;
    const wait = Math.max(0, Math.min(...ghosts.map((g) => g.until)) - Date.now());
    const timer = window.setTimeout(() => setGhosts((old) => old.filter((g) => g.until > Date.now())), wait);
    return () => window.clearTimeout(timer);
  }, [ghosts]);

  const items: ApprovalItem[] = approvals.map((approval) => ({ kind: "live", approval }));
  for (const g of [...ghosts].sort((a, b) => a.index - b.index)) {
    items.splice(Math.min(g.index, items.length), 0, { kind: "ghost", id: g.id, text: g.text, height: g.height });
  }

  const onHeight = useCallback((id: string, height: number) => {
    if (height > 0) heights.current.set(id, height);
  }, []);
  const onAnswered = useCallback((id: string) => {
    answered.current.add(id);
  }, []);
  return { items, onHeight, onAnswered };
}
