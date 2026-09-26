import { IconBell, IconBellFilled, IconChevronDown, IconChevronUp, IconPencil } from "../shared/icons";

interface Props {
  visible: boolean;
  composerOpen: boolean;
  notifications: boolean;
  collapsed: boolean;
  onCompose: () => void;
  onToggleNotifications: () => void;
  onToggleCollapsed: () => void;
}

/** The pill under the pet: composer, notifications, collapse. */
export function ControlBar({
  visible,
  composerOpen,
  notifications,
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
      data-hit={visible ? "" : undefined}
    >
      <button
        type="button"
        className={`icon-btn${composerOpen ? " is-active" : ""}`}
        aria-label={composerOpen ? "Close composer" : "New message"}
        aria-pressed={composerOpen}
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
        title={notifications ? "Notifications on" : "Notifications off"}
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
