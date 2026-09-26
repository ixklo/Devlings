import type { ReactNode } from "react";
import { IconAlert, IconCheck, IconInfo } from "../shared/icons";
import type { Snapshot } from "../shared/types";

const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

type Tone = "good" | "bad" | "neutral";

const ICONS: Record<Tone, ReactNode> = {
  good: <IconCheck size={12} strokeWidth={2.6} />,
  bad: <IconAlert size={14} />,
  neutral: <IconInfo size={14} />,
};

function Check({ tone, children }: { tone: Tone; children: string }) {
  return (
    <li className={tone}>
      <span className="check-icon" aria-hidden="true">
        {ICONS[tone]}
      </span>
      {children}
    </li>
  );
}

/** The Claude Code health list shared by onboarding and Settings. */
export function SetupChecks({ snap }: { snap: Snapshot }) {
  const { setup, config } = snap;
  const auth = setup.auth;
  const authText =
    auth == null
      ? "Login not checked yet"
      : auth.status === "allowed"
        ? `Logged in with Claude ${capitalize(auth.subscription)}`
        : auth.reason;
  const hooksTone: Tone = setup.hooksInstalled ? "good" : config.hooksDeclined ? "neutral" : "bad";
  const hooksText = setup.hooksInstalled
    ? "Watching your sessions"
    : config.hooksDeclined
      ? "Not watching sessions (Ask only)"
      : "Hooks not installed";
  return (
    <ul className="checks">
      <Check tone={setup.claudeError ? "bad" : "good"}>
        {setup.claudeError ?? `Claude Code ${setup.claudeVersion} found`}
      </Check>
      <Check tone={auth?.status === "allowed" ? "good" : auth == null ? "neutral" : "bad"}>{authText}</Check>
      <Check tone={hooksTone}>{hooksText}</Check>
      {setup.hookServerError && <Check tone="bad">{setup.hookServerError}</Check>}
    </ul>
  );
}
