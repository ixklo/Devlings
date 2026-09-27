import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot, makeThread } from "../test/fixtures";
import type { Snapshot, ThreadInfo, UpdateStatus } from "../shared/types";
import { BubbleStack } from "./BubbleStack";
import { PetApp } from "./PetApp";
import { UpdateCard, visibleUpdate } from "./UpdateCard";

const READY: UpdateStatus = { state: "ready", version: "1.0.1" };

const cardText = (el: HTMLElement) => el.textContent?.replace(/\s+/g, " ").trim();

describe("UpdateCard", () => {
  beforeEach(() => {
    fakeTransport();
  });

  it("says which version is ready, with Restart and Later", () => {
    render(<UpdateCard version="1.0.1" onRestart={async () => {}} onLater={() => {}} />);
    const card = screen.getByRole("group", { name: /update ready/ });
    expect(cardText(card)).toContain("Perch 1.0.1 is ready. Restart to install.");
    expect(within(card).getByRole("button", { name: "Restart" })).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Later" })).toBeInTheDocument();
    expect(card).toHaveAttribute("data-hit");
  });

  it("restarts, and shows why when the restart is refused", async () => {
    const onRestart = vi.fn(async () => {
      throw "Finish or stop the running ask first.";
    });
    render(<UpdateCard version="1.0.1" onRestart={onRestart} onLater={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Restart" }));
    expect(onRestart).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("alert")).toHaveTextContent("Finish or stop the running ask first.");
    expect(screen.getByRole("button", { name: "Restart" })).toBeEnabled();
  });

  it("hides on Later", async () => {
    const onLater = vi.fn();
    render(<UpdateCard version="1.0.1" onRestart={async () => {}} onLater={onLater} />);
    await userEvent.click(screen.getByRole("button", { name: "Later" }));
    expect(onLater).toHaveBeenCalled();
  });
});

describe("visibleUpdate", () => {
  it("shows a ready update unless an Ask is running or that version was put off", () => {
    expect(visibleUpdate(READY, [], null)).toBe("1.0.1");
    expect(visibleUpdate(READY, ["C:\\code\\app"], null)).toBeNull();
    expect(visibleUpdate(READY, [], "1.0.1")).toBeNull();
    expect(visibleUpdate({ state: "ready", version: "1.0.2" }, [], "1.0.1")).toBe("1.0.2");
    for (const state of ["idle", "checking", "available", "downloading", "error", "disabled"] as const) {
      expect(visibleUpdate({ state, version: "1.0.1" }, [], null)).toBeNull();
    }
    expect(visibleUpdate({ state: "ready" }, [], null)).toBeNull();
  });
});

describe("BubbleStack with an update", () => {
  beforeEach(() => {
    fakeTransport();
  });

  const stack = (threads: ThreadInfo[]) => (
    <BubbleStack
      threads={threads}
      now={60_000}
      expanded={false}
      onToggleExpanded={() => {}}
      onOpenThread={() => {}}
      update={{ version: "1.0.1", onRestart: async () => {}, onLater: () => {} }}
    />
  );

  it("points the tail from the update card when it's the only card", () => {
    render(stack([]));
    expect(screen.getByRole("group", { name: /update ready/ })).toHaveClass("has-tail");
  });

  it("sits above the threads, which keep the tail", () => {
    render(stack([makeThread({ projectName: "app", status: "ready" })]));
    const card = screen.getByRole("group", { name: /update ready/ });
    expect(card).not.toHaveClass("has-tail");
    expect(document.querySelectorAll(".has-tail")).toHaveLength(1);
  });
});

describe("PetApp update card", () => {
  function setup(snap: Snapshot, handlers: Record<string, (a?: Record<string, unknown>) => unknown> = {}) {
    return fakeTransport({ get_snapshot: () => snap, ...handlers });
  }

  it("restarts through install_update", async () => {
    const { commands } = setup(makeSnapshot({ update: READY }));
    render(<PetApp />);
    await userEvent.click(await screen.findByRole("button", { name: "Restart" }));
    expect(commands()).toContain("install_update");
  });

  it("is hidden while an Ask run is active", async () => {
    const { emit } = setup(makeSnapshot({ update: READY, running: ["C:\\code\\app"] }));
    render(<PetApp />);
    await screen.findByRole("button", { name: /Mochi/ });
    expect(screen.queryByRole("group", { name: /update ready/ })).not.toBeInTheDocument();
    act(() => emit("snapshot", makeSnapshot({ update: READY, running: [] })));
    expect(screen.getByRole("group", { name: /update ready/ })).toBeInTheDocument();
  });

  it("Later hides it until a newer version is ready", async () => {
    const { emit } = setup(makeSnapshot({ update: READY }));
    render(<PetApp />);
    await userEvent.click(await screen.findByRole("button", { name: "Later" }));
    expect(screen.queryByRole("group", { name: /update ready/ })).not.toBeInTheDocument();
    act(() => emit("snapshot", makeSnapshot({ update: READY })));
    expect(screen.queryByRole("group", { name: /update ready/ })).not.toBeInTheDocument();
    act(() => emit("snapshot", makeSnapshot({ update: { state: "ready", version: "1.0.2" } })));
    expect(cardText(screen.getByRole("group", { name: /update ready/ }))).toContain("Perch 1.0.2 is ready.");
  });
});
