import { projectName } from "../shared/paths";
import type { Kind, SessionInfo } from "../shared/types";

const STATE_TEXT: Record<Kind, string> = {
  started: "Session open",
  prompt: "Thinking…",
  step: "Working…",
  blocked: "Blocked",
  needs_you: "Needs your approval",
  reply_delta: "Writing a reply…",
  done: "Done",
  failed: "Something went wrong",
  ended: "Ended",
};

export function Activity({ sessions }: { sessions: SessionInfo[] }) {
  if (!sessions.length) {
    return <p className="muted activity-empty">No Claude Code sessions yet. Start one in VS Code, or ask below.</p>;
  }
  return (
    <ul className="activity">
      {sessions.map((s) => (
        <li key={s.sessionId} className={`state-${s.state}`}>
          <span className="proj">{projectName(s.project)}</span>
          <span className="source">{s.source === "ask" ? "Ask" : "Watching"}</span>
          <span className="label">{s.label ?? STATE_TEXT[s.state]}</span>
        </li>
      ))}
    </ul>
  );
}
