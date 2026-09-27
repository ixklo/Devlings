import type { ThreadStatus } from "../shared/types";
import { STATUS_TEXT } from "../shared/threads";

/** Running = spinning ring, needs input = pulsing amber dot, ready = green check, blocked = red cross. */
export function StatusIndicator({ status, size = 16 }: { status: ThreadStatus; size?: number }) {
  const label = STATUS_TEXT[status];
  return (
    <span className={`status status-${status}`} style={{ width: size, height: size }} role="img" aria-label={label}>
      {status === "running" && (
        <svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true">
          <circle className="status-track" cx="8" cy="8" r="6" fill="none" strokeWidth="2" />
          <path className="status-arc" d="M8 2a6 6 0 0 1 6 6" fill="none" strokeWidth="2" strokeLinecap="round" />
        </svg>
      )}
      {status === "needs_input" && <span className="status-dot" aria-hidden="true" />}
      {status === "ready" && (
        <svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true">
          <circle cx="8" cy="8" r="8" className="status-fill" />
          <path d="M4.8 8.3 7 10.4l4.2-4.6" className="status-glyph" fill="none" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      )}
      {status === "blocked" && (
        <svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true">
          <circle cx="8" cy="8" r="8" className="status-fill" />
          <path d="M5.6 5.6l4.8 4.8M10.4 5.6l-4.8 4.8" className="status-glyph" fill="none" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      )}
      {status === "idle" && <span className="status-dot" aria-hidden="true" />}
    </span>
  );
}
