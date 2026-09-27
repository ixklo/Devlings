import type { KeyboardEvent } from "react";
import { IconBell, IconBellFilled, IconChevronDown, IconChevronUp, IconPencil } from "../shared/icons";

interface Props {
  visible: boolean;
  composerOpen: boolean;
  notifications: boolean;
  /** The system won't show them anyway (Windows' own switch is off). */
  systemNotificationsOff?: boolean;
  collapsed: boolean;
  onCompose: () => void;
  onToggleNotifications: () => void;
  onToggleCollapsed: () => void;
}

/** Left/Right (and Home/End) move between the bar's buttons, as in any toolbar; Tab works too. */
function onToolbarKey(e: KeyboardEvent<HTMLDivElement>) {
  const keys = ["ArrowLeft", "ArrowRight", "Home", "End"];
  if (!keys.includes(e.key)) return;
  const buttons = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
  const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
  const last = buttons.length - 1;
  const next = e.key === "Home" ? 0 : e.key === "End" ? last : e.key === "ArrowLeft" ? (i <= 0 ? last : i - 1) : i >= last ? 0 : i + 1;
  e.preventDefault();
  // Arrow keys elsewhere nudge the pet; here they only move focus.
  e.stopPropagation();
  buttons[next]?.focus();
}

/** The pill under the pet: composer, notifications, collapse. */
export function ControlBar({
  visible,
  composerOpen,
  notifications,
  systemNotificationsOff = false,
  collapsed,
  onCompose,
  onToggleNotifications,
  onToggleCollapsed,
}: Props) {
  return (
    <div
      className={`control-bar${visible ? " is-visible" : ""}`}
      role="toolbar"
      aria-label="Pet controls"
      data-hit={visible ? "bar" : undefined}
      onKeyDown={onToolbarKey}
    >
      <button
        type="button"
        className={`icon-btn${composerOpen ? " is-active" : ""}`}
        aria-label={composerOpen ? "Close composer" : "New message"}
        aria-expanded={composerOpen}
        title={composerOpen ? "Close composer" : "New message"}
        onClick={onCompose}
      >
        <IconPencil size={15} />
      </button>
      <button
        type="button"
        className={`icon-btn${notifications ? " is-on" : ""}`}
        aria-label="Notifications"
        aria-pressed={notifications}
        title={
          !notifications ? "Notifications off" : systemNotificationsOff ? "Notifications on, but Windows has them turned off" : "Notifications on"
        }
        onClick={onToggleNotifications}
      >
        {notifications ? <IconBellFilled size={15} /> : <IconBell size={15} />}
      </button>
      <button
        type="button"
        className="icon-btn"
        aria-label={collapsed ? "Show threads" : "Hide threads"}
        aria-expanded={!collapsed}
        title={collapsed ? "Show threads" : "Hide threads"}
        onClick={onToggleCollapsed}
      >
        {collapsed ? <IconChevronUp size={16} /> : <IconChevronDown size={16} />}
      </button>
    </div>
  );
}
