import { useState } from "react";
import { errorText } from "../shared/errors";
import { IconAlert, IconDownload } from "../shared/icons";
import type { UpdateStatus } from "../shared/types";

/**
 * The version the "Update ready" card should offer, if any. Never while an
 * Ask run is active (installing restarts Perch), and not for a version the
 * user already put off with Later.
 */
export function visibleUpdate(update: UpdateStatus, running: string[], dismissed: string | null): string | null {
  if (update.state !== "ready" || !update.version) return null;
  if (running.length > 0 || update.version === dismissed) return null;
  return update.version;
}

interface Props {
  version: string;
  tail?: boolean;
  onRestart: () => Promise<void>;
  onLater: () => void;
}

/** "Perch <version> is ready. Restart to install." with Restart and Later. */
export function UpdateCard({ version, tail, onRestart, onLater }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const restart = async () => {
    setBusy(true);
    setError(null);
    try {
      // On success Perch quits to install, so there's nothing to reset.
      await onRestart();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <div
      className={`card thread-card update-card${tail ? " has-tail" : ""}`}
      data-hit=""
      role="group"
      aria-label="Update ready"
    >
      <div className="update-card-main">
        <span className="status status-update" aria-hidden="true">
          <IconDownload size={16} />
        </span>
        <span className="thread-card-body">
          <span className="thread-card-top">
            <span className="thread-card-project">Perch {version} is ready.</span>
          </span>
          <span className="thread-card-line"> Restart to install.</span>
        </span>
      </div>
      {error && (
        <p className="inline-error update-card-error" role="alert">
          <IconAlert size={13} />
          <span>{error}</span>
        </p>
      )}
      <div className="update-card-actions">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onLater}>
          Later
        </button>
        <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => void restart()}>
          Restart
        </button>
      </div>
    </div>
  );
}
