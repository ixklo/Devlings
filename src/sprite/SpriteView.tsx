import { useEffect, useRef, useState, type HTMLAttributes } from "react";
import { api } from "../shared/api";
import { cellBox } from "./atlas";
import { IDLE_REST, type Clip } from "./petAnimation";
import { placeholderAtlas } from "./placeholderAtlas";
import { useSpriteClip } from "./useSpriteClip";

interface SpriteViewProps extends HTMLAttributes<HTMLDivElement> {
  src: string | null;
  scale: number;
  clip: Clip;
  onClipDone?: () => void;
}

/**
 * One animated cell of a pet atlas, drawn as a pixelated CSS background. React renders the cell's
 * size and sheet; `useSpriteClip` moves `background-position` from frame to frame on its own,
 * so an animating pet never re-renders.
 */
export function SpriteView({ src, scale, clip, onClipDone, className, style, ...rest }: SpriteViewProps) {
  const ref = useRef<HTMLDivElement>(null);
  useSpriteClip(ref, clip, scale, onClipDone);
  return (
    <div
      {...rest}
      ref={ref}
      className={`sprite${className ? ` ${className}` : ""}`}
      data-row={clip.name}
      style={{ ...cellBox(scale), backgroundImage: src ? `url("${src}")` : undefined, ...style }}
    />
  );
}

const spriteCache = new Map<string, Promise<string>>();
const DECODE_TIMEOUT_MS = 3000;

/** Resolves when the image decodes; rejects if the browser can't read it. */
function decodes(src: string): Promise<string> {
  if (typeof Image === "undefined") return Promise.resolve(src);
  return new Promise((resolve, reject) => {
    const img = new Image();
    // Environments that never load images (tests) shouldn't hang the pet.
    const t = window.setTimeout(() => resolve(src), DECODE_TIMEOUT_MS);
    img.onload = () => (window.clearTimeout(t), resolve(src));
    img.onerror = () => (window.clearTimeout(t), reject(new Error("The pet's spritesheet couldn't be read.")));
    img.src = src;
  });
}

/**
 * Loads a pet's atlas as a data URL (cached per id). Any failure, including
 * "No pets found." or a sheet the browser can't decode, falls back to the
 * drawn placeholder so the pet never vanishes.
 */
export function loadPetSprite(id: string): Promise<string> {
  let p = spriteCache.get(id);
  if (!p) {
    p = api
      .getPetSprite(id)
      .then(decodes)
      .catch((e) => {
        spriteCache.delete(id);
        const fallback = placeholderAtlas(id || "perch");
        if (fallback) return fallback;
        throw e;
      });
    spriteCache.set(id, p);
  }
  return p;
}

export function usePetSprite(id: string | null | undefined): string | null {
  const [state, setState] = useState<{ id: string; src: string } | null>(null);
  useEffect(() => {
    if (id == null) return;
    let alive = true;
    loadPetSprite(id)
      .then((src) => alive && setState({ id, src }))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [id]);
  // Keep showing the previous pet until the new sheet arrives, so switching never blinks.
  return state?.src ?? null;
}

const REDUCED = "(prefers-reduced-motion: reduce)";

export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() => typeof window !== "undefined" && !!window.matchMedia?.(REDUCED).matches);
  useEffect(() => {
    const mq = window.matchMedia?.(REDUCED);
    if (!mq) return;
    const on = () => setReduced(mq.matches);
    mq.addEventListener?.("change", on);
    return () => mq.removeEventListener?.("change", on);
  }, []);
  return reduced;
}

/** The idle loop (resting between plays), used by pet pickers and avatars. */
export function idleClip(reducedMotion: boolean): Clip {
  return reducedMotion
    ? { name: "idle", loop: false, still: true, key: "still-idle" }
    : { name: "idle", loop: true, still: false, rest: IDLE_REST, key: "idle" };
}
