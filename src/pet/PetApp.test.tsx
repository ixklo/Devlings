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

describe("PetApp hidden-threads count on the Show threads button", () => {
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

  const toolbar = () => screen.getByRole("toolbar", { name: "Pet controls" });
  const controlBar = () => document.querySelector(".control-bar") as HTMLElement;
  /** The bar's collapse button: "Show threads" or "Hide threads", with or without a count. */
  const chevron = () => within(toolbar()).getByRole("button", { name: /^(Show|Hide) threads/ });
  const bubbles = () => document.querySelector(".bubbles") as HTMLElement;

  it("never draws a badge on the pet, collapsed or not, in any tone", async () => {
    const cases: Partial<Snapshot>[] = [
      { threads: [makeThread({ status: "running" }), makeThread({ status: "ready" })] },
      { threads: [makeThread({ status: "needs_input" })] },
      { threads: [makeThread({ status: "blocked" })] },
      { approvals: [makeApproval()] },
    ];
    for (const over of cases) {
      for (const threadsCollapsed of [true, false]) {
        const { pet, unmount } = await setupSnap(makeSnapshot({ ...over, config: makeConfig({ threadsCollapsed }) }));
        expect(pet.children).toHaveLength(1);
        expect(pet.firstElementChild).toHaveClass("sprite");
        expect(pet).toHaveTextContent(/^$/);
        expect(document.querySelector(".pet-badge")).toBeNull();
        unmount();
      }
    }
  });

  it("shows two collapsed threads as a neutral 2 on the chevron, and nothing in the cards' place", async () => {
    const { pet } = await setupSnap(collapsedSnap({ threads: [makeThread(), makeThread({ status: "ready" })] }));
    const button = screen.getByRole("button", { name: "Show threads, 2 hidden" });
    expect(button).toBe(chevron());
    expect(button).toHaveTextContent(/^2$/);
    expect(button).toHaveClass("has-count", "is-neutral");
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(button.closest(".dock .control-bar")).not.toBeNull();
    expect(pet.contains(button)).toBe(false);
    expect(document.querySelector(".thread-card")).toBeNull();
    expect(document.querySelector(".count-pill")).toBeNull();
    expect(bubbles().children).toHaveLength(0);
  });

  it("turns amber and shows how many need you when a thread needs input", async () => {
    await setupSnap(collapsedSnap({ threads: [makeThread({ status: "needs_input" }), makeThread({ status: "blocked" })] }));
    expect(chevron()).toHaveAccessibleName("Show threads, 2 hidden, 1 needs you");
    expect(chevron()).toHaveAttribute("title", "Show threads, 2 hidden, 1 needs you");
    expect(chevron()).toHaveTextContent(/^1$/);
    expect(chevron()).toHaveClass("is-wait");
  });

  it("follows the snapshot as a hidden thread starts waiting", async () => {
    const t = makeThread({ sessionId: "s", status: "running" });
    const other = makeThread();
    const { emit } = await setupSnap(collapsedSnap({ threads: [t, other] }));
    expect(chevron()).toHaveClass("is-neutral");
    act(() => emit("snapshot", collapsedSnap({ threads: [{ ...t, status: "needs_input" }, other] })));
    expect(chevron()).toHaveAccessibleName("Show threads, 2 hidden, 1 needs you");
    expect(chevron()).toHaveClass("is-wait");
  });

  it("counts a hidden permission request as needing you", async () => {
    await setupSnap(collapsedSnap({ threads: [makeThread({ status: "running" })], approvals: [makeApproval()] }));
    expect(chevron()).toHaveAccessibleName("Show threads, 1 hidden, 1 needs you");
    expect(chevron()).toHaveTextContent(/^1$/);
    expect(chevron()).toHaveClass("is-wait");
    expect(screen.queryByRole("group", { name: /request in/ })).toBeNull();
    expect(bubbles().children).toHaveLength(0);
  });

  it("keeps the bar out of the way until you hover the pet or use it, whatever is running or hidden", async () => {
    const cases: Snapshot[] = [
      collapsedSnap(),
      collapsedSnap({ threads: [makeThread({ status: "running" }), makeThread({ status: "ready" })] }),
      collapsedSnap({ approvals: [makeApproval()] }),
      makeSnapshot({ threads: [makeThread({ status: "needs_input" })] }),
    ];
    for (const snap of cases) {
      const { unmount } = await setupSnap(snap);
      expect(controlBar()).not.toHaveClass("is-visible");
      expect(controlBar()).not.toHaveAttribute("data-hit");
      unmount();
    }
  });

  it("shows the bar, count and all, while the pointer is on the pet and a moment after it leaves", async () => {
    const { emit } = await setupSnap(collapsedSnap({ approvals: [makeApproval()] }));
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      act(() => emit("pet-pointer", "pet"));
      expect(controlBar()).toHaveClass("is-visible");
      expect(controlBar()).toHaveAttribute("data-hit", "bar");
      expect(chevron()).toHaveAccessibleName("Show threads, 1 needs you");
      expect(chevron()).toHaveClass("is-wait");
      act(() => emit("pet-pointer", null));
      expect(controlBar()).toHaveClass("is-visible");
      act(() => void vi.advanceTimersByTime(2000));
      expect(controlBar()).not.toHaveClass("is-visible");
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows the bar while the pet has keyboard focus or a card is open", async () => {
    const { pet } = await setupSnap(collapsedSnap({ threads: [makeThread()] }));
    act(() => pet.focus());
    expect(controlBar()).toHaveClass("is-visible");
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("region", { name: "New message" })).toBeInTheDocument();
    act(() => (document.activeElement as HTMLElement | null)?.blur());
    expect(controlBar()).toHaveClass("is-visible");
  });

  it("uses the error tone for a blocked thread when nothing needs input", async () => {
    await setupSnap(collapsedSnap({ threads: [makeThread({ status: "blocked" }), makeThread({ status: "ready" })] }));
    expect(chevron()).toHaveAccessibleName("Show threads, 2 hidden, 1 blocked");
    expect(chevron()).toHaveTextContent(/^1$/);
    expect(chevron()).toHaveClass("is-err");
  });

  it("is the plain chevron with nothing hidden, or while the cards are showing", async () => {
    const empty = await setupSnap(collapsedSnap());
    expect(chevron()).toHaveAccessibleName("Show threads");
    expect(chevron()).toHaveAttribute("class", "icon-btn");
    expect(chevron()).toHaveTextContent(/^$/);
    empty.unmount();
    await setupSnap(makeSnapshot({ threads: [makeThread({ status: "needs_input" })], approvals: [makeApproval()] }));
    expect(chevron()).toHaveAccessibleName("Hide threads");
    expect(chevron()).toHaveAttribute("class", "icon-btn");
    expect(chevron()).toHaveTextContent(/^$/);
    expect(document.querySelector(".thread-card")).not.toBeNull();
  });

  it("shows the threads on click", async () => {
    const { calls } = await setupSnap(collapsedSnap({ threads: [makeThread()] }));
    await userEvent.click(chevron());
    expect(calls).toContainEqual(["set_threads_collapsed", { collapsed: false }]);
  });

  it("adds nothing to the stack when collapsed: the update card stays nearest the pet, with the tail", async () => {
    const { unmount } = await setupSnap(collapsedSnap({ threads: [makeThread()], update: { state: "ready", version: "9.9.9" } }));
    expect(bubbles().children).toHaveLength(1);
    expect(bubbles().querySelector(".update-card")).not.toBeNull();
    const tails = document.querySelectorAll(".has-tail");
    expect(tails).toHaveLength(1);
    expect(tails[0]).toHaveClass("update-card");
    unmount();
    await setupSnap(
      collapsedSnap({
        threads: [makeThread({ status: "needs_input" })],
        setup: makeSetup({ needsSetup: true, hooksInstalled: false }),
        update: { state: "ready", version: "9.9.9" },
      }),
    );
    const order = Array.from(bubbles().children).map((el) =>
      el.matches(".setup-card") ? "setup" : el.querySelector(".update-card") ? "update" : "?",
    );
    expect(order).toEqual(["setup", "update"]);
    expect(document.querySelector(".count-pill, .more-pill")).toBeNull();
  });

  it("keeps the count while the composer is open, and Show threads goes back to the cards", async () => {
    const { pet, calls } = await setupSnap(collapsedSnap({ threads: [makeThread({ status: "blocked" })] }));
    pet.focus();
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("region", { name: "New message" })).toBeInTheDocument();
    expect(chevron()).toHaveAccessibleName("Show threads, 1 hidden, 1 blocked");
    await userEvent.click(chevron());
    expect(screen.queryByRole("region", { name: "New message" })).toBeNull();
    expect(calls).toContainEqual(["set_threads_collapsed", { collapsed: false }]);
  });
});
