import { useId, useLayoutEffect, useRef, useState, type MouseEvent } from "react";
import { api } from "../shared/api";
import { IconAlert, IconShield } from "../shared/icons";
import type { ApprovalDecision, PendingApproval, Snapshot } from "../shared/types";
import { useArming } from "./useArming";

interface Props {
  approval: PendingApproval;
  /** The configured hold, so the countdown bar starts at the right width. */
  holdMs: number;
  /** "card" in the bubble stack, "row" inline in the mini chat. */
  variant?: "card" | "row";
  /** The card nearest the pet gets the speech-bubble tail. */
  tail?: boolean;
  /** Changes when the cards around it are laid out differently (e.g. flipped below the pet): re-arms. */
  layoutKey?: unknown;
  /** Reports the card's height, for the placeholder that keeps its place once it's gone. */
  onHeight?: (height: number) => void;
  /** Called once this card's answer went through. */
  onAnswered?: () => void;
}

/** What a request that can only be denied says, per source. */
export function tooLongText(a: PendingApproval): string {
  return a.source === "ask" ? "Too long to review here, so it can only be denied." : "Too long to review here. Answer in Claude Code.";
}

/** A thin bar that empties as a watched request's hold runs out. */
function Countdown({ expiresAt, holdMs }: { expiresAt: number; holdMs: number }) {
  // Measured once: the CSS animation carries it from here to empty.
  const [start] = useState(() => Date.now());
  const remaining = Math.max(0, expiresAt - start);
  const from = holdMs > 0 ? Math.min(1, remaining / holdMs) : 0;
  return (
    <div className="approval-countdown" aria-hidden="true">
      <span className="approval-countdown-bar" style={{ transform: `scaleX(${from})`, animationDuration: `${remaining}ms` }} />
    </div>
  );
}

/**
 * A Claude Code permission request: project, tool, the exact command/file/URL,
 * Claude's description and the whole input under Details (open by default when
 * the headline leaves something out), with Deny / Allow and, when Claude Code
 * offered a rule, the always button. The buttons only answer once the card has
 * sat still for a moment (see `useArming`); nothing is focused on mount and no
 * key answers on its own. A request too long to show can only be denied.
 */
export function ApprovalCard({ approval: a, holdMs, variant = "card", tail, layoutKey, onHeight, onAnswered }: Props) {
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState(a.lossy);
  const detailId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const arm = useArming<HTMLDivElement>(layoutKey);

  useLayoutEffect(() => {
    const h = rootRef.current?.getBoundingClientRect().height ?? 0;
    onHeight?.(h);
  });

  const answer = async (e: MouseEvent, decision: ApprovalDecision) => {
    if (!arm.accept(e) || busy) return;
    setBusy(true);
    try {
      // On success the snapshot drops the request; the first answer wins.
      await api.answerApproval(a.id, decision);
      onAnswered?.();
    } catch {
      // Already answered elsewhere, or no longer waiting: harmless.
      setBusy(false);
    }
  };

  const card = variant === "card";
  const inert = !arm.armed || busy;
  const offerAlways = a.canAlwaysAllow && !!a.alwaysLabel && !a.tooLong;
  const button = (decision: ApprovalDecision, label: string, className: string, extra: Record<string, string | undefined> = {}) => (
    <button
      type="button"
      className={`btn btn-sm ${className}`}
      disabled={busy}
      aria-disabled={inert}
      onPointerDown={arm.onPointerDown}
      onClick={(e) => void answer(e, decision)}
      {...extra}
    >
      {label}
    </button>
  );

  return (
    <div
      ref={rootRef}
      className={card ? `card thread-card approval-card${tail ? " has-tail" : ""}` : "approval-row"}
      data-hit=""
      data-armed={arm.armed}
      role="group"
      aria-label={`${a.toolName} request in ${a.projectName}`}
    >
      <div className="approval-head">
        <span className="status status-approval" aria-hidden="true">
          <IconShield size={15} />
        </span>
        {card && <span className="thread-card-project">{a.projectName}</span>}
        <span className="approval-tool">{a.toolName}</span>
        {card && a.source === "ask" && <span className="thread-card-source">Ask</span>}
      </div>
      {a.rawToolName && (
        <p className="approval-raw" title={a.rawToolName}>
          {a.rawToolName}
        </p>
      )}
      {a.risks.length > 0 && (
        <p className="approval-risks">
          {a.risks.map((r) => (
            <span key={r} className="approval-risk">
              <IconAlert size={12} />
              {r}
            </span>
          ))}
        </p>
      )}
      {a.summary && <code className="approval-summary">{a.summary}</code>}
      {a.description && <p className="approval-desc">{a.description}</p>}
      <details className="approval-details" open={open} onToggle={(e) => setOpen(e.currentTarget.open)}>
        <summary>Details</summary>
        <pre className="approval-details-body">{a.details}</pre>
      </details>
      {a.tooLong && <p className="approval-too-long">{tooLongText(a)}</p>}
      <div ref={arm.ref} className="approval-actions">
        {button("deny", "Deny", "btn-secondary")}
        {offerAlways &&
          button("always", a.alwaysLabel ?? "Always allow", "btn-ghost approval-always", {
            title: a.alwaysDetail ?? undefined,
            "aria-describedby": a.alwaysDetail ? detailId : undefined,
          })}
        {!a.tooLong && button("allow", "Allow", "btn-primary")}
      </div>
      {offerAlways && a.alwaysDetail && (
        <p id={detailId} className="approval-always-detail">
          {a.alwaysLabel}: {a.alwaysDetail}
        </p>
      )}
      {a.source === "watch" && a.expiresAt !== null && <Countdown expiresAt={a.expiresAt} holdMs={holdMs} />}
    </div>
  );
}

/** Keeps a removed request's place for a moment, so the cards around it don't slide under the cursor. */
export function ApprovalGhost({ text, height, variant = "card", tail }: { text: string; height: number; variant?: "card" | "row"; tail?: boolean }) {
  return (
    <div
      className={variant === "card" ? `card thread-card approval-ghost${tail ? " has-tail" : ""}` : "approval-row approval-ghost"}
      data-hit=""
      style={height > 0 ? { minHeight: height } : undefined}
      role="status"
    >
      {text}
    </div>
  );
}

/** The one-time intro after upgrading: onboarding already finished, hooks installed, not answered yet, not already on. */
export function showApprovalsIntro(snap: Snapshot): boolean {
  const c = snap.config;
  return !c.approvalsIntroSeen && !c.watchApprovals && c.onboarded && snap.setup.hooksInstalled;
}

/** "{pet} can answer permission prompts…" with Turn on and Not now. */
export function ApprovalsIntroCard({ petName, tail }: { petName: string; tail?: boolean }) {
  const [busy, setBusy] = useState(false);
  const run = async (turnOn: boolean) => {
    setBusy(true);
    try {
      if (turnOn) await api.setWatchApprovals(true);
      await api.markApprovalsIntroSeen();
    } catch {
      setBusy(false);
    }
  };
  return (
    <div
      className={`card thread-card approval-intro${tail ? " has-tail" : ""}`}
      data-hit=""
      role="group"
      aria-label="Answer permission prompts"
    >
      <div className="approval-intro-main">
        <span className="status status-approval" aria-hidden="true">
          <IconShield size={16} />
        </span>
        <p className="approval-intro-text">
          {petName} can answer permission prompts. When a Claude Code session asks to run something, you can Allow or
          Deny it right here. Claude Code's own prompt still works too.
        </p>
      </div>
      <div className="approval-actions">
        <button type="button" className="btn btn-ghost btn-sm" disabled={busy} onClick={() => void run(false)}>
          Not now
        </button>
        <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => void run(true)}>
          Turn on
        </button>
      </div>
    </div>
  );
}
