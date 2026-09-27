import { useLayoutEffect, useRef, type RefObject } from "react";
import { cellPosition, frameAt, ROW_SPECS } from "./atlas";
import type { Clip } from "./petAnimation";
import { clockRuns, restingFrameAt, restLength } from "./spriteClock";

const pageHidden = () => typeof document !== "undefined" && document.visibilityState === "hidden";

/**
 * Plays a clip on the element in `ref` with the per-frame durations from the atlas table, by
 * setting its `background-position` directly: a frame change never re-renders React. Timers are
 * rescheduled from the clip's start time, so frames don't drift. Calls `onDone` once when a
 * one-shot clip finishes.
 *
 * Idle work is kept to a minimum: a resting clip (the idle row) wakes once per rest instead of
 * once per frame, a still clip (reduced motion) sets no timer at all, and nothing runs while the
 * page is hidden. A hidden window hides its page (design D17), so a hidden pet or settings window
 * costs nothing here. On show, the clip picks up where the clock says it should be.
 */
export function useSpriteClip(ref: RefObject<HTMLElement | null>, clip: Clip, scale: number, onDone?: () => void): void {
  const doneRef = useRef(onDone);
  doneRef.current = onDone;
  const scaleRef = useRef(scale);
  scaleRef.current = scale;
  // What the element shows now, so a new scale can redraw it without restarting the clip.
  const shown = useRef({ row: ROW_SPECS[clip.name].row, frame: 0 });

  useLayoutEffect(() => {
    const draw = (frame: number) => {
      shown.current = { row: ROW_SPECS[clip.name].row, frame };
      const el = ref.current;
      if (!el) return;
      el.style.backgroundPosition = cellPosition(shown.current.row, frame, scaleRef.current);
      el.dataset.frame = String(frame);
    };
    draw(0);
    if (clip.still) return;

    const durations = ROW_SPECS[clip.name].durations;
    let start = performance.now();
    let restMs = clip.rest ? restLength(Math.random()) : 0;
    let timer: number | undefined;
    let done = false;

    const tick = () => {
      timer = undefined;
      if (!clockRuns(clip, pageHidden(), done)) return;
      const now = performance.now();
      let next: { frame: number; nextIn: number; done: boolean };
      if (clip.rest) {
        let f = restingFrameAt(durations, now - start, restMs);
        if (f.cycleOver) {
          start = now;
          restMs = restLength(Math.random());
          f = restingFrameAt(durations, 0, restMs);
        }
        next = { frame: f.frame, nextIn: f.nextIn, done: false };
      } else {
        next = frameAt(durations, now - start, clip.loop);
      }
      if (next.frame !== shown.current.frame) draw(next.frame);
      if (next.done) {
        done = true;
        doneRef.current?.();
        return;
      }
      timer = window.setTimeout(tick, Math.max(16, next.nextIn));
    };

    const onVisibility = () => {
      if (pageHidden()) {
        window.clearTimeout(timer);
        timer = undefined;
      } else if (timer === undefined) {
        tick();
      }
    };

    document.addEventListener("visibilitychange", onVisibility);
    tick();
    return () => {
      done = true;
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [ref, clip.key, clip.name, clip.loop, clip.still, clip.rest]);

  // A new scale (Settings → size) redraws the frame that's showing; the clip keeps its place.
  useLayoutEffect(() => {
    const el = ref.current;
    if (el) el.style.backgroundPosition = cellPosition(shown.current.row, shown.current.frame, scale);
  }, [ref, scale]);
}
