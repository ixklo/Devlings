import { useEffect, useRef, useState } from "react";
import { frameAt, ROW_SPECS } from "./atlas";
import type { Clip } from "./petAnimation";

/**
 * Plays a clip with the per-frame durations from the atlas table and returns
 * the current column. Timers are rescheduled from the clip's start time, so
 * frames don't drift. Calls `onDone` once when a one-shot clip finishes.
 */
export function useSpriteClip(clip: Clip, onDone?: () => void): number {
  const [state, setState] = useState({ key: clip.key, frame: 0 });
  const doneRef = useRef(onDone);
  doneRef.current = onDone;

  useEffect(() => {
    if (clip.still) {
      setState({ key: clip.key, frame: 0 });
      return;
    }
    const durations = ROW_SPECS[clip.name].durations;
    const start = performance.now();
    let timer: number | undefined;
    let alive = true;
    const tick = () => {
      if (!alive) return;
      const f = frameAt(durations, performance.now() - start, clip.loop);
      setState((prev) => (prev.key === clip.key && prev.frame === f.frame ? prev : { key: clip.key, frame: f.frame }));
      if (f.done) {
        doneRef.current?.();
        return;
      }
      timer = window.setTimeout(tick, Math.max(16, f.nextIn));
    };
    tick();
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [clip.key, clip.name, clip.loop, clip.still]);

  return state.key === clip.key ? state.frame : 0;
}
