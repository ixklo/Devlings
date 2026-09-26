import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../shared/api";
import type { Snapshot } from "../shared/types";
import { SetupChecks } from "./SetupChecks";

export function Settings({ snap, onBack }: { snap: Snapshot; onBack: () => void }) {
  const [name, setName] = useState(snap.config.petName);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const pet = snap.config.petName;

  const act = async (action: () => Promise<unknown>, ok?: string) => {
    setMessage(null);
    try {
      await action();
      if (ok) setMessage({ ok: true, text: ok });
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    }
  };

  const chooseBinary = async () => {
    const file = await open({ multiple: false, directory: false });
    if (typeof file === "string") await act(() => api.setClaudePath(file));
  };

  return (
    <div className="panel settings">
      <header className="panel-header">
        <button className="secondary" onClick={onBack}>
          Back
        </button>
        <h1>Settings</h1>
      </header>
      {message && (
        <p className={message.ok ? "notice" : "error"} role="status">
          {message.text}
        </p>
      )}

      <section>
        <h2>Pet</h2>
        <div className="row">
          <input aria-label="Pet name" value={name} maxLength={24} onChange={(e) => setName(e.target.value)} />
          <button onClick={() => act(() => api.setPetName(name), "Saved.")}>Save name</button>
        </div>
      </section>

      <section>
        <h2>Claude Code</h2>
        <SetupChecks snap={snap} />
        <div className="row">
          <button className="secondary" onClick={() => act(api.recheckSetup)}>
            Check again
          </button>
          <button className="secondary" onClick={chooseBinary}>
            Choose Claude Code file…
          </button>
          {snap.config.claudePath && (
            <button className="secondary" onClick={() => act(() => api.setClaudePath(null))}>
              Auto-detect
            </button>
          )}
        </div>
      </section>

      <section>
        <h2>Watching your sessions</h2>
        <p className="muted small">
          {pet} adds hooks to your Claude Code settings so it can see what your sessions are doing. They only talk to
          this computer.
        </p>
        <div className="row">
          {snap.setup.hooksInstalled ? (
            <button className="secondary" onClick={() => act(api.uninstallHooks, "Hooks removed.")}>
              Remove hooks
            </button>
          ) : (
            <button onClick={() => act(api.installHooks, "Hooks installed.")}>Install hooks</button>
          )}
          {snap.setup.hookServerError && (
            <button onClick={() => act(api.moveHooksPort, "Moved to a new port.")}>Move to a new port</button>
          )}
        </div>
      </section>

      <section>
        <h2>Notifications and startup</h2>
        <label className="check">
          <input
            type="checkbox"
            checked={snap.config.notifications}
            onChange={(e) => act(() => api.setNotifications(e.target.checked))}
          />
          Notify me when a session finishes or needs me
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={snap.config.launchAtLogin}
            onChange={(e) => act(() => api.setLaunchAtLogin(e.target.checked))}
          />
          Start {pet} when I log in
        </label>
      </section>

      <section>
        <h2>Cost</h2>
        <p className="muted small">
          Asks use your Claude subscription, never an API key. {pet} stops any run the moment it would use paid usage
          credits.
        </p>
        <button className="link" onClick={() => openUrl("https://claude.ai/settings/usage")}>
          Open your Claude usage settings
        </button>
      </section>

      <p className="muted small">Perch is free and open source, and is not affiliated with Anthropic.</p>
    </div>
  );
}
