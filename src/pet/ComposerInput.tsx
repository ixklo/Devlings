import { useLayoutEffect, useRef, type ReactNode } from "react";
import { IconArrowUp, IconStop } from "../shared/icons";

const MAX_LINES = 6;

interface Props {
  value: string;
  onChange: (value: string) => void;
  /** Called with trimmed, non-empty text on Enter or the send button. */
  onSubmit: (text: string) => void;
  onEscape?: () => void;
  placeholder: string;
  /** Sending is blocked (no project, busy…); typing still works. */
  sendDisabled?: boolean;
  running?: boolean;
  onStop?: () => void;
  autoFocus?: boolean;
  label?: string;
  /** "stacked" puts `tools` and the button under the text; "row" puts the button beside it. */
  layout?: "stacked" | "row";
  tools?: ReactNode;
}

/** Auto-growing message box: Enter sends, Shift+Enter adds a line, Esc closes. */
export function ComposerInput({
  value,
  onChange,
  onSubmit,
  onEscape,
  placeholder,
  sendDisabled,
  running,
  onStop,
  autoFocus,
  label = "Message",
  layout = "stacked",
  tools,
}: Props) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const trimmed = value.trim();
  const canSend = !!trimmed && !running && !sendDisabled;

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const cs = window.getComputedStyle(el);
    const line = parseFloat(cs.lineHeight) || 19;
    const pad = (parseFloat(cs.paddingTop) || 0) + (parseFloat(cs.paddingBottom) || 0);
    const max = line * MAX_LINES + pad;
    el.style.height = "auto";
    const next = Math.min(el.scrollHeight || line + pad, max);
    el.style.height = `${Math.max(next, line + pad)}px`;
    el.style.overflowY = el.scrollHeight > max ? "auto" : "hidden";
  }, [value]);

  const submit = () => {
    if (canSend) onSubmit(trimmed);
  };

  const button = running ? (
    <button type="button" className="send-btn is-stop" aria-label="Stop" title="Stop" onClick={onStop}>
      <IconStop size={12} />
    </button>
  ) : (
    <button
      type="button"
      className="send-btn"
      aria-label="Send"
      title="Send (Enter)"
      disabled={!canSend}
      onClick={submit}
    >
      <IconArrowUp size={16} strokeWidth={2.2} />
    </button>
  );

  const textarea = (
    <textarea
      ref={ref}
      className="composer-text"
      aria-label={label}
      rows={1}
      value={value}
      autoFocus={autoFocus}
      placeholder={placeholder}
      spellCheck
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
          e.preventDefault();
          submit();
        } else if (e.key === "Escape" && onEscape) {
          e.preventDefault();
          e.stopPropagation();
          onEscape();
        }
      }}
    />
  );

  if (layout === "row") {
    return (
      <div className="composer-input is-row">
        {textarea}
        {button}
      </div>
    );
  }
  return (
    <div className="composer-input is-stacked">
      {textarea}
      <div className="composer-foot">
        <div className="composer-tools">{tools}</div>
        {button}
      </div>
    </div>
  );
}
