import { IconAlert, IconCheck, IconX } from "../shared/icons";
import type { Notice } from "./useAction";

/** A small toast at the bottom of the settings window for results and errors. */
export function NoticeBar({ notice, onDismiss }: { notice: Notice | null; onDismiss: () => void }) {
  return (
    <div className="notice-region" aria-live="polite">
      {notice && (
        <div className={`notice ${notice.ok ? "is-ok" : "is-error"}`} role={notice.ok ? "status" : "alert"}>
          {notice.ok ? <IconCheck size={14} strokeWidth={2.4} /> : <IconAlert size={14} />}
          <span>{notice.text}</span>
          <button type="button" className="icon-btn notice-close" aria-label="Dismiss" onClick={onDismiss}>
            <IconX size={13} />
          </button>
        </div>
      )}
    </div>
  );
}
