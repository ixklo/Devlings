import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../shared/api";
import { IconCheck, IconRefresh } from "../shared/icons";
import type { PetInfo } from "../shared/types";
import { idleClip, SpriteView, usePetSprite, useReducedMotion } from "../sprite/SpriteView";
import { errorText } from "../shared/errors";
import { onRadioGroupKey, radioTabIndex } from "./controls";

const SOURCE_LABEL: Record<PetInfo["source"], string | null> = { bundled: null, perch: "Custom", codex: "Codex" };
const THUMB_SCALE = 0.4;

export function usePets() {
  const [pets, setPets] = useState<PetInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(() => {
    setError(null);
    api
      .listPets()
      .then(setPets)
      .catch((e) => setError(errorText(e)));
  }, []);
  useEffect(load, [load]);
  // Settings is hidden, not closed, between uses: look again each time it's shown, so a pet just added to the
  // pets folder appears without a restart (`list_pets` rescans the folders).
  useEffect(() => {
    const onVisibility = () => {
      if (document.visibilityState === "visible") load();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [load]);
  return { pets, error, reload: load };
}

function PetThumb({ pet, selected, tabIndex, onSelect }: { pet: PetInfo; selected: boolean; tabIndex: 0 | -1; onSelect: () => void }) {
  const src = usePetSprite(pet.id);
  const reduced = useReducedMotion();
  const source = SOURCE_LABEL[pet.source];
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      aria-label={pet.displayName}
      tabIndex={tabIndex}
      title={pet.description}
      className="pet-option"
      onClick={onSelect}
    >
      <span className="pet-option-stage">
        {src ? (
          <SpriteView src={src} scale={THUMB_SCALE} clip={idleClip(reduced)} aria-hidden="true" />
        ) : (
          <span className="pet-option-placeholder" />
        )}
      </span>
      <span className="pet-option-name">{pet.displayName}</span>
      {source && <span className="pet-option-source">{source}</span>}
      {selected && (
        <span className="pet-option-check" aria-hidden="true">
          <IconCheck size={11} strokeWidth={3} />
        </span>
      )}
    </button>
  );
}

interface Props {
  selectedId: string;
  onSelect: (id: string) => void;
}

/**
 * Grid of installed pets, each playing its idle animation. It scrolls once there are more pets than
 * fit (nine are built in), and brings the chosen one into view when it loads.
 */
export function PetPicker({ selectedId, onSelect }: Props) {
  const { pets, error, reload } = usePets();
  const grid = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Only when the list arrives: afterwards, focus (arrow keys, Tab) scrolls the grid by itself.
    grid.current?.querySelector<HTMLElement>('[aria-checked="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [pets]);
  if (error) {
    return (
      <div className="pet-grid-error" role="alert">
        <span>Couldn't load pets. {error}</span>
        <button type="button" className="btn btn-secondary btn-sm" onClick={reload}>
          <IconRefresh size={13} />
          Try again
        </button>
      </div>
    );
  }
  return (
    <div ref={grid} className="pet-grid" role="radiogroup" aria-label="Pet" aria-busy={!pets} onKeyDown={onRadioGroupKey}>
      {pets
        ? pets.map((p, i) => (
            <PetThumb
              key={p.id}
              pet={p}
              selected={p.id === selectedId}
              tabIndex={radioTabIndex(p.id === selectedId, i, pets.some((x) => x.id === selectedId))}
              onSelect={() => onSelect(p.id)}
            />
          ))
        : [0, 1, 2].map((i) => <span key={i} className="pet-option is-skeleton" aria-hidden="true" />)}
    </div>
  );
}
