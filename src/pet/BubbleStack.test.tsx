import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeThread } from "../test/fixtures";
import type { ThreadInfo } from "../shared/types";
import { BubbleStack } from "./BubbleStack";

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
    await userEvent.click(screen.getByRole("button", { name: /Finish setting up Perch/ }));
    expect(onOpen).toHaveBeenCalled();
  });

  it("marks every card as a hit region", () => {
    render(<Harness threads={threads} />);
    document.querySelectorAll(".thread-card, .more-pill").forEach((el) => expect(el).toHaveAttribute("data-hit"));
  });
});
