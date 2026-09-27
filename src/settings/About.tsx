import { useEffect, useRef, useState } from "react";
import { api } from "../shared/api";
import { IconCheck, IconCopy, IconExternal, IconFolder, IconGithub, IconRefresh } from "../shared/icons";
import type { Snapshot, UpdateStatus } from "../shared/types";
import { Section, SwitchRow } from "./controls";
import type { useAction } from "./useAction";

export const REPO_URL = "https://github.com/yeetstick/perch";
export const LICENSES_URL = `${REPO_URL}/blob/main/THIRD_PARTY_NOTICES.md`;
const COPIED_MS = 2000;

/** The inline status next to "Check for updates"; null when there's nothing to say. */
export function updateLine(u: UpdateStatus, upToDate: boolean): string | null {
  switch (u.state) {
    case "checking":
      return "Checking for updates…";
    case "available":
      return `Perch ${u.version} is available. Downloading…`;
    case "downloading":
      return `Downloading Perch ${u.version}…${u.progress == null ? "" : ` ${u.progress}%`}`;
    case "ready":
      return `Perch ${u.version} is ready. Restart to install.`;
    case "error":
      return u.error ?? "Couldn't check for updates.";
    case "disabled":
      return u.error ?? "Updates are off in this build.";
    case "idle":
      return upToDate ? "Perch is up to date." : null;
  }
}

const statusKey = (u: UpdateStatus) => `${u.state}|${u.version ?? ""}|${u.error ?? ""}`;

interface Props {
  snap: Snapshot;
  action: ReturnType<typeof useAction>;
}

/** Settings → About: version, updates, pets folder, diagnostics and licenses. */
export function About({ snap, action }: Props) {
  const { config, update } = snap;
  const [version, setVersion] = useState<string | null>(null);
  // The status a manual check ended on when it found nothing new. "Up to date" shows only while the
  // live status is still that one; once the status has matched it and moved on, it's dropped.
  const [upToDate, setUpToDate] = useState<{ key: string; seen: boolean } | null>(null);
  const [copied, setCopied] = useState(false);
  const copiedTimer = useRef<number | undefined>(undefined);
  const key = statusKey(update);
  const keyRef = useRef(key);
  keyRef.current = key;

  useEffect(() => {
    api.appVersion().then(setVersion, () => setVersion(null));
    return () => window.clearTimeout(copiedTimer.current);
  }, []);

  // The check's result and its snapshot can arrive in either order, so wait to see the status first.
  useEffect(() => {
    setUpToDate((u) => {
      if (!u) return u;
      if (key === u.key) return u.seen ? u : { ...u, seen: true };
      return u.seen ? null : u;
    });
  }, [key]);

  const busy = update.state === "checking" || update.state === "available" || update.state === "downloading";
  const line = updateLine(update, upToDate?.key === key);

  const check = () =>
    void action.run(async () => {
      setUpToDate(null);
      const result = await api.checkForUpdate();
      if (result.state === "idle") {
        const resultKey = statusKey(result);
        setUpToDate({ key: resultKey, seen: keyRef.current === resultKey });
      }
    });

  const copyDiagnostics = () =>
    void action.run(async () => {
      const text = await api.getDiagnostics();
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.clearTimeout(copiedTimer.current);
      copiedTimer.current = window.setTimeout(() => setCopied(false), COPIED_MS);
    });

  return (
    <Section title="About">
      <div className="row">
        <div className="row-text">
          <span className="row-label">Perch {version ? `v${version}` : ""}</span>
          <p className="row-desc">Free and open source. Not affiliated with Anthropic.</p>
        </div>
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => void api.openUrl(REPO_URL)}>
          <IconGithub size={13} />
          GitHub
        </button>
      </div>
      <SwitchRow
        label="Check for updates automatically"
        description="At launch and once a day. Updates download in the background and install when you restart."
        checked={config.autoUpdate}
        onChange={(v) => void action.run(() => api.setAutoUpdate(v))}
      />
      <div className="row">
        <div className="row-text">
          <span className="row-label">Updates</span>
          <p className={`row-desc${update.state === "error" ? " is-error" : ""}`} aria-live="polite">
            {line ?? "Perch checks GitHub Releases for new versions."}
          </p>
        </div>
        {update.state === "ready" ? (
          <button
            type="button"
            className="btn btn-primary btn-sm"
            disabled={action.busy}
            onClick={() => void action.run(api.installUpdate)}
          >
            Restart to update
          </button>
        ) : (
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            disabled={action.busy || busy || update.state === "disabled"}
            onClick={check}
          >
            <IconRefresh size={13} />
            Check for updates
          </button>
        )}
      </div>
      <div className="row">
        <div className="row-text">
          <span className="row-label">More pets</span>
          <p className="row-desc">
            Pet format compatible with Codex pets; drop pets into <code>~/.codex/pets</code> or Perch's pets folder.
          </p>
        </div>
        <button type="button" className="btn btn-secondary btn-sm" onClick={() => void action.run(api.openPetsFolder)}>
          <IconFolder size={13} />
          Open folder
        </button>
      </div>
      <div className="row is-block">
        <div className="row-text">
          <span className="row-label">Diagnostics</span>
          <p className="row-desc">A redacted summary to paste into a bug report. Logs stay on this computer.</p>
        </div>
        <div className="actions">
          <button type="button" className="btn btn-secondary btn-sm" onClick={copyDiagnostics}>
            {copied ? <IconCheck size={13} strokeWidth={2.4} /> : <IconCopy size={13} />}
            {copied ? "Copied" : "Copy diagnostics"}
          </button>
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => void action.run(api.openLogFolder)}>
            <IconFolder size={13} />
            Open log folder
          </button>
        </div>
      </div>
      <div className="row">
        <button type="button" className="link" onClick={() => void api.openUrl(LICENSES_URL)}>
          Third-party licenses
          <IconExternal size={12} />
        </button>
      </div>
    </Section>
  );
}
