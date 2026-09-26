import { useRef } from "react";
import { api } from "../shared/api";
import type { Clip } from "../sprite/petAnimation";
import { SpriteView } from "../sprite/SpriteView";

export const DRAG_THRESHOLD_PX = 4;

export interface Badge {
  count: number;
  tone: "neutral" | "wait" | "err";
}

interface Props {
  src: string | null;
  scale: number;
  clip: Clip;
  onClipDone: () => void;
  label: string;
  badge: Badge | null;
  /** A click without movement. */
  onActivate: () => void;
  onHover: () => void;
}

/**
 * The pet itself. Dragging it moves the window (after 4 px of movement), a
 * plain click toggles the composer, right-click opens the pet menu.
 */
export function PetSprite({ src, scale, clip, onClipDone, label, badge, onActivate, onHover }: Props) {
  const down = useRef<{ x: number; y: number } | null>(null);

  return (
    <div
      className="pet"
      role="button"
      tabIndex={0}
      aria-label={label}
      title={label}
      data-hit="pet"
      onPointerEnter={onHover}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        down.current = { x: e.screenX, y: e.screenY };
        e.currentTarget.setPointerCapture?.(e.pointerId);
      }}
      onPointerMove={(e) => {
        const start = down.current;
        if (!start || Math.hypot(e.screenX - start.x, e.screenY - start.y) <= DRAG_THRESHOLD_PX) return;
        down.current = null;
        e.currentTarget.releasePointerCapture?.(e.pointerId);
        api.startDragging().catch(() => {});
      }}
      onPointerUp={(e) => {
        if (e.button !== 0 || !down.current) return;
        down.current = null;
        onActivate();
      }}
      onPointerCancel={() => {
        down.current = null;
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        api.showPetMenu().catch(() => {});
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onActivate();
        }
      }}
    >
      <SpriteView src={src} scale={scale} clip={clip} onClipDone={onClipDone} aria-hidden="true" />
      {badge && badge.count > 0 && (
        <span className={`pet-badge is-${badge.tone}`} aria-hidden="true">
          {badge.count > 9 ? "9+" : badge.count}
        </span>
      )}
    </div>
  );
}
