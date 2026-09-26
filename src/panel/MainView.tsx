import { useEffect, useRef, useState } from "react";
import { api } from "../shared/api";
import { samePath } from "../shared/paths";
import type { ChatTurn, PermissionMode, Snapshot } from "../shared/types";
import { Activity } from "./Activity";
import { Composer } from "./Composer";
import { Conversation } from "./ConversationView";
import { applyAskEvent } from "./conversation";
import { CreditsNotice } from "./CreditsNotice";
import { ProjectPicker } from "./ProjectPicker";

export function MainView({ snap, onOpenSettings }: { snap: Snapshot; onOpenSettings?: () => void }) {
  const [project, setProject] = useState<string | null>(null);
  const [turns, setTurns] = useState<ChatTurn[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pendingPrompt, setPendingPrompt] = useState<string | null>(null);
  const projectRef = useRef<string | null>(null);
  projectRef.current = project;

  useEffect(() => {
    const unlisten = api.onPetEvent((ev) => {
      const current = projectRef.current;
      if (ev.source === "ask" && current && samePath(ev.project, current)) {
        setTurns((t) => applyAskEvent(t, ev));
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!project && snap.projects.length) setProject(snap.projects[0].path);
  }, [snap.projects, project]);

  useEffect(() => {
    if (!project) {
      setTurns([]);
      return;
    }
    api.loadConversation(project).then((t) => setTurns(t));
  }, [project]);

  const entry = project ? snap.projects.find((p) => samePath(p.path, project)) ?? null : null;
  const running = !!project && snap.running.some((r) => samePath(r, project));

  const send = async (text: string) => {
    if (!project) return;
    setError(null);
    setTurns((t) => [...t, { role: "user", text }]);
    try {
      await api.ask(project, text);
    } catch (e) {
      const msg = String(e);
      setTurns((t) => t.slice(0, -1));
      if (msg === "credits_notice") setPendingPrompt(text);
      else setError(msg);
    }
  };

  return (
    <div className="panel">
      <header className="panel-header">
        <span className="pet-name">{snap.config.petName}</span>
        <ProjectPicker projects={snap.projects} value={project} onChange={setProject} onError={setError} />
        {onOpenSettings && (
          <button className="secondary" onClick={onOpenSettings}>
            Settings
          </button>
        )}
      </header>
      {snap.setup.needsSetup && onOpenSettings && (
        <button className="setup-banner" onClick={onOpenSettings}>
          Setup needed. Open Settings
        </button>
      )}
      <Activity sessions={snap.sessions} />
      <Conversation turns={turns} petName={snap.config.petName} />
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {entry && (
        <div className="composer-bar">
          <select
            aria-label="Permission mode"
            value={entry.permissionMode}
            onChange={(e) => api.setPermissionMode(entry.path, e.target.value as PermissionMode)}
          >
            <option value="read_only">Read only</option>
            <option value="edit_files">Edit files</option>
            <option value="auto">Auto</option>
          </select>
          <button
            className="secondary"
            disabled={running}
            onClick={() => api.newConversation(entry.path).then(() => setTurns([]))}
          >
            New conversation
          </button>
        </div>
      )}
      <Composer running={running} disabled={!project} onSend={send} onStop={() => project && api.stopAsk(project)} />
      {pendingPrompt !== null && (
        <CreditsNotice
          petName={snap.config.petName}
          onCancel={() => setPendingPrompt(null)}
          onOk={async () => {
            const text = pendingPrompt;
            setPendingPrompt(null);
            await api.markCreditsNoticeSeen();
            await send(text);
          }}
        />
      )}
    </div>
  );
}
