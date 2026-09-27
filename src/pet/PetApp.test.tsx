import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import { PetApp } from "./PetApp";
import { HIT_THROTTLE_MS } from "./useHitRegions";

/**
 * Renders the pet overlay and waits for the first snapshot (PetApp renders nothing until then),
 * by which point its `pet-placement` listener (registered synchronously during mount, design D9)
 * is already in place.
 */
async function setup() {
  const fake = fakeTransport({
    get_snapshot: () => makeSnapshot(),
    get_pet_sprite: () => "data:image/png;base64,AAAA",
  });
  const utils = render(<PetApp />);
  await screen.findByRole("button", { name: /idle/ });
  return { ...fake, ...utils };
}

describe("PetApp placement (design D9)", () => {
  it("defaults to cards above the sprite, and flips via data-cards-below on .overlay", async () => {
    const { container, emit } = await setup();
    const overlay = container.querySelector(".overlay");
    expect(overlay).toHaveAttribute("data-cards-below", "false");

    act(() => emit("pet-placement", { cardsBelow: true, shiftX: 0, stageRoom: 400 }));
    expect(overlay).toHaveAttribute("data-cards-below", "true");

    act(() => emit("pet-placement", { cardsBelow: false, shiftX: 0, stageRoom: 400 }));
    expect(overlay).toHaveAttribute("data-cards-below", "false");
  });

  it("applies shiftX as a translateX transform on .stage, generic to whatever card is showing", async () => {
    const { container, emit } = await setup();
    const stage = container.querySelector(".stage") as HTMLElement;
    expect(stage.style.transform).toBe("");

    act(() => emit("pet-placement", { cardsBelow: false, shiftX: 42, stageRoom: 400 }));
    expect(stage.style.transform).toBe("translateX(42px)");

    act(() => emit("pet-placement", { cardsBelow: true, shiftX: -18, stageRoom: 400 }));
    expect(stage.style.transform).toBe("translateX(-18px)");

    act(() => emit("pet-placement", { cardsBelow: false, shiftX: 0, stageRoom: 400 }));
    expect(stage.style.transform).toBe("");
  });

  it("applies stageRoom as the --stage-room custom property on .stage, unset until the first placement", async () => {
    const { container, emit } = await setup();
    const stage = container.querySelector(".stage") as HTMLElement;
    expect(stage.style.getPropertyValue("--stage-room")).toBe("");

    act(() => emit("pet-placement", { cardsBelow: false, shiftX: 0, stageRoom: 344 }));
    expect(stage.style.getPropertyValue("--stage-room")).toBe("344px");

    act(() => emit("pet-placement", { cardsBelow: true, shiftX: 0, stageRoom: 131.2 }));
    expect(stage.style.getPropertyValue("--stage-room")).toBe("131.2px");
  });

  it("keeps reporting hit regions after a placement flip", async () => {
    const { calls, emit } = await setup();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    try {
      act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS));
      const reports = () => calls.filter(([cmd]) => cmd === "set_hit_regions").length;
      expect(reports()).toBeGreaterThan(0);

      const before = reports();
      act(() => emit("pet-placement", { cardsBelow: true, shiftX: 5, stageRoom: 400 }));
      act(() => void vi.advanceTimersByTime(HIT_THROTTLE_MS));
      expect(reports()).toBeGreaterThanOrEqual(before);
    } finally {
      vi.useRealTimers();
    }
  });
});
