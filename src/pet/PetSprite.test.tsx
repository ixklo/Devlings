import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { DRAG_FLUSH_MS, PetSprite } from "./PetSprite";

const clip = { name: "idle" as const, loop: true, still: true, key: "still-idle" };

function setup() {
  const fake = fakeTransport();
  const onActivate = vi.fn();
  const onHover = vi.fn();
  const onDragStart = vi.fn();
  const onDragEnd = vi.fn();
  render(
    <PetSprite
      src="data:image/png;base64,AAAA"
      scale={0.5}
      clip={clip}
      onClipDone={() => {}}
      label="Mochi, idle"
      onActivate={onActivate}
      onHover={onHover}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
    />,
  );
  return { ...fake, onActivate, onHover, onDragStart, onDragEnd, pet: screen.getByRole("button", { name: "Mochi, idle" }) };
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

  it("drags the window itself after 4 px, batching moves, and that is not a click", async () => {
    const { pet, onActivate, transport, calls, onDragEnd } = setup();
    fireEvent.pointerDown(pet, { button: 0, screenX: 100, screenY: 100 });
    fireEvent.pointerMove(pet, { screenX: 106, screenY: 100 });
    fireEvent.pointerMove(pet, { screenX: 110, screenY: 97 });
    await new Promise((r) => setTimeout(r, DRAG_FLUSH_MS * 3));
    expect(calls.filter(([c]) => c === "drag_pet_by")).toEqual([["drag_pet_by", { dx: 10, dy: -3 }]]);
    fireEvent.pointerMove(pet, { screenX: 112, screenY: 97 });
    fireEvent.pointerUp(pet, { button: 0 });
    // Release flushes what's left right away.
    const drags = calls.filter(([c]) => c === "drag_pet_by");
    expect(drags[drags.length - 1]).toEqual(["drag_pet_by", { dx: 2, dy: 0 }]);
    expect(onDragEnd).toHaveBeenCalledTimes(1);
    expect(onActivate).not.toHaveBeenCalled();
    expect(transport.startDragging).not.toHaveBeenCalled();
  });

  it("reports the drag direction when a drag starts", () => {
    const right = setup();
    fireEvent.pointerDown(right.pet, { button: 0, screenX: 100, screenY: 100 });
    fireEvent.pointerMove(right.pet, { screenX: 106, screenY: 100 });
    expect(right.onDragStart).toHaveBeenCalledWith("right");
    cleanup();
    const left = setup();
    fireEvent.pointerDown(left.pet, { button: 0, screenX: 100, screenY: 100 });
    fireEvent.pointerMove(left.pet, { screenX: 94, screenY: 102 });
    expect(left.onDragStart).toHaveBeenCalledWith("left");
  });

  it("ends the drag if the pointer is cancelled", () => {
    const { pet, onDragEnd, onActivate } = setup();
    fireEvent.pointerDown(pet, { button: 0, screenX: 100, screenY: 100 });
    fireEvent.pointerMove(pet, { screenX: 120, screenY: 100 });
    fireEvent.pointerCancel(pet);
    expect(onDragEnd).toHaveBeenCalledTimes(1);
    fireEvent.pointerUp(pet, { button: 0 });
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

  it("draws nothing over the pet's art (the hidden-threads count is on the Show threads button)", () => {
    const { pet } = setup();
    expect(pet.children).toHaveLength(1);
    expect(pet.firstElementChild).toHaveClass("sprite");
    expect(document.querySelector(".pet-badge")).toBeNull();
  });
});
