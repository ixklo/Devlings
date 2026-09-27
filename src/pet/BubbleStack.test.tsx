import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeApproval, makeThread } from "../test/fixtures";
import type { PendingApproval, ThreadInfo } from "../shared/types";
import { BubbleStack } from "./BubbleStack";
import { GHOST_MS } from "./useApprovalGhosts";
import { ARM_CHECK_MS, ARM_MS } from "./useArming";

function Harness({ threads, setup }: { threads: ThreadInfo[]; setup?: { detail: string; onOpen: () => void } }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <BubbleStack
      threads={threads}
      now={60_000}
      expanded={expanded}
      onToggleExpanded={() => setExpanded((x) => !x)}
      onOpenThread={() => {}}
      setup={setup ?? null}
    />
  );
}

const projectNames = () =>
  Array.from(document.querySelectorAll(".thread-card-project")).map((el) => el.textContent);

describe("BubbleStack", () => {
  beforeEach(() => {
    fakeTransport();
  });

  const threads = [
    makeThread({ projectName: "running-old", status: "running", updatedAt: 1 }),
    makeThread({ projectName: "ready", status: "ready", updatedAt: 5 }),
    makeThread({ projectName: "running-new", status: "running", updatedAt: 9 }),
    makeThread({ projectName: "needs", status: "needs_input", updatedAt: 2 }),
    makeThread({ projectName: "blocked", status: "blocked", updatedAt: 3 }),
  ];

  it("shows the three highest-priority threads and a +N more pill", () => {
    render(<Harness threads={threads} />);
    expect(projectNames()).toEqual(["needs", "blocked", "ready"]);
    expect(screen.getByRole("button", { name: "+2 more" })).toBeInTheDocument();
  });

  it("expands to every thread and collapses again", async () => {
    render(<Harness threads={threads} />);
    await userEvent.click(screen.getByRole("button", { name: "+2 more" }));
    expect(projectNames()).toEqual(["needs", "blocked", "ready", "running-new", "running-old"]);
    await userEvent.click(screen.getByRole("button", { name: "Show less" }));
    expect(projectNames()).toHaveLength(3);
  });

  it("puts the tail on the card nearest the pet", () => {
    render(<Harness threads={threads} />);
    const tails = document.querySelectorAll(".has-tail");
    expect(tails).toHaveLength(1);
    expect(within(tails[0] as HTMLElement).getByText("needs")).toBeInTheDocument();
  });

  it("has no pill for three or fewer threads", () => {
    render(<Harness threads={threads.slice(0, 3)} />);
    expect(screen.queryByRole("button", { name: /more/ })).not.toBeInTheDocument();
  });

  it("shows the setup card above the pet and opens settings from it", async () => {
    const onOpen = vi.fn();
    render(<Harness threads={[]} setup={{ detail: "Claude Code wasn't found.", onOpen }} />);
    await userEvent.click(screen.getByRole("button", { name: /Finish setting up Devlings/ }));
    expect(onOpen).toHaveBeenCalled();
  });

  it("marks every card as a hit region", () => {
    render(<Harness threads={threads} />);
    document.querySelectorAll(".thread-card, .more-pill").forEach((el) => expect(el).toHaveAttribute("data-hit"));
  });
});

describe("BubbleStack with approvals", () => {
  beforeEach(() => {
    fakeTransport();
  });

  const stack = (props: Partial<Parameters<typeof BubbleStack>[0]>) => (
    <BubbleStack threads={[]} now={60_000} expanded={false} onToggleExpanded={() => {}} onOpenThread={() => {}} holdMs={60_000} {...props} />
  );

  it("shows one approval card per request, nearest the pet, with the tail", () => {
    const threads = [makeThread({ projectName: "other", status: "ready" })];
    render(stack({ threads, approvals: [makeApproval({ id: "a", projectName: "app" }), makeApproval({ id: "b", projectName: "api" })] }));
    const cards = screen.getAllByRole("group", { name: /request in/ });
    expect(cards.map((c) => within(c).getByText(/^(app|api)$/).textContent)).toEqual(["app", "api"]);
    expect(cards[0]).toHaveClass("has-tail");
    expect(document.querySelectorAll(".has-tail")).toHaveLength(1);
    expect(projectNames()).toContain("other");
  });

  it("replaces the thread card of a session that has a request", () => {
    const waiting = makeThread({ sessionId: "s-wait", projectName: "waiting", status: "needs_input" });
    const approval = makeApproval({ sessionId: "s-wait", projectName: "waiting" });
    const { rerender } = render(stack({ threads: [waiting], approvals: [approval] }));
    expect(document.querySelectorAll(".thread-card:not(.approval-card)")).toHaveLength(0);
    // After the hold ends, the plain "needs input" card comes back.
    rerender(stack({ threads: [waiting], approvals: [] }));
    expect(projectNames()).toEqual(["waiting"]);
  });

  it("keeps the stack short and folds the rest into +N more", async () => {
    const threads = [makeThread({ projectName: "t1" }), makeThread({ projectName: "t2" }), makeThread({ projectName: "t3" })];
    const approvals = [makeApproval({ id: "x" }), makeApproval({ id: "y" }), makeApproval({ id: "z" })];
    function Expandable() {
      const [expanded, setExpanded] = useState(false);
      return stack({ threads, approvals, expanded, onToggleExpanded: () => setExpanded((x) => !x) });
    }
    render(<Expandable />);
    expect(screen.getAllByRole("group", { name: /request in/ })).toHaveLength(2);
    expect(document.querySelectorAll(".thread-card:not(.approval-card)")).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: "+3 more" }));
    expect(screen.getAllByRole("group", { name: /request in/ })).toHaveLength(3);
    expect(document.querySelectorAll(".thread-card:not(.approval-card)")).toHaveLength(3);
    await userEvent.click(screen.getByRole("button", { name: "Show less" }));
    expect(screen.getAllByRole("group", { name: /request in/ })).toHaveLength(2);
  });

  it("shows the intro card, which takes the tail when it's alone", () => {
    render(stack({ intro: { petName: "Mochi" } }));
    expect(screen.getByRole("group", { name: "Answer permission prompts" })).toHaveClass("has-tail");
  });

  it("marks approval and intro cards as hit regions", () => {
    render(stack({ approvals: [makeApproval()], intro: { petName: "Mochi" } }));
    const cards = screen.getAllByRole("group", { name: /request in|Answer permission prompts/ });
    expect(cards).toHaveLength(2);
    for (const el of cards) expect(el).toHaveAttribute("data-hit");
  });
});

describe("BubbleStack placeholders", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    fakeTransport();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const stack = (approvals: PendingApproval[], props: Partial<Parameters<typeof BubbleStack>[0]> = {}) => (
    <BubbleStack threads={[]} now={60_000} expanded={false} onToggleExpanded={() => {}} onOpenThread={() => {}} approvals={approvals} {...props} />
  );

  it("keeps a gone request's place for a moment, so the next card doesn't slide under the cursor", () => {
    const a = makeApproval({ id: "a", summary: "first" });
    const b = makeApproval({ id: "b", summary: "second" });
    const { rerender } = render(stack([a, b]));
    rerender(stack([b]));
    const slots = Array.from(document.querySelectorAll(".bubble-slot")).map((el) => el.textContent);
    expect(slots[0]).toBe("No longer waiting");
    expect(slots[1]).toContain("second");
    expect(document.querySelector(".approval-ghost")).toHaveAttribute("data-hit");
    act(() => {
      vi.advanceTimersByTime(GHOST_MS);
    });
    expect(document.querySelector(".approval-ghost")).not.toBeInTheDocument();
    expect(document.querySelectorAll(".bubble-slot")).toHaveLength(1);
  });

  it("says Answered for a request answered from its card", async () => {
    const a = makeApproval({ id: "a" });
    const { rerender } = render(stack([a]));
    act(() => {
      vi.advanceTimersByTime(ARM_MS + 2 * ARM_CHECK_MS);
    });
    fireEvent.pointerDown(screen.getByRole("button", { name: "Allow" }));
    fireEvent.click(screen.getByRole("button", { name: "Allow" }), { detail: 1 });
    await act(async () => {});
    rerender(stack([]));
    expect(document.querySelector(".approval-ghost")).toHaveTextContent("Answered");
  });

  it("leaves no placeholder when the cards are collapsed", () => {
    const a = makeApproval({ id: "a" });
    const { rerender } = render(stack([a]));
    rerender(stack([a], { approvalsHidden: true }));
    expect(document.querySelector(".approval-ghost")).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /request in/ })).not.toBeInTheDocument();
  });
});
