import { useEffect, useRef, useState } from "react";
import { api } from "../shared/api";
import { IconChevronLeft, IconExternal, IconRefresh } from "../shared/icons";
import type { Snapshot } from "../shared/types";
import { REPO_URL } from "./About";
import { SwitchRow } from "./controls";
import { NAME_MAX } from "./NameField";
import { NoticeBar } from "./NoticeBar";
import { PetPicker } from "./PetPicker";
import { SetupChecks } from "./SetupChecks";
import { useAction } from "./useAction";

/** Three steps at most: what Perch watches, asking from the pet, and permission prompts. */
export const STEPS = ["Watch", "Ask", "Permission prompts"] as const;

/** The README's Cost section: how Asks are counted against a plan. */
export const COST_URL = `${REPO_URL}#cost`;

/**
 * First run (and a re-run from Settings). Step 1 picks and names the pet and offers the hooks that let it
 * watch sessions; step 2 explains Asks and checks Claude Code; step 3 explains permission prompts, with the
 * same switch as Settings → Approvals. Finishing marks onboarding done, and with it the approvals intro.
 */
export function Onboarding({ snap, onDone }: { snap: Snapshot; onDone: () => void }) {
  const [step, setStep] = useState(0);
  const [name, setName] = useState(snap.config.petName);
  // Until you type in it, the Name box follows the saved name, which picking a pet can change
  // (a default name follows the pet; a name you chose stays).
  const [typed, setTyped] = useState(false);
  const action = useAction();
  const pet = snap.config.petName;
  useEffect(() => {
    if (!typed) setName(pet);
  }, [pet, typed]);
  const hooks = snap.setup.hooksInstalled;
  const total = STEPS.length;
  // Step 1's text follows the name as it's typed.
  const shown = step === 0 ? name.trim() || pet : pet;

  // Each new step's heading takes focus, so keyboard and screen reader users land at its start
  // (the button they pressed is gone). Not on the first render.
  const heading = useRef<HTMLHeadingElement>(null);
  const moved = useRef(false);
  useEffect(() => {
    if (moved.current) heading.current?.focus();
  }, [step]);

  const go = (to: number) => {
    moved.current = true;
    setStep(to);
  };

  const next = async (work: () => Promise<unknown>, to: number) => {
    if (await action.run(work)) go(to);
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
      <div
        className="progress"
        role="progressbar"
        aria-valuemin={1}
        aria-valuemax={total}
        aria-valuenow={step + 1}
        aria-label={`Step ${step + 1} of ${total}: ${STEPS[step]}`}
      >
        {STEPS.map((s, i) => (
          <span key={s} className={`progress-seg${i <= step ? " is-done" : ""}`} />
        ))}
      </div>

      <div className="onboarding-body" key={step}>
        <p className="step-count">
          Step {step + 1} of {total} · {STEPS[step]}
        </p>

        {step === 0 && (
          <>
            <h1 ref={heading} tabIndex={-1}>
              Choose your pet
            </h1>
            <PetPicker selectedId={snap.config.petId} onSelect={(id) => void action.run(() => api.setPet(id))} />
            <label className="field">
              <span className="field-label">Name</span>
              <input
                className="text-input"
                value={name}
                maxLength={NAME_MAX}
                spellCheck={false}
                onChange={(e) => {
                  setTyped(true);
                  setName(e.target.value);
                }}
                onKeyDown={(e) => e.key === "Enter" && void action.run(saveName)}
              />
            </label>
            <h2 className="step-sub">Watch your sessions</h2>
            <p className="lede">
              {shown} shows every Claude Code session as a card: working, needs you, or done. To see them, it adds small
              hooks to Claude Code's settings. They only talk to this computer. Your settings file is backed up first,
              and you can remove them any time in Settings.
            </p>
            {!hooks && <p className="hint">Skip this and {shown} only follows the questions you ask it.</p>}
          </>
        )}

        {step === 1 && (
          <>
            <h1 ref={heading} tabIndex={-1}>
              Ask Claude Code from {pet}
            </h1>
            <p className="lede">
              Click {pet}, choose a project and type. It runs your own Claude Code on your subscription and never uses an
              API key.
            </p>
            <ul className="facts">
              <li>If a run would start using paid usage credits, {pet} stops it.</li>
              <li>
                Asks draw on your plan's monthly Agent SDK credit.{" "}
                <button type="button" className="link" onClick={() => void api.openUrl(COST_URL)}>
                  How Asks are counted
                  <IconExternal size={12} />
                </button>
              </li>
            </ul>
            <h2 className="step-sub">Claude Code on this computer</h2>
            <div className="group is-plain">
              <SetupChecks snap={snap} />
            </div>
            {snap.setup.needsSetup && (
              <p className="hint">You can go on and fix this later from Settings; {pet} will remind you.</p>
            )}
          </>
        )}

        {step === 2 && (
          <>
            <h1 ref={heading} tabIndex={-1}>
              Answer permission prompts
            </h1>
            <p className="lede">
              When Claude Code asks to run a command or change a file, {pet} can show the request as a card with Allow
              and Deny.
            </p>
            <ul className="facts">
              <li>On for Asks you start from {pet}.</li>
              <li>Claude Code's own prompt still works too.</li>
            </ul>
            <div className="group">
              <SwitchRow
                label="Answer prompts from your other sessions too"
                description={
                  hooks
                    ? "Claude Code in VS Code or the terminal. Off until you turn it on. Change it any time in Settings → Approvals."
                    : "Needs the hooks from step 1. You can turn this on later in Settings → Approvals."
                }
                checked={snap.config.watchApprovals}
                disabled={!hooks || action.busy}
                onChange={(v) => void action.run(() => api.setWatchApprovals(v))}
              />
            </div>
          </>
        )}
      </div>

      <footer className="onboarding-foot">
        {step > 0 ? (
          <button type="button" className="btn btn-ghost" onClick={() => go(step - 1)} disabled={action.busy}>
            <IconChevronLeft size={15} />
            Back
          </button>
        ) : (
          <span />
        )}
        <div className="actions">
          {step === 0 &&
            (hooks ? (
              <button type="button" className="btn btn-primary" disabled={action.busy || !name.trim()} onClick={() => void next(saveName, 1)}>
                Continue
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="btn btn-ghost"
                  disabled={action.busy || !name.trim()}
                  onClick={() => void next(() => saveName().then(api.declineHooks), 1)}
                >
                  Skip
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={action.busy || !name.trim()}
                  onClick={() => void next(() => saveName().then(api.installHooks), 1)}
                >
                  Install hooks
                </button>
              </>
            ))}
          {step === 1 && (
            <>
              <button type="button" className="btn btn-secondary" disabled={action.busy} onClick={() => void action.run(api.recheckSetup)}>
                <IconRefresh size={14} />
                Check again
              </button>
              <button type="button" className="btn btn-primary" disabled={action.busy} onClick={() => go(2)}>
                Continue
              </button>
            </>
          )}
          {step === 2 && (
            <button type="button" className="btn btn-primary" disabled={action.busy} onClick={() => void finish()}>
              Finish
            </button>
          )}
        </div>
      </footer>
      <NoticeBar notice={action.notice} onDismiss={action.dismiss} />
    </div>
  );
}
