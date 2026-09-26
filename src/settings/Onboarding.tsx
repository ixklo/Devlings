import { useState } from "react";
import { api } from "../shared/api";
import { IconChevronLeft, IconRefresh } from "../shared/icons";
import type { Snapshot } from "../shared/types";
import { NAME_MAX } from "./NameField";
import { NoticeBar } from "./NoticeBar";
import { PetPicker } from "./PetPicker";
import { SetupChecks } from "./SetupChecks";
import { useAction } from "./useAction";

const STEPS = ["Pet", "Watch", "Check"] as const;

/** First run: choose and name the pet, offer the hooks, check Claude Code. */
export function Onboarding({ snap, onDone }: { snap: Snapshot; onDone: () => void }) {
  const [step, setStep] = useState(0);
  const [name, setName] = useState(snap.config.petName);
  const action = useAction();
  const pet = snap.config.petName;

  const next = async (work: () => Promise<unknown>, to: number) => {
    if (await action.run(work)) setStep(to);
  };

  const saveName = () => {
    const trimmed = name.trim();
    if (!trimmed) return Promise.reject("Give your pet a name.");
    return trimmed === pet ? Promise.resolve() : api.setPetName(trimmed);
  };

  const finish = async () => {
    if (await action.run(api.finishOnboarding)) {
      onDone();
      api.closeSettings().catch(() => {});
    }
  };

  return (
    <div className="onboarding">
      <div className="progress" role="progressbar" aria-valuemin={1} aria-valuemax={3} aria-valuenow={step + 1} aria-label={`Step ${step + 1} of 3`}>
        {STEPS.map((s, i) => (
          <span key={s} className={`progress-seg${i <= step ? " is-done" : ""}`} />
        ))}
      </div>

      <div className="onboarding-body" key={step}>
        {step === 0 && (
          <>
            <p className="step-count">Step 1 of 3</p>
            <h1>Choose your pet</h1>
            <p className="lede">It sits above your windows and shows what Claude Code is up to.</p>
            <PetPicker selectedId={snap.config.petId} onSelect={(id) => void action.run(() => api.setPet(id))} />
            <label className="field">
              <span className="field-label">Name</span>
              <input
                className="text-input"
                value={name}
                maxLength={NAME_MAX}
                spellCheck={false}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && void next(saveName, 1)}
              />
            </label>
          </>
        )}

        {step === 1 && (
          <>
            <p className="step-count">Step 2 of 3</p>
            <h1>Let {pet} watch your sessions?</h1>
            <p className="lede">
              {pet} adds a few hooks to your Claude Code settings file, so it can tell when a session is working, needs
              you, or is done.
            </p>
            <ul className="facts">
              <li>The hooks only talk to this computer.</li>
              <li>Your settings file is backed up first.</li>
              <li>You can remove them any time in Settings.</li>
            </ul>
            <p className="hint">Skip this and {pet} only follows the questions you ask it directly.</p>
          </>
        )}

        {step === 2 && (
          <>
            <p className="step-count">Step 3 of 3</p>
            <h1>Checking Claude Code</h1>
            <p className="lede">
              {pet} runs Claude Code with your subscription login. Here's what it found.
            </p>
            <div className="group is-plain">
              <SetupChecks snap={snap} />
            </div>
            {snap.setup.needsSetup && (
              <p className="hint">You can finish now and fix this later from Settings; {pet} will remind you.</p>
            )}
          </>
        )}
      </div>

      <footer className="onboarding-foot">
        {step > 0 ? (
          <button type="button" className="btn btn-ghost" onClick={() => setStep(step - 1)} disabled={action.busy}>
            <IconChevronLeft size={15} />
            Back
          </button>
        ) : (
          <span />
        )}
        <div className="actions">
          {step === 0 && (
            <button type="button" className="btn btn-primary" disabled={action.busy || !name.trim()} onClick={() => void next(saveName, 1)}>
              Continue
            </button>
          )}
          {step === 1 && (
            <>
              <button type="button" className="btn btn-ghost" disabled={action.busy} onClick={() => void next(api.declineHooks, 2)}>
                Skip
              </button>
              <button type="button" className="btn btn-primary" disabled={action.busy} onClick={() => void next(api.installHooks, 2)}>
                Install hooks
              </button>
            </>
          )}
          {step === 2 && (
            <>
              <button type="button" className="btn btn-secondary" disabled={action.busy} onClick={() => void action.run(api.recheckSetup)}>
                <IconRefresh size={14} />
                Check again
              </button>
              <button type="button" className="btn btn-primary" disabled={action.busy} onClick={() => void finish()}>
                Finish
              </button>
            </>
          )}
        </div>
      </footer>
      <NoticeBar notice={action.notice} onDismiss={action.dismiss} />
    </div>
  );
}
