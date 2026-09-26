import type { Snapshot } from "../shared/types";

const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export function SetupChecks({ snap }: { snap: Snapshot }) {
  const { setup, config } = snap;
  const auth = setup.auth;
  const authText =
    auth == null ? "Login not checked yet" : auth.status === "allowed" ? `Logged in with Claude ${capitalize(auth.subscription)}` : auth.reason;
  const hooksClass = setup.hooksInstalled ? "good" : config.hooksDeclined ? "neutral" : "bad";
  const hooksText = setup.hooksInstalled
    ? "Watching your sessions"
    : config.hooksDeclined
      ? "Not watching sessions (Ask only)"
      : "Hooks not installed";
  return (
    <ul className="checks">
      <li className={setup.claudeError ? "bad" : "good"}>
        {setup.claudeError ?? `Claude Code ${setup.claudeVersion} found`}
      </li>
      <li className={auth?.status === "allowed" ? "good" : "bad"}>{authText}</li>
      <li className={hooksClass}>{hooksText}</li>
      {setup.hookServerError && <li className="bad">{setup.hookServerError}</li>}
    </ul>
  );
}
