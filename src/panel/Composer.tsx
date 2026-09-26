import { useState } from "react";

interface Props {
  running: boolean;
  disabled?: boolean;
  onSend: (text: string) => void;
  onStop: () => void;
}

export function Composer({ running, disabled, onSend, onStop }: Props) {
  const [text, setText] = useState("");
  const send = () => {
    const t = text.trim();
    if (!t || running || disabled) return;
    onSend(t);
    setText("");
  };
  return (
    <div className="composer">
      <textarea
        aria-label="Message"
        rows={2}
        value={text}
        disabled={disabled}
        placeholder={disabled ? "Pick a project first" : "Ask about this project…"}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
            e.preventDefault();
            send();
          }
        }}
      />
      {running ? (
        <button onClick={onStop}>Stop</button>
      ) : (
        <button onClick={send} disabled={disabled || !text.trim()}>
          Send
        </button>
      )}
    </div>
  );
}
