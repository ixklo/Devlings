import { api } from "../shared/api";
import { IconExternal, IconRefresh, IconTerminal } from "../shared/icons";
import type { Snapshot } from "../shared/types";
import { idleClip, SpriteView, usePetSprite, useReducedMotion } from "../sprite/SpriteView";
import { About } from "./About";
import { Section, Segmented, SwitchRow } from "./controls";
import { NameField } from "./NameField";
import { PetPicker } from "./PetPicker";
import { PlanUsage } from "./PlanUsage";
import { SetupChecks } from "./SetupChecks";
import { TrustedFolders } from "./TrustedFolders";
import { useAction } from "./useAction";
import { NoticeBar } from "./NoticeBar";

const USAGE_URL = "https://claude.ai/settings/usage";

/** How long a watched permission request stays open for an answer here (design v1.0 D3). */
const HOLDS = [
  { value: "30", label: "30 s", title: "30 seconds" },
  { value: "60", label: "1 min", title: "1 minute" },
  { value: "120", label: "2 min", title: "2 minutes" },
  { value: "240", label: "4 min", title: "4 minutes" },
];

type Size = "s" | "m" | "l";
export const SIZES: Record<Size, number> = { s: 0.45, m: 0.6, l: 0.8 };

function sizeFor(scale: number): Size {
  return (Object.keys(SIZES) as Size[]).reduce((best, k) =>
    Math.abs(SIZES[k] - scale) < Math.abs(SIZES[best] - scale) ? k : best,
  );
}

export function Settings({ snap }: { snap: Snapshot }) {
  const { config, setup } = snap;
  const pet = config.petName;
  const action = useAction();
  const reduced = useReducedMotion();
  const avatar = usePetSprite(config.petId);
  const chooseBinary = async () => {
    const file = await api.chooseFile();
    if (file) await action.run(() => api.setClaudePath(file), "Using that Claude Code file.");
  };

  return (
    <div className="settings-page">
      <header className="page-head">
        <span className="avatar" aria-hidden="true">
          {avatar && <SpriteView src={avatar} scale={0.22} clip={idleClip(reduced)} />}
        </span>
        <div className="page-title">
          <h1>Settings</h1>
          <p>{pet} is keeping an eye on Claude Code.</p>
        </div>
      </header>

      <Section title="Pet">
        <div className="row is-block">
          <PetPicker selectedId={config.petId} onSelect={(id) => void action.run(() => api.setPet(id))} />
        </div>
        <div className="row">
          <div className="row-text">
            <span className="row-label" id="pet-name-label">
              Name
            </span>
          </div>
          <NameField value={pet} labelledBy="pet-name-label" onSave={(name) => action.run(() => api.setPetName(name), "Name saved.")} />
        </div>
        <div className="row">
          <div className="row-text">
            <span className="row-label">Size</span>
            <p className="row-desc">How big {pet} is on your desktop.</p>
          </div>
          <Segmented
            label="Pet size"
            value={sizeFor(config.petScale)}
            options={[
              { value: "s", label: "S", title: "Small" },
              { value: "m", label: "M", title: "Medium" },
              { value: "l", label: "L", title: "Large" },
            ]}
            onChange={(s) => void action.run(() => api.setPetScale(SIZES[s]))}
          />
        </div>
      </Section>

      <Section title="Claude Code">
        <div className="row is-block">
          <SetupChecks snap={snap} />
          {setup.claudePath && (
            <p className="mono-line" title={setup.claudePath}>
              <IconTerminal size={13} />
              <span>{setup.claudePath}</span>
            </p>
          )}
          <div className="actions">
            <button type="button" className="btn btn-secondary btn-sm" disabled={action.busy} onClick={() => void action.run(api.recheckSetup)}>
              <IconRefresh size={13} />
              Check again
            </button>
            <button type="button" className="btn btn-secondary btn-sm" disabled={action.busy} onClick={() => void chooseBinary()}>
              Choose file…
            </button>
            {config.claudePath && (
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                disabled={action.busy}
                onClick={() => void action.run(() => api.setClaudePath(null), "Auto-detecting Claude Code.")}
              >
                Auto-detect
              </button>
            )}
          </div>
        </div>
      </Section>

      <Section title="Watching">
        <div className="row">
          <div className="row-text">
            <span className="row-label">{setup.hooksInstalled ? "Hooks installed" : "Hooks not installed"}</span>
            <p className="row-desc">
              {pet} adds hooks to your Claude Code settings so it can see when a session is working, needs you, or is
              done. They only talk to this computer.
            </p>
          </div>
          {setup.hooksInstalled ? (
            <button
              type="button"
              className="btn btn-danger btn-sm"
              disabled={action.busy}
              onClick={() => void action.run(api.uninstallHooks, "Hooks removed.")}
            >
              Remove
            </button>
          ) : (
            <button
              type="button"
              className="btn btn-primary btn-sm"
              disabled={action.busy}
              onClick={() => void action.run(api.installHooks, "Hooks installed.")}
            >
              Install
            </button>
          )}
        </div>
        {setup.hookServerError && (
          <div className="row">
            <div className="row-text">
              <span className="row-label">Port in use</span>
              <p className="row-desc is-error">{setup.hookServerError}</p>
            </div>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              disabled={action.busy}
              onClick={() => void action.run(api.moveHooksPort, "Moved to a new port.")}
            >
              Move port
            </button>
          </div>
        )}
      </Section>

      <Section title="Approvals">
        <SwitchRow
          label={`Answer permission prompts from ${pet}`}
          description={
            <>
              When a Claude Code session asks to run something, Allow or Deny it from a card above {pet}. Claude
              Code's own prompt stays available the whole time. Asks you start from {pet} always ask here.
            </>
          }
          checked={config.watchApprovals}
          onChange={(v) => void action.run(() => api.setWatchApprovals(v))}
        />
        <div className="row">
          <div className="row-text">
            <span className="row-label">Keep a request open for</span>
            <p className="row-desc">After that, the card goes away and the request waits in Claude Code as usual.</p>
          </div>
          <Segmented
            label="Keep a request open for"
            value={HOLDS.some((h) => h.value === String(config.approvalHoldSecs)) ? String(config.approvalHoldSecs) : null}
            options={HOLDS}
            onChange={(secs) => void action.run(() => api.setApprovalHold(Number(secs)))}
          />
        </div>
      </Section>

      <TrustedFolders projects={snap.projects} petName={pet} action={action} />

      <Section title="Notifications & startup">
        <SwitchRow
          label="Notifications"
          description="When a session finishes, needs you, or gets stuck."
          checked={config.notifications}
          onChange={(v) => void action.run(() => api.setNotifications(v))}
        />
        <SwitchRow
          label={`Start ${pet} when you log in`}
          checked={config.launchAtLogin}
          onChange={(v) => void action.run(() => api.setLaunchAtLogin(v))}
        />
      </Section>

      <Section title="Cost">
        <div className="row is-block">
          <p className="row-desc is-body">
            Asks run Claude Code on your subscription, never an API key. If usage credits are on in your Claude account,{" "}
            {pet} stops a run the moment it would start using them.
          </p>
          <PlanUsage usage={snap.usage} />
          <button type="button" className="link" onClick={() => void api.openUrl(USAGE_URL)}>
            Claude usage settings
            <IconExternal size={12} />
          </button>
        </div>
      </Section>

      <About snap={snap} action={action} />

      <NoticeBar notice={action.notice} onDismiss={action.dismiss} />
    </div>
  );
}
