import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../shared/api";
import { useSnapshot } from "../shared/useSnapshot";
import { Critter } from "./Critter";
import { badgeFor, moodClass } from "./moodClass";
import "./pet.css";

export function Pet() {
  const snap = useSnapshot();
  const [bubble, setBubble] = useState<string | null>(null);
  const bubbleTimer = useRef<number | undefined>(undefined);
  const down = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    const unBubble = api.onBubble((text) => {
      setBubble(text);
      window.clearTimeout(bubbleTimer.current);
      bubbleTimer.current = window.setTimeout(() => setBubble(null), 4000);
    });
    let saveTimer: number | undefined;
    const unMoved = getCurrentWindow().onMoved(({ payload }) => {
      window.clearTimeout(saveTimer);
      saveTimer = window.setTimeout(() => api.savePetPosition(payload.x, payload.y), 400);
    });
    return () => {
      unBubble.then((f) => f());
      unMoved.then((f) => f());
    };
  }, []);

  const mood = snap?.mood ?? "idle";
  const name = snap?.config.petName ?? "Perch";

  return (
    <div
      className={`pet ${moodClass(mood)}`}
      title={name}
      onPointerDown={(e) => {
        if (e.button === 0) down.current = { x: e.screenX, y: e.screenY };
      }}
      onPointerMove={(e) => {
        const start = down.current;
        if (start && Math.hypot(e.screenX - start.x, e.screenY - start.y) > 4) {
          down.current = null;
          getCurrentWindow().startDragging();
        }
      }}
      onPointerUp={() => {
        if (down.current) {
          down.current = null;
          api.togglePanel();
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        api.showPetMenu();
      }}
    >
      {bubble && (
        <div className="bubble" role="status">
          <strong>{name}:</strong> {bubble}
        </div>
      )}
      <Critter badge={badgeFor(mood)} />
    </div>
  );
}
