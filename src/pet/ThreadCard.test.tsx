import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeThread } from "../test/fixtures";
import { ThreadCard } from "./ThreadCard";

describe("ThreadCard", () => {
  it("shows project, status, line and relative time", () => {
    fakeTransport();
    const t = makeThread({ projectName: "api", status: "needs_input", label: "Wants to run npm test", updatedAt: 0 });
    render(<ThreadCard thread={t} now={2 * 60_000} onOpenThread={() => {}} />);
    expect(screen.getByText("api")).toBeInTheDocument();
    expect(screen.getByText("Wants to run npm test")).toBeInTheDocument();
    expect(screen.getByText("2m")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Needs you" })).toBeInTheDocument();
  });

  it("opens the thread view for an Ask thread", async () => {
    const { calls } = fakeTransport();
    const onOpen = vi.fn();
    const t = makeThread({ source: "ask", status: "ready", unread: true });
    render(<ThreadCard thread={t} now={0} onOpenThread={onOpen} />);
    await userEvent.click(screen.getByRole("button", { name: /Open conversation/ }));
    expect(onOpen).toHaveBeenCalledWith(t);
    expect(calls).toEqual([]);
  });

  it("marks a Watch thread viewed, then opens its project", async () => {
    const { calls } = fakeTransport();
    const onOpen = vi.fn();
    const t = makeThread({ sessionId: "w1", project: "C:\\code\\site", source: "watch", status: "ready" });
    render(<ThreadCard thread={t} now={0} onOpenThread={onOpen} />);
    await userEvent.click(screen.getByRole("button", { name: /Open project/ }));
    expect(onOpen).not.toHaveBeenCalled();
    expect(calls).toEqual([
      ["mark_viewed", { sessionId: "w1" }],
      ["open_project", { path: "C:\\code\\site" }],
    ]);
  });

  it("only marks a folderless Watch thread viewed", async () => {
    const { calls } = fakeTransport();
    const t = makeThread({ sessionId: "w2", project: "", projectName: "Claude Code", source: "watch", status: "ready" });
    render(<ThreadCard thread={t} now={0} onOpenThread={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: /Mark as seen/ }));
    expect(calls).toEqual([["mark_viewed", { sessionId: "w2" }]]);
  });

  it("dismisses a finished thread with the x", async () => {
    const { calls } = fakeTransport();
    const t = makeThread({ sessionId: "b1", projectName: "ml", status: "blocked" });
    render(<ThreadCard thread={t} now={0} onOpenThread={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss ml" }));
    expect(calls).toEqual([["mark_viewed", { sessionId: "b1" }]]);
  });

  it("uses a distinct indicator per status", () => {
    fakeTransport();
    const statuses = ["running", "needs_input", "ready", "blocked"] as const;
    const { container } = render(
      <>
        {statuses.map((status) => (
          <ThreadCard key={status} thread={makeThread({ status })} now={0} onOpenThread={() => {}} />
        ))}
      </>,
    );
    expect(Array.from(container.querySelectorAll(".status")).map((el) => el.className)).toEqual([
      "status status-running",
      "status status-needs_input",
      "status status-ready",
      "status status-blocked",
    ]);
  });
});
