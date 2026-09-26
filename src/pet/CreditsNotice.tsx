import { useId } from "react";
import { IconInfo } from "../shared/icons";

interface Props {
  petName: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Shown inline in the composer the first time the user sends an Ask. */
export function CreditsNotice({ petName, onConfirm, onCancel }: Props) {
  const title = useId();
  const body = useId();
  return (
    <div className="credits-notice" role="alertdialog" aria-labelledby={title} aria-describedby={body}>
      <div className="credits-head">
        <IconInfo size={15} />
        <strong id={title}>Before your first ask</strong>
      </div>
      <div id={body} className="credits-body">
        <p>
          {petName} runs Claude Code on your subscription, never an API key. Asks count toward your plan's usage like
          any other prompt.
        </p>
        <p>If usage credits are on in your Claude account, {petName} stops a run the moment it would start using them.</p>
      </div>
      <div className="credits-actions">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="btn btn-primary btn-sm" onClick={onConfirm} autoFocus>
          Got it, send
        </button>
      </div>
    </div>
  );
}
