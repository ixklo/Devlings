import { useId, type ReactNode } from "react";

interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: ReactNode;
  disabled?: boolean;
}

/** A settings row with a label on the left and a switch on the right. */
export function SwitchRow({ checked, onChange, label, description, disabled }: SwitchProps) {
  const id = useId();
  return (
    <div className="row">
      <div className="row-text">
        <label htmlFor={id} className="row-label">
          {label}
        </label>
        {description && <p className="row-desc">{description}</p>}
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        className="switch"
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        <span className="switch-thumb" />
      </button>
    </div>
  );
}

interface SegmentedProps<T extends string> {
  label: string;
  value: T | null;
  options: { value: T; label: string; title?: string }[];
  onChange: (value: T) => void;
}

export function Segmented<T extends string>({ label, value, options, onChange }: SegmentedProps<T>) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          title={o.title}
          className="segment"
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

interface SectionProps {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}

export function Section({ title, description, children }: SectionProps) {
  const id = useId();
  return (
    <section className="section" aria-labelledby={id}>
      <div className="section-head">
        <h2 id={id}>{title}</h2>
        {description && <p className="section-desc">{description}</p>}
      </div>
      <div className="group">{children}</div>
    </section>
  );
}
