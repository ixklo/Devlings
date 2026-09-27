import { useRef, type PointerEvent } from "react";
import { api } from "../shared/api";
import type { Clip } from "../sprite/petAnimation";
import { SpriteView } from "../sprite/SpriteView";

export const DRAG_THRESHOLD_PX = 4;
/** Window moves are batched to about one per frame. */
export const DRAG_FLUSH_MS = 16;

interface Props {
  src: string | null;
  scale: number;
  clip: Clip;
  onClipDone: () => void;
  label: string;
  /** Id of an element describing the keys (Enter, arrows, Esc, the menu key). */
  describedBy?: string;
  /** A click without movement. */
  onActivate: () => void;
  onHover: () => void;
  /** A drag started, heading this way. */
  onDragStart?: (dir: "left" | "right") => void;
  /** The drag ended (button released or capture lost). */
  onDragEnd?: () => void;
}

interface Drag {
  x: number;
  y: number;
  dx: number;
  dy: number;
  timer: number | null;
}

/**
 * The pet itself. Dragging it moves the window (after 4 px of movement), a
 * plain click toggles the composer, right-click opens the pet menu.
 *
 * The drag moves the window from here rather than through the OS drag loop:
 * during that loop the webview hears nothing, so the pet couldn't run.
 */
export function PetSprite({
  src,
  scale,
  clip,
  onClipDone,
  label,
  describedBy,
  onActivate,
  onHover,
  onDragStart,
  onDragEnd,
}: Props) {
  const down = useRef<{ x: number; y: number } | null>(null);
  const drag = useRef<Drag | null>(null);

  const flush = () => {
    const d = drag.current;
    if (!d) return;
    d.timer = null;
    const { dx, dy } = d;
    d.dx = 0;
    d.dy = 0;
    if (dx || dy) api.dragPetBy(dx, dy).catch(() => {});
  };

  const queue = (d: Drag, dx: number, dy: number) => {
    d.dx += dx;
    d.dy += dy;
    if (d.timer === null) d.timer = window.setTimeout(flush, DRAG_FLUSH_MS);
  };

  const endDrag = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return false;
    if (d.timer !== null) window.clearTimeout(d.timer);
    flush();
    drag.current = null;
    e.currentTarget.releasePointerCapture?.(e.pointerId);
    onDragEnd?.();
    return true;
  };

  return (
    <div
      className="pet"
      role="button"
      tabIndex={0}
      aria-label={label}
      aria-describedby={describedBy}
      title={label}
      data-hit="pet"
      onPointerEnter={onHover}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        down.current = { x: e.screenX, y: e.screenY };
        e.currentTarget.setPointerCapture?.(e.pointerId);
      }}
      onPointerMove={(e) => {
        const d = drag.current;
        if (d) {
          queue(d, e.screenX - d.x, e.screenY - d.y);
          d.x = e.screenX;
          d.y = e.screenY;
          return;
        }
        const start = down.current;
        if (!start || Math.hypot(e.screenX - start.x, e.screenY - start.y) <= DRAG_THRESHOLD_PX) return;
        down.current = null;
        const next: Drag = { x: e.screenX, y: e.screenY, dx: 0, dy: 0, timer: null };
        drag.current = next;
        onDragStart?.(e.screenX - start.x < 0 ? "left" : "right");
        queue(next, e.screenX - start.x, e.screenY - start.y);
      }}
      onPointerUp={(e) => {
        if (endDrag(e)) return;
        if (e.button !== 0 || !down.current) return;
        down.current = null;
        onActivate();
      }}
      onPointerCancel={(e) => {
        endDrag(e);
        down.current = null;
      }}
      onLostPointerCapture={(e) => {
        endDrag(e);
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
    </div>
  );
}
