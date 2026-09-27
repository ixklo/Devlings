import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PetInfo } from "../shared/types";
import { fakeTransport } from "../test/fakeTransport";
import { PetPicker } from "./PetPicker";

// The nine bundled pets, in the order `list_pets` returns them.
const BUNDLED: PetInfo[] = [
  ["perch", "Perch"],
  ["ember", "Ember"],
  ["plum", "Plum"],
  ["fox", "Pip"],
  ["cat", "Miso"],
  ["axolotl", "Nori"],
  ["capybara", "Bean"],
  ["robot", "Bolt"],
  ["ghost", "Wisp"],
].map(([id, displayName]) => ({ id, displayName, description: `${displayName} the pet.`, source: "bundled" }));
const NAMES = BUNDLED.map((p) => p.displayName);

function renderPicker(selectedId = "perch") {
  const fake = fakeTransport({ list_pets: () => BUNDLED, get_pet_sprite: () => "data:image/png;base64," });
  const onSelect = vi.fn();
  const utils = render(<PetPicker selectedId={selectedId} onSelect={onSelect} />);
  return { ...fake, ...utils, onSelect };
}

const scrollIntoView = Element.prototype.scrollIntoView;
afterEach(() => {
  Element.prototype.scrollIntoView = scrollIntoView;
});

describe("PetPicker", () => {
  it("shows all nine bundled pets in one radio group, in order", async () => {
    renderPicker();
    const group = screen.getByRole("radiogroup", { name: "Pet" });
    await waitFor(() => expect(screen.getAllByRole("radio")).toHaveLength(9));
    expect(screen.getAllByRole("radio").map((r) => r.getAttribute("aria-label"))).toEqual(NAMES);
    expect(group).toHaveClass("pet-grid");
    expect(screen.getByRole("radio", { name: "Perch" })).toBeChecked();
    // One Tab stop: the chosen pet.
    expect(screen.getAllByRole("radio").filter((r) => r.tabIndex === 0)).toEqual([screen.getByRole("radio", { name: "Perch" })]);
  });

  it("lets arrow keys move across all nine and choose each, wrapping at the ends", async () => {
    const { onSelect } = renderPicker();
    const perch = await screen.findByRole("radio", { name: "Perch" });
    perch.focus();
    for (const name of NAMES.slice(1)) {
      await userEvent.keyboard("{ArrowRight}");
      expect(screen.getByRole("radio", { name })).toHaveFocus();
    }
    expect(onSelect).toHaveBeenLastCalledWith("ghost");
    expect(onSelect).toHaveBeenCalledTimes(8);
    await userEvent.keyboard("{ArrowDown}");
    expect(perch).toHaveFocus();
    expect(onSelect).toHaveBeenLastCalledWith("perch");
    await userEvent.keyboard("{ArrowUp}");
    expect(screen.getByRole("radio", { name: "Wisp" })).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(perch).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(screen.getByRole("radio", { name: "Wisp" })).toHaveFocus();
    expect(onSelect).toHaveBeenLastCalledWith("ghost");
  });

  it("scrolls the chosen pet into view when the grid loads", async () => {
    const scrolled: string[] = [];
    Element.prototype.scrollIntoView = vi.fn(function (this: Element) {
      scrolled.push(this.getAttribute("aria-label") ?? "");
    });
    renderPicker("ghost");
    await waitFor(() => expect(scrolled).toEqual(["Wisp"]));
    expect(screen.getByRole("radio", { name: "Wisp" })).toHaveAttribute("tabindex", "0");
  });
});
