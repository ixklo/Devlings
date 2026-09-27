import { useEffect, useRef, useState } from "react";
import { api } from "../shared/api";
import { IconAlert, IconSquarePen, IconX } from "../shared/icons";
import { projectName, samePath } from "../shared/paths";
import { STATUS_TEXT } from "../shared/threads";
import type { Snapshot, ThreadStatus } from "../shared/types";
import { ApprovalCard } from "./ApprovalCard";
import { ComposerInput } from "./ComposerInput";
import { CreditsNotice } from "./CreditsNotice";
import { Messages } from "./Messages";
import { StatusIndicator } from "./StatusIndicator";
import { UntrustedNotice } from "./UntrustedNotice";
import { useAskGate } from "./useAskGate";
import type { Conversation } from "./useConversation";

interface Props {
  snap: Snapshot;
  project: string;
  /** The session the view was opened for, if any (from a card or the tray). */
  initialSessionId: string | null;
  /** A prompt typed in the composer; sent once when the view opens. */
  initialPrompt?: string;
  conversation: Conversation;
  onClose: () => void;
}

/** The mini chat for one Ask thread: header, streamed messages, follow-up box. */
export function ThreadView({ snap, project, initialSessionId, initialPrompt, conversation, onClose }: Props) {
  const entry = snap.projects.find((p) => samePath(p.path, project)) ?? null;
  const thread =
    snap.threads.find((t) => t.source === "ask" && samePath(t.project, project)) ??
    (initialSessionId ? snap.threads.find((t) => t.sessionId === initialSessionId) : undefined);
  const running = snap.running.some((r) => samePath(r, project));
  const name = entry?.name ?? thread?.projectName ?? projectName(project);
  const sessionId = thread?.sessionId ?? conversation.sessionId ?? entry?.askSessionId ?? initialSessionId;
  const [draft, setDraft] = useState("");
  const gate = useAskGate(conversation.send);

  // Viewing a finished thread turns it idle, which drops it from the snapshot;
  // remember the last status so the header still says "Done".
  const [lastStatus, setLastStatus] = useState<ThreadStatus>("idle");
  useEffect(() => {
    if (thread) setLastStatus(thread.status);
  }, [thread?.status]);
  // This run's permission requests, answered inline (design v1.0 D4). Stop and New chat deny them in the backend.
  const approvals = snap.approvals.filter((a) => a.source === "ask" && samePath(a.project, project));
  const status: ThreadStatus = approvals.length ? "needs_input" : running ? "running" : (thread?.status ?? lastStatus);

  useEffect(() => {
    if (!sessionId) return;
    api.setFocusedThread(sessionId).catch(() => {});
    return () => {
      api.setFocusedThread(null).catch(() => {});
    };
  }, [sessionId]);

  const marked = useRef<string | null>(null);
  useEffect(() => {
    if (!thread) return;
    if (thread.unread || marked.current !== thread.sessionId) {
      marked.current = thread.sessionId;
      api.markViewed(thread.sessionId).catch(() => {});
    }
  }, [thread?.sessionId, thread?.unread]);

  const submit = async (text: string) => {
    setDraft("");
    if (!(await gate.submit(text))) setDraft(text);
  };

  // The ref survives StrictMode's double effect run, so the composer's prompt is sent once.
  const initialSent = useRef(false);
  useEffect(() => {
    if (!initialPrompt || initialSent.current) return;
    initialSent.current = true;
    void submit(initialPrompt);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialPrompt]);

  const statusText = status === "idle" ? "" : STATUS_TEXT[status];
  const activity = !running
    ? null
    : approvals.length
      ? "Waiting for your answer"
      : thread?.status === "running" && thread.label
        ? thread.label
        : "Thinking…";

  return (
    <section className="card thread-view view-enter" data-hit="" aria-label={`Conversation in ${name}`}>
      <header className="thread-head">
        <StatusIndicator status={status} size={14} />
        <div className="thread-title">
          <span className="thread-name" title={project}>
            {name}
          </span>
          {statusText && <span className={`thread-status is-${status}`}>{statusText}</span>}
        </div>
        <button
          type="button"
          className="btn btn-ghost btn-sm thread-new"
          onClick={() => {
            gate.clearError();
            void conversation.reset().catch(() => {});
          }}
          disabled={running || conversation.turns.length === 0}
          title="Start a new conversation"
        >
          <IconSquarePen size={14} />
          New chat
        </button>
        <button type="button" className="icon-btn" aria-label="Close" title="Close (Esc)" onClick={onClose}>
          <IconX size={15} />
        </button>
      </header>
      <Messages
        turns={conversation.turns}
        loading={conversation.loading}
        activity={activity}
        empty={
          <>
            <strong>Ask {snap.config.petName} about {name}.</strong>
            <span>Claude Code works in this folder with your subscription login.</span>
          </>
        }
        renderUntrusted={(turn) => (
          <UntrustedNotice
            headline={turn.text}
            skipped={turn.detail}
            petName={snap.config.petName}
            trusted={entry?.trusted ?? false}
            onTrust={() => api.trustProject(project)}
            onUntrust={() => api.untrustProject(project)}
          />
        )}
      />
      <footer className="thread-foot">
        {approvals.length > 0 && (
          <div className="approval-rows">
            {approvals.map((a) => (
              <ApprovalCard key={a.id} approval={a} holdMs={snap.config.approvalHoldSecs * 1000} variant="row" />
            ))}
          </div>
        )}
        {gate.pending !== null && (
          <CreditsNotice
            petName={snap.config.petName}
            onConfirm={() => void gate.confirm().then((ok) => ok && setDraft(""))}
            onCancel={gate.cancel}
          />
        )}
        {gate.error && (
          <p className="inline-error" role="alert">
            <IconAlert size={14} />
            <span>{gate.error}</span>
          </p>
        )}
        <ComposerInput
          layout="row"
          value={draft}
          onChange={setDraft}
          onSubmit={(t) => void submit(t)}
          onEscape={onClose}
          placeholder={running ? "Claude is working…" : "Reply…"}
          label="Reply"
          running={running}
          onStop={() => void api.stopAsk(project).catch(() => {})}
          sendDisabled={gate.busy || gate.pending !== null}
          autoFocus
        />
      </footer>
    </section>
  );
}
