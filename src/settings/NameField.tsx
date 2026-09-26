import { useEffect, useState } from "react";

interface Props {
  value: string;
  onSave: (name: string) => Promise<boolean>;
  labelledBy?: string;
  label?: string;
  autoFocus?: boolean;
}

export const NAME_MAX = 24;

/** Pet name input that saves on Enter or when it loses focus. */
export function NameField({ value, onSave, labelledBy, label, autoFocus }: Props) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  const commit = async () => {
    const name = draft.trim();
    if (!name || name === value) {
      setDraft(value);
      return;
    }
    if (!(await onSave(name))) setDraft(value);
  };

  return (
    <input
      className="text-input name-input"
      aria-labelledby={labelledBy}
      aria-label={labelledBy ? undefined : (label ?? "Pet name")}
      value={draft}
      maxLength={NAME_MAX}
      autoFocus={autoFocus}
      spellCheck={false}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => void commit()}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          (e.target as HTMLInputElement).blur();
        } else if (e.key === "Escape") {
          setDraft(value);
        }
      }}
    />
  );
}
