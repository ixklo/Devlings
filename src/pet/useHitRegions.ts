import { useCallback, useEffect, useRef } from "react";
import type { HitRect } from "../shared/types";

export const HIT_THROTTLE_MS = 60;

/** Window-relative rectangles of every `[data-hit]` element that is on screen. */
export function collectHitRects(root: ParentNode = document): HitRect[] {
  const rects: HitRect[] = [];
  root.querySelectorAll<HTMLElement>("[data-hit]").forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return;
    rects.push({ x: Math.round(r.left), y: Math.round(r.top), w: Math.round(r.width), h: Math.round(r.height) });
  });
  return rects;
}

/**
 * The single collector for click-through: watches layout (ResizeObserver),
 * DOM changes (MutationObserver) and finished transitions, and reports the
 * interactive rectangles at most every 60 ms, skipping unchanged sets.
 * Returns a `refresh` to call after state changes.
 */
export function useHitRegions(send: (regions: HitRect[]) => void): () => void {
  const sendRef = useRef(send);
  sendRef.current = send;
  const scheduleRef = useRef<() => void>(() => {});

  useEffect(() => {
    let timer: number | undefined;
    let lastAt = -Infinity;
    let lastKey = "";

    const flush = () => {
      timer = undefined;
      lastAt = performance.now();
      const rects = collectHitRects();
      const key = JSON.stringify(rects);
      if (key === lastKey) return;
      lastKey = key;
      sendRef.current(rects);
    };
    const schedule = () => {
      if (timer !== undefined) return;
      timer = window.setTimeout(flush, Math.max(0, HIT_THROTTLE_MS - (performance.now() - lastAt)));
    };
    scheduleRef.current = schedule;

    const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(schedule) : null;
    const observeAll = () => {
      if (!ro) return;
      ro.disconnect();
      ro.observe(document.body);
      document.querySelectorAll("[data-hit]").forEach((el) => ro.observe(el));
    };
    const mo =
      typeof MutationObserver !== "undefined"
        ? new MutationObserver(() => {
            observeAll();
            schedule();
          })
        : null;
    // Not "style": the sprite restyles every frame and never moves because of it.
    mo?.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["data-hit", "class"] });
    observeAll();

    const events = ["transitionend", "animationend", "scroll"] as const;
    events.forEach((e) => document.addEventListener(e, schedule, true));
    window.addEventListener("resize", schedule);
    schedule();

    return () => {
      window.clearTimeout(timer);
      ro?.disconnect();
      mo?.disconnect();
      events.forEach((e) => document.removeEventListener(e, schedule, true));
      window.removeEventListener("resize", schedule);
      scheduleRef.current = () => {};
    };
  }, []);

  return useCallback(() => scheduleRef.current(), []);
}
