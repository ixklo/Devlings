import { useState } from "react";
import { api } from "../shared/api";
import type { Snapshot } from "../shared/types";
import { SetupChecks } from "./SetupChecks";

export function Onboarding({ snap }: { snap: Snapshot }) {
  const [step, setStep] = useState(0);
  const [name, setName] = useState(snap.config.petName);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pet = snap.config.petName;

  const run = async (action: () => Promise<unknown>, next?: number) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      if (next !== undefined) setStep(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="panel onboarding">
      <p className="step">Step {step + 1} of 3</p>
      {step === 0 && (
        <>
          <h1>Name your pet</h1>
          <input
            autoFocus
            aria-label="Pet name"
            value={name}
            maxLength={24}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && run(() => api.setPetName(name), 1)}
          />
          <button disabled={busy} onClick={() => run(() => api.setPetName(name), 1)}>
            Next
          </button>
        </>
      )}
      {step === 1 && (
        <>
          <h1>Let {pet} watch your sessions?</h1>
          <p>
            {pet} adds a few hooks to your Claude Code settings file so it can see when a session is working, needs
            you, or is done. The hooks only talk to this computer. A backup of the file is saved first, and you can
            remove the hooks any time in Settings.
          </p>
          <div className="row">
            <button disabled={busy} onClick={() => run(api.installHooks, 2)}>
              Install hooks
            </button>
            <button disabled={busy} className="secondary" onClick={() => run(api.declineHooks, 2)}>
              Skip (Ask only)
            </button>
          </div>
        </>
      )}
      {step === 2 && (
        <>
          <h1>Checking Claude Code</h1>
          <SetupChecks snap={snap} />
          <div className="row">
            <button disabled={busy} className="secondary" onClick={() => run(api.recheckSetup)}>
              Check again
            </button>
            <button disabled={busy} onClick={() => run(api.finishOnboarding)}>
              Finish
            </button>
          </div>
        </>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
