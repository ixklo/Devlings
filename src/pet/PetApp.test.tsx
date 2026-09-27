import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import type { Snapshot, ThreadInfo } from "../shared/types";
import { makeApproval, makeConfig, makeSetup, makeSnapshot, makeThread } from "../test/fixtures";
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

describe("PetApp hidden-threads pill", () => {
  const collapsedSnap = (over: Partial<Snapshot> = {}) =>
    makeSnapshot({ config: makeConfig({ threadsCollapsed: true }), ...over });

  async function setupSnap(snap: Snapshot) {
    const fake = fakeTransport({
      get_snapshot: () => snap,
      get_pet_sprite: () => "data:image/png;base64,AAAA",
      set_threads_collapsed: () => snap,
    });
    const utils = render(<PetApp />);
    const pet = await screen.findByRole("button", { name: /^Mochi, / });
    return { ...fake, ...utils, pet };
  }

  const pill = () => document.querySelector<HTMLButtonElement>(".count-pill");

  it("never draws a badge on the pet, collapsed or not, in any tone", async () => {
    const tones: ThreadInfo[][] = [
      [makeThread({ status: "running" }), makeThread({ status: "ready" })],
      [makeThread({ status: "needs_input" })],
      [makeThread({ status: "blocked" })],
    ];
    for (const threads of tones) {
      for (const threadsCollapsed of [true, false]) {
        const { pet, unmount } = await setupSnap(makeSnapshot({ threads, config: makeConfig({ threadsCollapsed }) }));
        expect(pet.children).toHaveLength(1);
        expect(pet.firstElementChild).toHaveClass("sprite");
        expect(pet).toHaveTextContent(/^$/);
        expect(document.querySelector(".pet-badge")).toBeNull();
        unmount();
      }
    }
  });

  it("folds two collapsed threads into a neutral pill in the cards' place", async () => {
    const { pet } = await setupSnap(collapsedSnap({ threads: [makeThread(), makeThread({ status: "ready" })] }));
    const button = screen.getByRole("button", { name: "2 threads hidden. Show threads." });
    expect(button).toBe(pill());
    expect(button).toHaveTextContent(/^2 threads$/);
    expect(button).toHaveClass("is-neutral");
    expect(button.closest(".stage .bubbles")).not.toBeNull();
    expect(pet.contains(button)).toBe(false);
    expect(document.querySelector(".thread-card")).toBeNull();
  });

  it("turns amber and says how many need you when a thread needs input", async () => {
    await setupSnap(collapsedSnap({ threads: [makeThread({ status: "needs_input" }), makeThread({ status: "blocked" })] }));
    expect(pill()).toHaveClass("is-wait");
    expect(pill()).toHaveTextContent(/^1 needs you$/);
    expect(pill()).toHaveAccessibleName("2 threads hidden, 1 needs you. Show threads.");
  });

  it("counts a hidden permission request as needing you", async () => {
    await setupSnap(collapsedSnap({ threads: [makeThread({ status: "running" })], approvals: [makeApproval()] }));
    expect(pill()).toHaveClass("is-wait");
    expect(pill()).toHaveTextContent(/^1 needs you$/);
    expect(screen.queryByRole("group", { name: /request in/ })).toBeNull();
  });

  it("uses the error tone for a blocked thread when nothing needs input", async () => {
    await setupSnap(collapsedSnap({ threads: [makeThread({ status: "blocked" }), makeThread({ status: "ready" })] }));
    expect(pill()).toHaveClass("is-err");
    expect(pill()).toHaveTextContent(/^1 blocked$/);
  });

  it("shows no pill without threads, or while the cards are showing", async () => {
    const empty = await setupSnap(collapsedSnap());
    expect(pill()).toBeNull();
    empty.unmount();
    await setupSnap(makeSnapshot({ threads: [makeThread({ status: "needs_input" })] }));
    expect(pill()).toBeNull();
    expect(document.querySelector(".thread-card")).not.toBeNull();
  });

  it("yields to the composer like the cards do, and comes back when it closes", async () => {
    const { pet } = await setupSnap(collapsedSnap({ threads: [makeThread()] }));
    expect(pill()).not.toBeNull();
    pet.focus();
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("region", { name: "New message" })).toBeInTheDocument();
    expect(pill()).toBeNull();
    await userEvent.keyboard("{Escape}");
    expect(pill()).not.toBeNull();
  });

  it("sits nearest the pet in the flipped stack when the cards open below it", async () => {
    const { container, emit } = await setupSnap(collapsedSnap({ threads: [makeThread(), makeThread()] }));
    act(() => emit("pet-placement", { cardsBelow: true, shiftX: 24, stageRoom: 300 }));
    expect(container.querySelector(".overlay")).toHaveAttribute("data-cards-below", "true");
    const stage = container.querySelector(".stage") as HTMLElement;
    // The stage (not the dock) is what flips below the pet and shifts sideways with it.
    expect(stage.style.transform).toBe("translateX(24px)");
    const bubbles = stage.querySelector(".bubbles") as HTMLElement;
    expect(bubbles.firstElementChild?.contains(pill())).toBe(true);
    expect(container.querySelector(".dock")?.contains(pill())).toBe(false);
  });

  it("keeps the setup card nearest the pet and in charge of the tail, and stays below the update card", async () => {
    await setupSnap(
      collapsedSnap({
        threads: [makeThread()],
        setup: makeSetup({ needsSetup: true, hooksInstalled: false }),
        update: { state: "ready", version: "9.9.9" },
      }),
    );
    const bubbles = document.querySelector(".bubbles") as HTMLElement;
    const order = Array.from(bubbles.children).map((el) =>
      el.matches(".setup-card") ? "setup" : el.querySelector(".count-pill") ? "pill" : el.querySelector(".update-card") ? "update" : "?",
    );
    expect(order).toEqual(["setup", "pill", "update"]);
    const tails = document.querySelectorAll(".has-tail");
    expect(tails).toHaveLength(1);
    expect(tails[0]).toHaveClass("setup-card");
  });

  it("leaves the update card without the tail while the pill sits between it and the pet", async () => {
    await setupSnap(collapsedSnap({ threads: [makeThread()], update: { state: "ready", version: "9.9.9" } }));
    expect(document.querySelectorAll(".has-tail")).toHaveLength(0);
  });

  it("shows the threads on click, like the chevron", async () => {
    const { calls } = await setupSnap(collapsedSnap({ threads: [makeThread()] }));
    await userEvent.click(pill()!);
    expect(calls).toContainEqual(["set_threads_collapsed", { collapsed: false }]);
  });

  it("shows the threads from the keyboard and hands focus to the pet as it goes", async () => {
    for (const key of ["{Enter}", " "]) {
      const { calls, pet, unmount } = await setupSnap(collapsedSnap({ threads: [makeThread()] }));
      pill()!.focus();
      await userEvent.keyboard(key);
      expect(calls.filter(([c]) => c === "set_threads_collapsed")).toEqual([["set_threads_collapsed", { collapsed: false }]]);
      expect(pet).toHaveFocus();
      expect(screen.queryByRole("region", { name: "New message" })).toBeNull();
      unmount();
    }
  });

  it("is reported as a hit region, so a click lands in the click-through window", async () => {
    const { calls } = await setupSnap(collapsedSnap({ threads: [makeThread()] }));
    const button = pill()!;
    expect(button).toHaveAttribute("data-hit", "");
    button.getBoundingClientRect = () => ({ x: 130, y: 40, left: 130, top: 40, width: 90, height: 24, right: 220, bottom: 64, toJSON: () => ({}) }) as DOMRect;
    // Nudge the collector (it also watches the DOM and layout).
    act(() => window.dispatchEvent(new Event("resize")));
    await vi.waitFor(() => {
      const sent = calls.filter(([c]) => c === "set_hit_regions").map(([, a]) => (a as { regions: unknown[] }).regions);
      expect(sent[sent.length - 1]).toContainEqual({ x: 130, y: 40, w: 90, h: 24 });
    });
    expect(within(document.querySelector(".bubbles") as HTMLElement).getByRole("button")).toBe(button);
  });
});
