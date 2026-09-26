import { api } from "../shared/api";
import { IconX } from "../shared/icons";
import { STATUS_TEXT, threadLine } from "../shared/threads";
import { relativeTime } from "../shared/time";
import type { ThreadInfo } from "../shared/types";
import { StatusIndicator } from "./StatusIndicator";

interface Props {
  thread: ThreadInfo;
  now: number;
  /** The card nearest the pet gets the speech-bubble tail. */
  tail?: boolean;
  /** Ask threads open in the pet's thread view. */
  onOpenThread: (thread: ThreadInfo) => void;
}

/** Ask cards open the thread view; Watch cards mark the thread viewed and open the project in the editor. */
export async function activateThread(thread: ThreadInfo, onOpenThread: (t: ThreadInfo) => void) {
  if (thread.source === "ask") {
    onOpenThread(thread);
    return;
  }
  await api.markViewed(thread.sessionId);
  // Sessions without a folder (projectName "Claude Code") have nothing to open.
  if (thread.project) await api.openProject(thread.project);
}

export function ThreadCard({ thread, now, tail, onOpenThread }: Props) {
  const line = threadLine(thread);
  const dismissible = thread.status === "ready" || thread.status === "blocked";
  const action = thread.source === "ask" ? "Open conversation" : thread.project ? "Open project" : "Mark as seen";
  return (
    <div
      className={`card thread-card is-${thread.status}${tail ? " has-tail" : ""}${thread.unread ? " is-unread" : ""}${
        dismissible ? " is-dismissible" : ""
      }`}
      data-hit=""
    >
      <button
        type="button"
        className="thread-card-main"
        title={thread.project ? `${action}: ${thread.project}` : action}
        aria-label={`${thread.projectName}, ${STATUS_TEXT[thread.status]}: ${line}. ${action}.`}
        onClick={() => void activateThread(thread, onOpenThread).catch(() => {})}
      >
        <StatusIndicator status={thread.status} />
        <span className="thread-card-body">
          <span className="thread-card-top">
            <span className="thread-card-project">{thread.projectName}</span>
            {thread.source === "ask" && <span className="thread-card-source">Ask</span>}
            <time className="thread-card-time" dateTime={new Date(thread.updatedAt).toISOString()}>
              {relativeTime(thread.updatedAt, now)}
            </time>
          </span>
          <span className="thread-card-line">{line}</span>
        </span>
      </button>
      {dismissible && (
        <button
          type="button"
          className="thread-card-dismiss"
          aria-label={`Dismiss ${thread.projectName}`}
          title="Dismiss"
          onClick={() => void api.markViewed(thread.sessionId).catch(() => {})}
        >
          <IconX size={12} strokeWidth={2.2} />
        </button>
      )}
    </div>
  );
}
