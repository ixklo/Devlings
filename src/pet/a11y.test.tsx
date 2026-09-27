import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Snapshot } from "../shared/types";
import { fakeTransport } from "../test/fakeTransport";
import { makeApproval, makeConfig, makeSnapshot, makeThread } from "../test/fixtures";
import { Messages } from "./Messages";
import { PetApp } from "./PetApp";

async function setup(first: Snapshot = makeSnapshot()) {
  const fake = fakeTransport({
    get_snapshot: () => first,
    get_pet_sprite: () => "data:image/png;base64,AAAA",
  });
  const utils = render(<PetApp />);
  const pet = await screen.findByRole("button", { name: /^Mochi, / });
  return { ...fake, ...utils, pet };
}

const liveRegion = () => document.querySelector<HTMLElement>('[role="status"][aria-live="polite"]');

describe("pet window keyboard", () => {
  it("makes the pet a focusable button that Enter opens the composer from, and Esc closes it back to the pet", async () => {
    const { pet } = await setup();
    expect(pet).toHaveAttribute("tabindex", "0");
    expect(pet).toHaveAccessibleDescription(/Press Enter to ask\. Arrow keys move Mochi/);
    pet.focus();
    await userEvent.keyboard("{Enter}");
    const box = await screen.findByRole("textbox", { name: "Message" });
    expect(box).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("textbox", { name: "Message" })).not.toBeInTheDocument();
    expect(pet).toHaveFocus();
  });

  it("activates the pet with Space too", async () => {
    const { pet } = await setup();
    pet.focus();
    await userEvent.keyboard(" ");
    expect(await screen.findByRole("region", { name: "New message" })).toBeInTheDocument();
  });

  it("tabs through cards, then the pet, then the control bar", async () => {
    const t = makeThread({ projectName: "api-server", status: "needs_input", label: "Waiting" });
    const { pet } = await setup(makeSnapshot({ threads: [t] }));
    await userEvent.tab();
    expect(screen.getByRole("button", { name: /^api-server, Needs you/ })).toHaveFocus();
    await userEvent.tab();
    expect(pet).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "New message" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Notifications" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Hide threads" })).toHaveFocus();
  });

  it("puts the hidden-threads count on Show threads, with no Tab stop before the pet, and keeps focus there as the threads show", async () => {
    const threads = [makeThread({ status: "needs_input" }), makeThread({ status: "running" })];
    const collapsed = makeSnapshot({ threads, config: makeConfig({ threadsCollapsed: true }) });
    const { pet, calls, emit } = await setup(collapsed);
    await userEvent.tab();
    expect(pet).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "New message" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Notifications" })).toHaveFocus();
    await userEvent.tab();
    const chevron = screen.getByRole("button", { name: "Show threads, 2 hidden, 1 needs you" });
    expect(chevron).toHaveFocus();
    expect(chevron).toHaveAttribute("title", "Show threads, 2 hidden, 1 needs you");
    expect(chevron).toHaveAttribute("aria-expanded", "false");
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "New message" })).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(chevron).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(calls).toContainEqual(["set_threads_collapsed", { collapsed: false }]);
    act(() => emit("snapshot", { ...collapsed, config: makeConfig({ threadsCollapsed: false }) }));
    expect(chevron).toHaveFocus();
    expect(chevron).toHaveAccessibleName("Hide threads");
    expect(chevron).toHaveAttribute("aria-expanded", "true");
  });

  it("names every control bar button and exposes its state", async () => {
    await setup(makeSnapshot({ config: makeConfig({ threadsCollapsed: true, notifications: false }) }));
    const bar = screen.getByRole("toolbar", { name: "Pet controls" });
    const compose = within(bar).getByRole("button", { name: "New message" });
    expect(compose).toHaveAttribute("title", "New message");
    expect(compose).toHaveAttribute("aria-expanded", "false");
    expect(within(bar).getByRole("button", { name: "Notifications", pressed: false })).toBeInTheDocument();
    const chevron = within(bar).getByRole("button", { name: "Show threads" });
    expect(chevron).toHaveAttribute("title", "Show threads");
    expect(chevron).toHaveAttribute("aria-expanded", "false");
  });

  it("moves between control bar buttons with Left/Right without nudging the pet", async () => {
    const { commands } = await setup();
    screen.getByRole("button", { name: "New message" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Notifications" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    expect(screen.getByRole("button", { name: "New message" })).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(screen.getByRole("button", { name: "Hide threads" })).toHaveFocus();
    expect(commands()).not.toContain("move_pet_by");
  });
});

describe("pet window names and roles", () => {
  it("gives each card a group role named with its project and status", async () => {
    const threads = [
      makeThread({ projectName: "api-server", status: "needs_input" }),
      makeThread({ projectName: "web", status: "ready", excerpt: "All set." }),
    ];
    await setup(makeSnapshot({ threads, approvals: [makeApproval({ projectName: "app", toolName: "Bash" })] }));
    expect(screen.getByRole("region", { name: "Claude Code sessions" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "api-server: Needs you" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "web: Done" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Bash request in app: needs your answer" })).toBeInTheDocument();
  });

  it("never focuses a permission card's buttons by itself, and shows them inactive until armed", async () => {
    await setup(makeSnapshot({ approvals: [makeApproval()] }));
    expect(document.activeElement).toBe(document.body);
    for (const name of ["Deny", "Always allow", "Allow"]) {
      expect(screen.getByRole("button", { name })).toHaveAttribute("aria-disabled", "true");
    }
  });
});

describe("the live region", () => {
  it("is the only polite live region, and announces a status change once", async () => {
    const t = makeThread({ sessionId: "s", projectName: "api-server", status: "running" });
    const { emit } = await setup(makeSnapshot({ threads: [t] }));
    expect(document.querySelectorAll('[aria-live="polite"], [role="status"], [role="log"]:not([aria-live="off"])')).toHaveLength(1);
    const region = liveRegion()!;
    expect(region).toHaveClass("sr-only");
    expect(region).toHaveTextContent(/^$/);

    act(() => emit("snapshot", makeSnapshot({ threads: [{ ...t, status: "needs_input" }] })));
    expect(region).toHaveTextContent("api-server: needs input");
    const first = region.textContent;

    // A new label (not a status change) leaves the announcement alone.
    act(() => emit("snapshot", makeSnapshot({ threads: [{ ...t, status: "needs_input", label: "Other" }] })));
    expect(region.textContent).toBe(first);
  });

  it("announces an update ready to install", async () => {
    const { emit } = await setup();
    act(() => emit("snapshot", makeSnapshot({ update: { state: "ready", version: "1.0.1" } })));
    expect(liveRegion()).toHaveTextContent("Devlings 1.0.1 is ready to install");
  });

  it("keeps the mini chat's streaming log out of it", () => {
    render(
      <Messages
        turns={[{ role: "assistant", text: "Half a repl", pending: true }]}
        loading={false}
        activity={null}
        empty={null}
      />,
    );
    expect(screen.getByRole("log", { name: "Messages" })).toHaveAttribute("aria-live", "off");
  });
});

describe("reduced motion", () => {
  const original = window.matchMedia;
  afterEach(() => {
    window.matchMedia = original;
  });

  const frameAfter = async (ms: number) => {
    const sprite = document.querySelector<HTMLElement>(".pet .sprite")!;
    await act(() => new Promise((r) => setTimeout(r, ms)));
    return sprite.dataset.frame;
  };

  it("animates a working pet normally", async () => {
    await setup(makeSnapshot({ petState: "running" }));
    expect(await frameAfter(400)).not.toBe("0");
  });

  it("holds a still frame when the system asks for reduced motion, even on hover", async () => {
    window.matchMedia = vi.fn((query: string) => ({
      matches: query.includes("prefers-reduced-motion: reduce"),
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
    })) as unknown as typeof window.matchMedia;
    const { pet } = await setup(makeSnapshot({ petState: "running" }));
    fireEvent.pointerEnter(pet);
    expect(document.querySelector(".pet .sprite")).toHaveAttribute("data-row", "running");
    expect(await frameAfter(400)).toBe("0");
  });
});
