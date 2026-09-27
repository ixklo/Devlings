import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../shared/api";

/** How long answer buttons must sit visible and still before a click counts. */
export const ARM_MS = 800;
/** How often their position is re-checked. */
export const ARM_CHECK_MS = 100;
/** A press that began before the buttons armed. */
const PRESSED_UNARMED = -1;

function sameRect(a: DOMRect, b: DOMRect): boolean {
  return (
    Math.abs(a.left - b.left) < 0.5 &&
    Math.abs(a.top - b.top) < 0.5 &&
    Math.abs(a.width - b.width) < 0.5 &&
    Math.abs(a.height - b.height) < 0.5
  );
}

/** An entrance (or any) animation running on the element or the few boxes around it. */
function animating(el: Element): boolean {
  let n: Element | null = el;
  for (let i = 0; n && i < 5; i++, n = n.parentElement) {
    if (n.getAnimations?.().some((a) => a.playState === "running")) return true;
  }
  return false;
}

/**
 * Guards answer buttons against clicks meant for something else: a card can
 * appear, or slide, under the cursor. The buttons count only once they have
 * been visible and in the same place for ARM_MS; any move (a sibling card
 * leaving, an entrance animation, the cards flipping below the pet, the window
 * moving, `resetKey` changing) disarms them until they're still again. A mouse
 * click counts only when its pointerdown also came after arming; a keyboard
 * press on a deliberately focused button counts once armed.
 */
export function useArming<T extends HTMLElement>(resetKey?: unknown) {
  const ref = useRef<T>(null);
  const [armed, setArmed] = useState(false);
  const st = useRef({ rect: null as DOMRect | null, stableSince: 0, armedAt: 0, downAt: 0 });

  /** Re-measures, updates the armed state, and returns whether the buttons are armed right now. */
  const check = useCallback((): boolean => {
    const el = ref.current;
    const s = st.current;
    if (!el) return false;
    const now = Date.now();
    const rect = el.getBoundingClientRect();
    const moved = !s.rect || !sameRect(rect, s.rect) || animating(el) || document.visibilityState === "hidden";
    s.rect = rect;
    if (moved) {
      s.stableSince = now;
      if (s.armedAt) {
        s.armedAt = 0;
        setArmed(false);
      }
      return false;
    }
    if (!s.armedAt && now - s.stableSince >= ARM_MS) {
      s.armedAt = now;
      setArmed(true);
    }
    return s.armedAt > 0;
  }, []);

  const disarm = useCallback(() => {
    const s = st.current;
    s.rect = null;
    s.armedAt = 0;
    // A press already under way now began before the buttons (re-)arm.
    if (s.downAt !== 0) s.downAt = PRESSED_UNARMED;
    s.stableSince = Date.now();
    setArmed(false);
  }, []);

  // Layout around the card changed (e.g. the cards flipped below the pet).
  useLayoutEffect(() => {
    disarm();
    check();
  }, [resetKey, disarm, check]);

  // After any render: siblings may have moved it.
  useLayoutEffect(() => {
    check();
  });

  useEffect(() => {
    const timer = window.setInterval(check, ARM_CHECK_MS);
    const unlisten = api.onMoved(() => disarm());
    return () => {
      window.clearInterval(timer);
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [check, disarm]);

  // downAt: 0 = no press on these buttons, PRESSED_UNARMED = pressed before they armed, else when an armed press began.
  const onPointerDown = useCallback(() => {
    st.current.downAt = check() ? Date.now() : PRESSED_UNARMED;
  }, [check]);

  /**
   * Whether this click may answer. Call it first thing in the click handler. A click whose press began before
   * the buttons armed is refused. A click with no press on them at all comes from the keyboard or from
   * assistive technology (screen readers and voice control fire a click with no pointerdown); a real mouse
   * click can't land on a button without pressing on it, so those count once armed.
   */
  const accept = useCallback(
    (e: { detail: number }): boolean => {
      const s = st.current;
      const noPress = e.detail === 0 || s.downAt === 0;
      const ok = check() && (noPress || (s.downAt > 0 && s.downAt >= s.armedAt));
      s.downAt = 0;
      return ok;
    },
    [check],
  );

  return { ref, armed, onPointerDown, accept };
}
