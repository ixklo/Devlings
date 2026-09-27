import { useState } from "react";
import { errorText } from "../shared/errors";
import { IconAlert, IconShield, IconShieldCheck } from "../shared/icons";

interface Props {
  /** "This folder isn't trusted in Claude Code yet, so … ran without its project settings." */
  headline: string;
  /** "Skipped: hooks (…) · MCP servers (…) · …" */
  skipped?: string;
  petName: string;
  /** Trusted in Devlings right now (from the snapshot), so every notice in the chat agrees. */
  trusted: boolean;
  onTrust: () => Promise<unknown>;
  onUntrust: () => Promise<unknown>;
}

/**
 * The untrusted-folder notice in the mini chat (v1.0 D6): a system row, not Claude's text. It says what the run
 * skipped and offers to trust the folder in Devlings; once trusted, it offers to undo that.
 */
export function UntrustedNotice({ headline, skipped, petName, trusted, onTrust, onUntrust }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="msg-untrusted" role="group" aria-label="Untrusted folder">
      <p className="msg-untrusted-head">
        <IconShield size={14} />
        <span>{headline}</span>
      </p>
      {skipped && <p className="msg-untrusted-skipped">{skipped}</p>}
      {trusted ? (
        <p className="msg-untrusted-trusted">
          <IconShieldCheck size={14} />
          <span>Trusted in {petName}. The next Ask uses this folder's settings.</span>
          <button type="button" className="link" disabled={busy} onClick={() => void run(onUntrust)}>
            Stop trusting
          </button>
        </p>
      ) : (
        <div className="msg-untrusted-actions">
          <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => void run(onTrust)}>
            Trust this folder in {petName}
          </button>
        </div>
      )}
      {error && (
        <p className="inline-error" role="alert">
          <IconAlert size={13} />
          <span>{error}</span>
        </p>
      )}
    </div>
  );
}
