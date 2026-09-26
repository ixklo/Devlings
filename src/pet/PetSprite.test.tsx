import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { PetSprite } from "./PetSprite";

const clip = { name: "idle" as const, loop: true, still: true, key: "still-idle" };

function setup(badge: { count: number; tone: "neutral" | "wait" | "err" } | null = null) {
  const fake = fakeTransport();
  const onActivate = vi.fn();
  const onHover = vi.fn();
  render(
    <PetSprite
      src="data:image/png;base64,AAAA"
      scale={0.5}
      clip={clip}
      onClipDone={() => {}}
      label="Mochi, idle"
      badge={badge}
      onActivate={onActivate}
      onHover={onHover}
    />,
  );
  return { ...fake, onActivate, onHover, pet: screen.getByRole("button", { name: "Mochi, idle" }) };
}

describe("PetSprite", () => {
  it("renders one pixelated cell of the atlas at the pet's scale", () => {
    const { pet } = setup();
    const sprite = pet.querySelector(".sprite") as HTMLElement;
    expect(sprite.style.width).toBe("96px");
    expect(sprite.style.height).toBe("104px");
    expect(sprite.style.backgroundSize).toBe("768px 936px");
    expect(sprite.style.backgroundPosition).toBe("0px 0px");
    expect(sprite.style.backgroundImage).toContain("data:image/png");
    expect(pet).toHaveAttribute("data-hit");
  });

  it("toggles the composer on a click without movement", () => {
    const { pet, onActivate, transport } = setup();
    fireEvent.pointerDown(pet, { button: 0, screenX: 100, screenY: 100 });
    fireEvent.pointerMove(pet, { screenX: 102, screenY: 101 });
    fireEvent.pointerUp(pet, { button: 0 });
    expect(onActivate).toHaveBeenCalledTimes(1);
    expect(transport.startDragging).not.toHaveBeenCalled();
  });

  it("starts a window drag after 4 px of movement, and that is not a click", () => {
    const { pet, onActivate, transport } = setup();
    fireEvent.pointerDown(pet, { button: 0, screenX: 100, screenY: 100 });
    fireEvent.pointerMove(pet, { screenX: 106, screenY: 100 });
    fireEvent.pointerUp(pet, { button: 0 });
    expect(transport.startDragging).toHaveBeenCalledTimes(1);
    expect(onActivate).not.toHaveBeenCalled();
  });

  it("opens the pet menu on right-click", () => {
    const { pet, commands } = setup();
    fireEvent.contextMenu(pet);
    expect(commands()).toEqual(["show_pet_menu"]);
  });

  it("reports hover for the wave", () => {
    const { pet, onHover } = setup();
    fireEvent.pointerEnter(pet);
    expect(onHover).toHaveBeenCalled();
  });

  it("shows the thread count badge when collapsed", () => {
    setup({ count: 4, tone: "wait" });
    expect(document.querySelector(".pet-badge")).toHaveTextContent("4");
  });
});
