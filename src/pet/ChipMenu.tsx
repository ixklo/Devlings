import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { IconCheck, IconChevronDown } from "../shared/icons";

export interface ChipMenuItem {
  key: string;
  label: string;
  detail?: string;
  icon?: ReactNode;
  selected?: boolean;
  separatorBefore?: boolean;
  onSelect: () => void;
}

interface Props {
  /** Accessible name of the chip, e.g. "Project". */
  label: string;
  icon?: ReactNode;
  text: string;
  title?: string;
  heading?: string;
  items: ChipMenuItem[];
  disabled?: boolean;
}

/** A pill button that opens a small menu upward, above the composer. */
export function ChipMenu({ label, icon, text, title, heading, items, disabled }: Props) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const id = useId();

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (!wrap.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    const first =
      menu.current?.querySelector<HTMLButtonElement>('[aria-checked="true"]') ??
      menu.current?.querySelector<HTMLButtonElement>("button");
    first?.focus();
    return () => document.removeEventListener("pointerdown", onDown);
  }, [open]);

  const onMenuKey = (e: KeyboardEvent) => {
    const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button") ?? []);
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = e.key === "ArrowDown" ? (i + 1) % buttons.length : (i - 1 + buttons.length) % buttons.length;
      buttons[next]?.focus();
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      buttons[e.key === "Home" ? 0 : buttons.length - 1]?.focus();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      button.current?.focus();
    } else if (e.key === "Tab") {
      setOpen(false);
    }
  };

  return (
    <div className="chip-wrap" ref={wrap}>
      <button
        ref={button}
        type="button"
        className={`chip${open ? " is-open" : ""}`}
        aria-label={`${label}: ${text}`}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        title={title}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        {icon}
        <span className="chip-text">{text}</span>
        <IconChevronDown className="chip-caret" size={12} strokeWidth={2.2} />
      </button>
      {open && (
        <div className="menu" id={id} role="menu" aria-label={label} ref={menu} data-hit="" onKeyDown={onMenuKey}>
          {heading && <div className="menu-heading">{heading}</div>}
          {items.map((item) => (
            <div key={item.key} role="none">
              {item.separatorBefore && <div className="menu-sep" role="separator" />}
              <button
                type="button"
                role={item.selected === undefined ? "menuitem" : "menuitemradio"}
                aria-checked={item.selected === undefined ? undefined : item.selected}
                className={`menu-item${item.detail ? " has-detail" : ""}`}
                onClick={() => {
                  setOpen(false);
                  button.current?.focus();
                  item.onSelect();
                }}
              >
                {item.icon && <span className="menu-icon">{item.icon}</span>}
                <span className="menu-label">
                  <span className="menu-title">{item.label}</span>
                  {item.detail && <span className="menu-detail">{item.detail}</span>}
                </span>
                {item.selected && <IconCheck className="menu-check" size={14} strokeWidth={2.2} />}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
