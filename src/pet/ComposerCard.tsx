import { useCallback, useState, type ReactNode } from "react";
import { api } from "../shared/api";
import { IconAlert, IconEye, IconFolder, IconFolderPlus, IconPencil, IconZap } from "../shared/icons";
import { projectName, samePath } from "../shared/paths";
import type { PermissionMode, Snapshot } from "../shared/types";
import { ChipMenu, type ChipMenuItem } from "./ChipMenu";
import { ComposerInput } from "./ComposerInput";
import { CreditsNotice } from "./CreditsNotice";
import { StatusIndicator } from "./StatusIndicator";
import { errorText } from "../shared/errors";
import { useAskGate } from "./useAskGate";

export const MODES: { mode: PermissionMode; label: string; detail: string; icon: ReactNode }[] = [
  { mode: "read_only", label: "Read only", detail: "Looks and answers. Changes nothing.", icon: <IconEye size={13} /> },
  { mode: "edit_files", label: "Edit files", detail: "Can edit files in the project.", icon: <IconPencil size={13} /> },
  { mode: "auto", label: "Auto", detail: "Edits and runs commands it judges safe.", icon: <IconZap size={13} /> },
];

const RECENT_LIMIT = 6;

interface Props {
  snap: Snapshot;
  project: string | null;
  draft: string;
  onDraftChange: (text: string) => void;
  onProjectChange: (path: string) => void;
  /** Sends the first message; resolves once the run has started. */
  onSend: (project: string, text: string) => Promise<void>;
  onClose: () => void;
  onOpenThread: (project: string) => void;
}

/** The compact "new message" card that opens from the pet. */
export function ComposerCard({
  snap,
  project,
  draft,
  onDraftChange,
  onProjectChange,
  onSend,
  onClose,
  onOpenThread,
}: Props) {
  const entry = project ? (snap.projects.find((p) => samePath(p.path, project)) ?? null) : null;
  const running = !!project && snap.running.some((r) => samePath(r, project));
  const name = entry?.name ?? (project ? projectName(project) : null);
  const petName = snap.config.petName;
  const [localError, setLocalError] = useState<string | null>(null);

  const send = useCallback(
    (text: string) => (project ? onSend(project, text) : Promise.reject(new Error("Choose a project first."))),
    [project, onSend],
  );
  const gate = useAskGate(send);
  const error = gate.error ?? localError;

  const submit = async (text: string) => {
    setLocalError(null);
    onDraftChange("");
    if (!(await gate.submit(text))) onDraftChange(text);
  };

  const confirmNotice = async () => {
    if (await gate.confirm()) onDraftChange("");
  };

  const chooseFolder = async () => {
    gate.clearError();
    setLocalError(null);
    try {
      const dir = await api.chooseFolder();
      if (!dir) return;
      await api.addProject(dir);
      onProjectChange(dir);
    } catch (e) {
      setLocalError(errorText(e));
    }
  };

  const projectItems: ChipMenuItem[] = [
    ...[...snap.projects]
      .sort((a, b) => b.lastSeen - a.lastSeen)
      .slice(0, RECENT_LIMIT)
      .map((p) => ({
        key: p.path,
        label: p.name,
        detail: p.path,
        selected: !!project && samePath(p.path, project),
        onSelect: () => onProjectChange(p.path),
      })),
    {
      key: "__choose",
      label: "Choose folder…",
      icon: <IconFolderPlus size={14} />,
      separatorBefore: snap.projects.length > 0,
      onSelect: () => void chooseFolder(),
    },
  ];

  const mode = MODES.find((m) => m.mode === entry?.permissionMode) ?? MODES[1];
  const modeItems: ChipMenuItem[] = MODES.map((m) => ({
    key: m.mode,
    label: m.label,
    detail: m.detail,
    icon: m.icon,
    selected: m.mode === mode.mode,
    onSelect: () => {
      if (entry) api.setPermissionMode(entry.path, m.mode).catch((e) => setLocalError(errorText(e)));
    },
  }));

  return (
    <section className="card composer-card view-enter" data-hit="" aria-label="New message">
      {gate.pending !== null && (
        <CreditsNotice petName={petName} onConfirm={() => void confirmNotice()} onCancel={gate.cancel} />
      )}
      {running && name && (
        <div className="composer-banner">
          <StatusIndicator status="running" size={14} />
          <span className="composer-banner-text">
            Working in <strong>{name}</strong>
          </span>
          <button type="button" className="link" onClick={() => project && onOpenThread(project)}>
            View
          </button>
        </div>
      )}
      {error && (
        <p className="inline-error" role="alert">
          <IconAlert size={14} />
          <span>{error}</span>
        </p>
      )}
      <ComposerInput
        value={draft}
        onChange={(v) => {
          onDraftChange(v);
          if (localError) setLocalError(null);
        }}
        onSubmit={(t) => void submit(t)}
        onEscape={onClose}
        placeholder={project ? "Ask Claude Code…" : "Choose a project to start"}
        sendDisabled={!project || running || gate.busy || gate.pending !== null}
        autoFocus
        tools={
          <>
            <ChipMenu
              label="Project"
              icon={<IconFolder size={13} />}
              text={name ?? "Choose project"}
              title={project ?? undefined}
              heading={snap.projects.length ? "Recent projects" : undefined}
              items={projectItems}
            />
            <ChipMenu
              label="Mode"
              icon={mode.icon}
              text={mode.label}
              title={mode.detail}
              items={modeItems}
              disabled={!entry}
            />
          </>
        }
      />
    </section>
  );
}
