import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import type { ThreadInfo } from "../shared/types";
import { makeSnapshot, makeThread } from "../test/fixtures";
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
  it("asks for the current layout on mount, since the startup event is sent before the page listens", async () => {
    fakeTransport({
      get_snapshot: () => makeSnapshot(),
      get_pet_sprite: () => "data:image/png;base64,AAAA",
      get_pet_placement: () => ({ cardsBelow: false, shiftX: -116, stageRoom: 300 }),
    });
    const { container } = render(<PetApp />);
    await screen.findByRole("button", { name: /idle/ });
    const stage = container.querySelector(".stage") as HTMLElement;
    await vi.waitFor(() => expect(stage.style.transform).toBe("translateX(-116px)"));
    expect(stage.style.getPropertyValue("--stage-room")).toBe("300px");
  });

  it("lets a live placement event win over a slower answer to that request", async () => {
    let answer: (p: unknown) => void = () => {};
    const { emit } = fakeTransport({
      get_snapshot: () => makeSnapshot(),
      get_pet_sprite: () => "data:image/png;base64,AAAA",
      get_pet_placement: () => new Promise((r) => (answer = r)),
    });
    const { container } = render(<PetApp />);
    await screen.findByRole("button", { name: /idle/ });
    act(() => emit("pet-placement", { cardsBelow: false, shiftX: 42, stageRoom: 400 }));
    await act(async () => answer({ cardsBelow: false, shiftX: -116, stageRoom: 300 }));
    expect((container.querySelector(".stage") as HTMLElement).style.transform).toBe("translateX(42px)");
  });

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

describe("PetApp notification clicks (v1.0 S1)", () => {
  async function setupWith(threads: ThreadInfo[]) {
    const fake = fakeTransport({
      get_snapshot: () => makeSnapshot({ threads }),
      get_pet_sprite: () => "data:image/png;base64,AAAA",
      load_conversation: () => [],
    });
    const utils = render(<PetApp />);
    await screen.findByRole("button", { name: /Mochi,/ });
    return { ...fake, ...utils };
  }

  it("opens an Ask thread's mini chat, like clicking its card", async () => {
    const ask = makeThread({ sessionId: "a1", source: "ask", project: "C:\\code\\app", projectName: "app", status: "ready" });
    const { emit } = await setupWith([ask]);
    act(() => emit("pet-open", { view: "thread", sessionId: "a1", project: "C:\\code\\app", source: "ask" }));
    expect(await screen.findByRole("region", { name: /^Conversation in app(:|$)/ })).toBeInTheDocument();
  });

  it("marks a Watch thread seen and opens its project, like clicking its card", async () => {
    const watch = makeThread({ sessionId: "w1", source: "watch", project: "C:\\code\\api", status: "ready" });
    const { emit, calls } = await setupWith([watch]);
    await act(async () => emit("pet-open", { view: "thread", sessionId: "w1", project: "C:\\code\\api", source: "watch" }));
    await vi.waitFor(() => expect(calls).toContainEqual(["open_project", { path: "C:\\code\\api" }]));
    expect(calls).toContainEqual(["mark_viewed", { sessionId: "w1" }]);
    expect(screen.queryByRole("region", { name: /Conversation in/ })).toBeNull();
  });

  it("still opens a Watch thread that has already left the cards", async () => {
    const { emit, calls } = await setupWith([]);
    await act(async () => emit("pet-open", { view: "thread", sessionId: "w9", project: "C:\\code\\api", source: "watch" }));
    await vi.waitFor(() => expect(calls).toContainEqual(["open_project", { path: "C:\\code\\api" }]));
  });

  it("opens each clicked notification's own thread", async () => {
    const one = makeThread({ sessionId: "a1", source: "ask", project: "C:\\code\\one", projectName: "one", status: "ready" });
    const two = makeThread({ sessionId: "a2", source: "ask", project: "C:\\code\\two", projectName: "two", status: "blocked" });
    const { emit } = await setupWith([one, two]);
    act(() => emit("pet-open", { view: "thread", sessionId: "a2", project: "C:\\code\\two", source: "ask" }));
    expect(await screen.findByRole("region", { name: /^Conversation in two(:|$)/ })).toBeInTheDocument();
    act(() => emit("pet-open", { view: "thread", sessionId: "a1", project: "C:\\code\\one", source: "ask" }));
    expect(await screen.findByRole("region", { name: /^Conversation in one(:|$)/ })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: /^Conversation in two(:|$)/ })).toBeNull();
  });
});
