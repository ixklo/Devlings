import { useId, useState } from "react";
import { api } from "../shared/api";
import { IconShield } from "../shared/icons";
import type { ApprovalDecision, PendingApproval, Snapshot } from "../shared/types";

interface Props {
  approval: PendingApproval;
  /** The configured hold, so the countdown bar starts at the right width. */
  holdMs: number;
  /** "card" in the bubble stack, "row" inline in the mini chat. */
  variant?: "card" | "row";
  /** The card nearest the pet gets the speech-bubble tail. */
  tail?: boolean;
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
 * A Claude Code permission request: project, tool, the exact command/file/URL
 * and Claude's description, with Deny / Allow and, when Claude Code offered a
 * rule, the always button. Nothing is focused on mount and no key answers:
 * only a click (or a deliberately focused button) does.
 */
export function ApprovalCard({ approval: a, holdMs, variant = "card", tail }: Props) {
  const [busy, setBusy] = useState(false);
  const detailId = useId();

  const answer = async (decision: ApprovalDecision) => {
    if (busy) return;
    setBusy(true);
    try {
      // On success the snapshot drops the request; the first answer wins.
      await api.answerApproval(a.id, decision);
    } catch {
      // Already answered elsewhere, or no longer waiting: harmless.
      setBusy(false);
    }
  };

  const card = variant === "card";
  return (
    <div
      className={card ? `card thread-card approval-card${tail ? " has-tail" : ""}` : "approval-row"}
      data-hit=""
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
      {a.summary && <code className="approval-summary">{a.summary}</code>}
      {a.description && <p className="approval-desc">{a.description}</p>}
      <div className="approval-actions">
        <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => void answer("deny")}>
          Deny
        </button>
        {a.canAlwaysAllow && a.alwaysLabel && (
          <>
            <button
              type="button"
              className="btn btn-ghost btn-sm approval-always"
              disabled={busy}
              title={a.alwaysDetail ?? undefined}
              aria-describedby={a.alwaysDetail ? detailId : undefined}
              onClick={() => void answer("always")}
            >
              {a.alwaysLabel}
            </button>
            {a.alwaysDetail && (
              <span id={detailId} className="sr-only">
                {a.alwaysDetail}
              </span>
            )}
          </>
        )}
        <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => void answer("allow")}>
          Allow
        </button>
      </div>
      {a.source === "watch" && a.expiresAt !== null && <Countdown expiresAt={a.expiresAt} holdMs={holdMs} />}
    </div>
  );
}

/** The one-time intro after upgrading: onboarding already finished, hooks installed, not answered yet. */
export function showApprovalsIntro(snap: Snapshot): boolean {
  return !snap.config.approvalsIntroSeen && snap.config.onboarded && snap.setup.hooksInstalled;
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
