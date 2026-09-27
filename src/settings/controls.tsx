import { useId, type KeyboardEvent, type ReactNode } from "react";

/**
 * Arrow keys in a radio group (the segmented controls, the pet picker): move to the previous or
 * next option and choose it, wrapping around, as native radio buttons do. Home and End jump to the
 * first and last. Only the chosen option is a Tab stop (see `radioTabIndex`).
 */
export function onRadioGroupKey(e: KeyboardEvent<HTMLElement>) {
  const back = e.key === "ArrowLeft" || e.key === "ArrowUp";
  const forward = e.key === "ArrowRight" || e.key === "ArrowDown";
  if (!back && !forward && e.key !== "Home" && e.key !== "End") return;
  const radios = Array.from(e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]:not(:disabled)'));
  if (!radios.length) return;
  const i = radios.indexOf(document.activeElement as HTMLElement);
  const last = radios.length - 1;
  const next = e.key === "Home" ? 0 : e.key === "End" ? last : back ? (i <= 0 ? last : i - 1) : i >= last ? 0 : i + 1;
  e.preventDefault();
  radios[next].focus();
  radios[next].click();
}

/** Roving Tab stop: the chosen option, or the first one while none is chosen. */
export function radioTabIndex(checked: boolean, index: number, anyChecked: boolean): 0 | -1 {
  return checked || (!anyChecked && index === 0) ? 0 : -1;
}

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
  const descId = useId();
  return (
    <div className="row">
      <div className="row-text">
        <label htmlFor={id} className="row-label">
          {label}
        </label>
        {description && (
          <p id={descId} className="row-desc">
            {description}
          </p>
        )}
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-describedby={description ? descId : undefined}
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
    <div className="segmented" role="radiogroup" aria-label={label} onKeyDown={onRadioGroupKey}>
      {options.map((o, i) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          tabIndex={radioTabIndex(value === o.value, i, options.some((x) => x.value === value))}
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
