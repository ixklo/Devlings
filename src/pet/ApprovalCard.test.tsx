import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeApproval, makeConfig, makeSetup, makeSnapshot } from "../test/fixtures";
import type { PendingApproval, Snapshot } from "../shared/types";
import { ApprovalCard, ApprovalsIntroCard, showApprovalsIntro } from "./ApprovalCard";
import { PetApp } from "./PetApp";
import { ARM_CHECK_MS, ARM_MS } from "./useArming";

const answers = (calls: [string, Record<string, unknown> | undefined][]) =>
  calls.filter(([cmd]) => cmd === "answer_approval").map(([, args]) => args);

/** Lets the card sit still long enough to arm its buttons. */
const settle = () =>
  act(() => {
    vi.advanceTimersByTime(ARM_MS + 2 * ARM_CHECK_MS);
  });

/** A real mouse click: pointerdown, then click with detail 1. */
const mouseClick = (el: HTMLElement) => {
  fireEvent.pointerDown(el);
  fireEvent.click(el, { detail: 1 });
};

const flush = () => act(async () => {});

function card(approval: PendingApproval = makeApproval(), props: Partial<Parameters<typeof ApprovalCard>[0]> = {}) {
  return <ApprovalCard approval={approval} holdMs={60_000} {...props} />;
}

describe("ApprovalCard", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("shows the project, tool, exact command and Claude's description", () => {
    fakeTransport();
    render(card(makeApproval({ summary: 'git push --force "origin"' })));
    const group = screen.getByRole("group", { name: "Bash request in app: needs your answer" });
    expect(within(group).getByText("app")).toBeInTheDocument();
    expect(within(group).getByText("Bash")).toBeInTheDocument();
    const summary = within(group).getByText('git push --force "origin"');
    expect(summary.tagName).toBe("CODE");
    expect(summary).toHaveClass("approval-summary");
    expect(within(group).getByText("Run the test suite")).toBeInTheDocument();
    expect(group).toHaveAttribute("data-hit");
  });

  it("leaves out an empty description", () => {
    fakeTransport();
    render(card(makeApproval({ description: null })));
    expect(document.querySelector(".approval-desc")).toBeNull();
  });

  it("shows the whole input under Details, closed unless the headline leaves something out", () => {
    fakeTransport();
    const { unmount } = render(card(makeApproval({ details: '{\n  "command": "ls"\n}' })));
    const details = document.querySelector("details")!;
    expect(details.open).toBe(false);
    expect(within(details).getByText("Details")).toBeInTheDocument();
    expect(details.querySelector("pre")!.textContent).toBe('{\n  "command": "ls"\n}');
    unmount();
    const write = makeApproval({
      toolName: "mcp__fs__write_file",
      summary: "C:\\p\\a.txt",
      details: '{\n  "path": "C:\\\\p\\\\a.txt",\n  "content": "secret stuff"\n}',
      lossy: true,
    });
    render(card(write));
    expect(document.querySelector("details")!.open).toBe(true);
    expect(screen.getByText(/secret stuff/)).toBeInTheDocument();
  });

  it("shows an MCP tool's raw name next to its display name", () => {
    fakeTransport();
    render(card(makeApproval({ toolName: "Create issue", rawToolName: "mcp__github__create_issue" })));
    expect(screen.getByText("Create issue")).toBeInTheDocument();
    expect(screen.getByText("mcp__github__create_issue")).toHaveClass("approval-raw");
  });

  it("badges risky flags", () => {
    fakeTransport();
    render(card(makeApproval({ risks: ["Runs outside the sandbox"] })));
    expect(screen.getByText("Runs outside the sandbox")).toHaveClass("approval-risk");
  });

  it("offers only Deny for a request too long to review", async () => {
    const { calls } = fakeTransport();
    render(card(makeApproval({ tooLong: true, lossy: true, id: "big" })));
    expect(screen.getByText("Too long to review here. Answer in Claude Code.")).toBeInTheDocument();
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Deny"]);
    settle();
    mouseClick(screen.getByRole("button", { name: "Deny" }));
    await flush();
    expect(answers(calls)).toEqual([{ id: "big", decision: "deny" }]);
  });

  it("says an Ask request too long to review can only be denied", () => {
    fakeTransport();
    render(card(makeApproval({ tooLong: true, source: "ask" }), { variant: "row" }));
    expect(screen.getByText("Too long to review here, so it can only be denied.")).toBeInTheDocument();
  });

  it("offers the always button only when Claude Code sent a rule, and says what it does", () => {
    fakeTransport();
    const detail = "Lets Claude Code edit files for the rest of this session";
    const { unmount } = render(card(makeApproval({ alwaysLabel: "Allow for this session", alwaysDetail: detail })));
    const always = screen.getByRole("button", { name: "Allow for this session" });
    expect(always).toHaveAccessibleDescription(`Allow for this session: ${detail}`);
    expect(always).toHaveAttribute("title", detail);
    // Visible, not only a tooltip.
    expect(screen.getByText(`Allow for this session: ${detail}`)).toBeVisible();
    unmount();
    render(card(makeApproval({ canAlwaysAllow: false, alwaysLabel: null, alwaysDetail: null })));
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Deny", "Allow"]);
    expect(document.querySelector(".approval-always-detail")).toBeNull();
  });

  it("focuses nothing on mount, and no key answers on its own", async () => {
    const { calls } = fakeTransport();
    render(card());
    settle();
    expect(document.activeElement).toBe(document.body);
    for (const key of ["Enter", " ", "y", "a"]) fireEvent.keyDown(document.body, { key });
    await flush();
    expect(answers(calls)).toEqual([]);
  });

  it.each([
    ["Allow", "allow"],
    ["Deny", "deny"],
    ["Always allow", "always"],
  ])("%s calls answer_approval with %s, once", async (name, decision) => {
    const { calls } = fakeTransport();
    render(card(makeApproval({ id: "req-1" })));
    settle();
    mouseClick(screen.getByRole("button", { name }));
    mouseClick(screen.getByRole("button", { name: "Allow" }));
    await flush();
    expect(answers(calls)).toEqual([{ id: "req-1", decision }]);
    for (const b of screen.getAllByRole("button")) expect(b).toBeDisabled();
  });

  it("lets the user try again when an answer fails", async () => {
    const { calls } = fakeTransport({
      answer_approval: () => {
        throw "That request was already answered or is no longer waiting.";
      },
    });
    render(card());
    settle();
    mouseClick(screen.getByRole("button", { name: "Allow" }));
    await flush();
    expect(screen.getByRole("button", { name: "Deny" })).toBeEnabled();
    expect(answers(calls)).toHaveLength(1);
  });

  it("shows a countdown bar to the end of a watched request's hold", () => {
    fakeTransport();
    vi.setSystemTime(1_000_000);
    const { container, unmount } = render(card(makeApproval({ expiresAt: 1_030_000 })));
    const bar = container.querySelector<HTMLElement>(".approval-countdown-bar");
    expect(bar).not.toBeNull();
    expect(bar!.style.transform).toBe("scaleX(0.5)");
    // As a custom property, so reduced motion can keep its pace (pet.css).
    expect(bar!.style.getPropertyValue("--countdown-ms")).toBe("30000ms");
    unmount();
    // Ask requests have no hold.
    const ask = render(card(makeApproval({ source: "ask", expiresAt: null })));
    expect(ask.container.querySelector(".approval-countdown")).toBeNull();
    expect(within(ask.container).getByText("Ask")).toBeInTheDocument();
  });

  it("renders as an inline row in the mini chat", () => {
    fakeTransport();
    const { container } = render(card(makeApproval({ source: "ask" }), { variant: "row" }));
    const row = screen.getByRole("group", { name: "Bash request in app: needs your answer" });
    expect(row).toHaveClass("approval-row");
    expect(row).not.toHaveClass("card");
    expect(container.querySelector(".thread-card-project")).toBeNull();
  });
});

describe("ApprovalCard arming", () => {
  let rect = { x: 10, y: 20, width: 280, height: 30 };

  beforeEach(() => {
    vi.useFakeTimers();
    rect = { x: 10, y: 20, width: 280, height: 30 };
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      () =>
        ({
          ...rect,
          left: rect.x,
          top: rect.y,
          right: rect.x + rect.width,
          bottom: rect.y + rect.height,
          toJSON: () => ({}),
        }) as DOMRect,
    );
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  const buttons = () => screen.getAllByRole("button");
  const group = () => screen.getByRole("group", { name: /request in/ });

  it("ignores clicks until the card has been still for a moment, then fades the buttons in", async () => {
    const { calls } = fakeTransport();
    render(card());
    expect(group()).toHaveAttribute("data-armed", "false");
    for (const b of buttons()) expect(b).toHaveAttribute("aria-disabled", "true");
    mouseClick(screen.getByRole("button", { name: "Allow" }));
    act(() => {
      vi.advanceTimersByTime(ARM_MS - ARM_CHECK_MS);
    });
    mouseClick(screen.getByRole("button", { name: "Allow" }));
    await flush();
    expect(answers(calls)).toEqual([]);
    settle();
    expect(group()).toHaveAttribute("data-armed", "true");
    for (const b of buttons()) expect(b).toHaveAttribute("aria-disabled", "false");
    mouseClick(screen.getByRole("button", { name: "Allow" }));
    await flush();
    expect(answers(calls)).toHaveLength(1);
  });

  it("only counts a click whose press also came after arming", async () => {
    const { calls } = fakeTransport();
    render(card());
    const allow = screen.getByRole("button", { name: "Allow" });
    // Pressed while the card was still settling, released after it armed.
    fireEvent.pointerDown(allow);
    settle();
    fireEvent.click(allow, { detail: 1 });
    await flush();
    expect(answers(calls)).toEqual([]);
    // A fresh press counts.
    mouseClick(allow);
    await flush();
    expect(answers(calls)).toHaveLength(1);
  });

  it("re-arms when the card moves", async () => {
    const { calls } = fakeTransport();
    render(card());
    settle();
    expect(group()).toHaveAttribute("data-armed", "true");
    // A card above it went away, so it slid down.
    rect = { ...rect, y: 120 };
    act(() => {
      vi.advanceTimersByTime(ARM_CHECK_MS);
    });
    expect(group()).toHaveAttribute("data-armed", "false");
    mouseClick(screen.getByRole("button", { name: "Allow" }));
    await flush();
    expect(answers(calls)).toEqual([]);
    settle();
    expect(group()).toHaveAttribute("data-armed", "true");
  });

  it("rejects a click in the same frame as a move", async () => {
    const { calls } = fakeTransport();
    render(card());
    settle();
    rect = { ...rect, x: 40 };
    mouseClick(screen.getByRole("button", { name: "Allow" }));
    await flush();
    expect(answers(calls)).toEqual([]);
  });

  it("re-arms when the stage is laid out differently or the window moves", async () => {
    const { transport } = fakeTransport();
    let moved: (() => void) | null = null;
    transport.onMoved = vi.fn(async (cb: () => void) => {
      moved = cb;
      return () => {};
    }) as never;
    const { rerender } = render(card(makeApproval(), { layoutKey: "above" }));
    await flush();
    settle();
    expect(group()).toHaveAttribute("data-armed", "true");
    rerender(card(makeApproval(), { layoutKey: "below" }));
    expect(group()).toHaveAttribute("data-armed", "false");
    settle();
    expect(group()).toHaveAttribute("data-armed", "true");
    act(() => moved?.());
    expect(group()).toHaveAttribute("data-armed", "false");
  });

  it("answers from the keyboard only once armed, and only on a focused button", async () => {
    const { calls } = fakeTransport();
    render(card(makeApproval({ id: "k" })));
    // Tabbed to on purpose.
    act(() => screen.getByRole("button", { name: "Deny" }).focus());
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Deny" }));
    fireEvent.click(document.activeElement!, { detail: 0 });
    await flush();
    expect(answers(calls)).toEqual([]);
    settle();
    fireEvent.click(document.activeElement!, { detail: 0 });
    await flush();
    expect(answers(calls)).toEqual([{ id: "k", decision: "deny" }]);
  });
});

describe("approvals intro", () => {
  const snap = (over: Partial<Snapshot["config"]> = {}, hooksInstalled = true): Snapshot =>
    makeSnapshot({ config: makeConfig({ approvalsIntroSeen: false, onboarded: true, ...over }), setup: makeSetup({ hooksInstalled }) });

  it("shows once, after an upgrade with hooks installed, unless watching is already on", () => {
    expect(showApprovalsIntro(snap())).toBe(true);
    expect(showApprovalsIntro(snap({ approvalsIntroSeen: true }))).toBe(false);
    expect(showApprovalsIntro(snap({ onboarded: false }))).toBe(false);
    expect(showApprovalsIntro(snap({}, false))).toBe(false);
    expect(showApprovalsIntro(snap({ watchApprovals: true }))).toBe(false);
  });

  it("explains the feature in plain words", () => {
    fakeTransport();
    render(<ApprovalsIntroCard petName="Mochi" />);
    const intro = screen.getByRole("group", { name: "Answer permission prompts" });
    expect(intro.textContent?.replace(/\s+/g, " ")).toContain(
      "Mochi can answer permission prompts. When a Claude Code session asks to run something, you can Allow or Deny it right here. Claude Code's own prompt still works too.",
    );
    expect(intro).toHaveAttribute("data-hit");
    expect(document.activeElement).toBe(document.body);
  });

  it("Turn on switches watching on and marks the intro seen", async () => {
    const { commands, calls } = fakeTransport();
    render(<ApprovalsIntroCard petName="Mochi" />);
    await userEvent.click(screen.getByRole("button", { name: "Turn on" }));
    expect(commands()).toEqual(["set_watch_approvals", "mark_approvals_intro_seen"]);
    expect(calls[0][1]).toEqual({ enabled: true });
  });

  it("Not now only marks it seen", async () => {
    const { commands } = fakeTransport();
    render(<ApprovalsIntroCard petName="Mochi" />);
    await act(async () => {
      await userEvent.click(screen.getByRole("button", { name: "Not now" }));
    });
    expect(commands()).toEqual(["mark_approvals_intro_seen"]);
  });
});

describe("PetApp approvals", () => {
  it("shows approval cards above the pet and makes it wait", async () => {
    fakeTransport({ get_snapshot: () => makeSnapshot({ petState: "running", approvals: [makeApproval({ summary: "npm publish" })] }) });
    render(<PetApp />);
    expect(await screen.findByText("npm publish")).toBeInTheDocument();
    expect(document.querySelector("main")).toHaveAttribute("data-pet-state", "needs_input");
    expect(document.activeElement).toBe(document.body);
  });

  it("keeps setup ahead of approvals", async () => {
    fakeTransport({ get_snapshot: () => makeSnapshot({ petState: "setup", approvals: [makeApproval()] }) });
    render(<PetApp />);
    await screen.findByRole("button", { name: /Mochi/ });
    expect(document.querySelector("main")).toHaveAttribute("data-pet-state", "setup");
  });

  it("shows the intro once after an upgrade", async () => {
    const { emit } = fakeTransport({
      get_snapshot: () => makeSnapshot({ config: makeConfig({ approvalsIntroSeen: false }) }),
    });
    render(<PetApp />);
    expect(await screen.findByRole("group", { name: "Answer permission prompts" })).toBeInTheDocument();
    act(() => emit("snapshot", makeSnapshot({ config: makeConfig({ approvalsIntroSeen: true }) })));
    expect(screen.queryByRole("group", { name: "Answer permission prompts" })).not.toBeInTheDocument();
  });

  it("re-arms approval cards when the cards flip below the pet", async () => {
    vi.useFakeTimers();
    try {
      const { emit } = fakeTransport({ get_snapshot: () => makeSnapshot({ approvals: [makeApproval()] }) });
      render(<PetApp />);
      await flush();
      settle();
      const group = screen.getByRole("group", { name: /request in/ });
      expect(group).toHaveAttribute("data-armed", "true");
      act(() => emit("pet-placement", { cardsBelow: true, shiftX: 0, stageRoom: 300 }));
      expect(screen.getByRole("group", { name: /request in/ })).toHaveAttribute("data-armed", "false");
    } finally {
      vi.useRealTimers();
    }
  });
});
