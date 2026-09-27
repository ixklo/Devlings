import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CELL_H, ROW_SPECS } from "../sprite/atlas";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import { PetApp } from "./PetApp";

const row = (sprite: HTMLElement) => Math.abs(parseFloat(sprite.style.backgroundPosition.split(" ")[1])) / (CELL_H * 0.6);

async function setup() {
  const fake = fakeTransport({ get_snapshot: () => makeSnapshot(), get_pet_sprite: () => "data:image/png;base64,AAAA" });
  const utils = render(<PetApp />);
  const pet = await screen.findByRole("button", { name: /idle/ });
  return { ...fake, ...utils, sprite: pet.querySelector(".sprite") as HTMLElement };
}

describe("welcome back (design v1.2)", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("waves once when the backend says the user is back", async () => {
    const { emit, sprite } = await setup();
    expect(Math.round(row(sprite))).toBe(ROW_SPECS.idle.row);
    act(() => emit("pet-welcome", null));
    expect(Math.round(row(sprite))).toBe(ROW_SPECS.waving.row);
  });

  it("stays still with reduced motion", async () => {
    vi.stubGlobal("matchMedia", (q: string) => ({
      matches: q.includes("reduce"),
      addEventListener: () => {},
      removeEventListener: () => {},
    }));
    const { emit, sprite } = await setup();
    act(() => emit("pet-welcome", null));
    expect(Math.round(row(sprite))).toBe(ROW_SPECS.idle.row);
  });
});
