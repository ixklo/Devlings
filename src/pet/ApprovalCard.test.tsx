import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeApproval, makeConfig, makeSetup, makeSnapshot } from "../test/fixtures";
import type { Snapshot } from "../shared/types";
import { ApprovalCard, ApprovalsIntroCard, showApprovalsIntro } from "./ApprovalCard";
import { PetApp } from "./PetApp";

const answers = (calls: [string, Record<string, unknown> | undefined][]) =>
  calls.filter(([cmd]) => cmd === "answer_approval").map(([, args]) => args);

describe("ApprovalCard", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("shows the project, tool, exact command and Claude's description", () => {
    fakeTransport();
    render(<ApprovalCard approval={makeApproval({ summary: 'git push --force "origin"' })} holdMs={60_000} />);
    const card = screen.getByRole("group", { name: "Bash request in app" });
    expect(within(card).getByText("app")).toBeInTheDocument();
    expect(within(card).getByText("Bash")).toBeInTheDocument();
    const summary = within(card).getByText('git push --force "origin"');
    expect(summary.tagName).toBe("CODE");
    expect(summary).toHaveClass("approval-summary");
    expect(within(card).getByText("Run the test suite")).toBeInTheDocument();
    expect(card).toHaveAttribute("data-hit");
  });

  it("leaves out an empty description", () => {
    fakeTransport();
    render(<ApprovalCard approval={makeApproval({ description: null })} holdMs={60_000} />);
    expect(document.querySelector(".approval-desc")).toBeNull();
  });

  it("offers the always button only when Claude Code sent a rule, with its detail as the description", () => {
    fakeTransport();
    const { unmount } = render(
      <ApprovalCard approval={makeApproval({ alwaysLabel: "Allow for this session", alwaysDetail: "Lets Claude Code edit files for the rest of this session" })} holdMs={60_000} />,
    );
    const always = screen.getByRole("button", { name: "Allow for this session" });
    expect(always).toHaveAccessibleDescription("Lets Claude Code edit files for the rest of this session");
    expect(always).toHaveAttribute("title", "Lets Claude Code edit files for the rest of this session");
    unmount();
    render(<ApprovalCard approval={makeApproval({ canAlwaysAllow: false, alwaysLabel: null, alwaysDetail: null })} holdMs={60_000} />);
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Deny", "Allow"]);
  });

  it("focuses nothing on mount, and Enter answers nothing", async () => {
    const { calls } = fakeTransport();
    render(<ApprovalCard approval={makeApproval()} holdMs={60_000} />);
    expect(document.activeElement).toBe(document.body);
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    await userEvent.keyboard("y");
    expect(answers(calls)).toEqual([]);
  });

  it.each([
    ["Allow", "allow"],
    ["Deny", "deny"],
    ["Always allow", "always"],
  ])("%s calls answer_approval with %s, once", async (name, decision) => {
    const { calls } = fakeTransport();
    render(<ApprovalCard approval={makeApproval({ id: "req-1" })} holdMs={60_000} />);
    await userEvent.click(screen.getByRole("button", { name }));
    await userEvent.click(screen.getByRole("button", { name: "Allow" }));
    expect(answers(calls)).toEqual([{ id: "req-1", decision }]);
    for (const b of screen.getAllByRole("button")) expect(b).toBeDisabled();
  });

  it("lets the user try again when an answer fails", async () => {
    const { calls } = fakeTransport({
      answer_approval: () => {
        throw "That request was already answered or is no longer waiting.";
      },
    });
    render(<ApprovalCard approval={makeApproval()} holdMs={60_000} />);
    await userEvent.click(screen.getByRole("button", { name: "Allow" }));
    expect(screen.getByRole("button", { name: "Deny" })).toBeEnabled();
    expect(answers(calls)).toHaveLength(1);
  });

  it("shows a countdown bar to the end of a watched request's hold", () => {
    fakeTransport();
    vi.spyOn(Date, "now").mockReturnValue(1_000_000);
    const { container, unmount } = render(<ApprovalCard approval={makeApproval({ expiresAt: 1_030_000 })} holdMs={60_000} />);
    const bar = container.querySelector<HTMLElement>(".approval-countdown-bar");
    expect(bar).not.toBeNull();
    expect(bar!.style.transform).toBe("scaleX(0.5)");
    expect(bar!.style.animationDuration).toBe("30000ms");
    unmount();
    // Ask requests have no hold.
    const ask = render(<ApprovalCard approval={makeApproval({ source: "ask", expiresAt: null })} holdMs={60_000} />);
    expect(ask.container.querySelector(".approval-countdown")).toBeNull();
    expect(within(ask.container).getByText("Ask")).toBeInTheDocument();
  });

  it("renders as an inline row in the mini chat", () => {
    fakeTransport();
    const { container } = render(<ApprovalCard approval={makeApproval({ source: "ask" })} holdMs={60_000} variant="row" />);
    const row = screen.getByRole("group", { name: "Bash request in app" });
    expect(row).toHaveClass("approval-row");
    expect(row).not.toHaveClass("card");
    expect(container.querySelector(".thread-card-project")).toBeNull();
  });
});

describe("approvals intro", () => {
  const snap = (over: Partial<Snapshot["config"]> = {}, hooksInstalled = true): Snapshot =>
    makeSnapshot({ config: makeConfig({ approvalsIntroSeen: false, onboarded: true, ...over }), setup: makeSetup({ hooksInstalled }) });

  it("shows once, after an upgrade with hooks installed", () => {
    expect(showApprovalsIntro(snap())).toBe(true);
    expect(showApprovalsIntro(snap({ approvalsIntroSeen: true }))).toBe(false);
    expect(showApprovalsIntro(snap({ onboarded: false }))).toBe(false);
    expect(showApprovalsIntro(snap({}, false))).toBe(false);
  });

  it("explains the feature in plain words", () => {
    fakeTransport();
    render(<ApprovalsIntroCard petName="Mochi" />);
    const card = screen.getByRole("group", { name: "Answer permission prompts" });
    expect(card.textContent?.replace(/\s+/g, " ")).toContain(
      "Mochi can answer permission prompts. When a Claude Code session asks to run something, you can Allow or Deny it right here. Claude Code's own prompt still works too.",
    );
    expect(card).toHaveAttribute("data-hit");
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
});
