import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Snapshot } from "../shared/types";
import { makeConfig, makeSetup, makeSnapshot } from "../test/fixtures";
import { SetupChecks } from "./SetupChecks";

function snap(over: Partial<Snapshot["setup"]>, hooksDeclined = false): Snapshot {
  return makeSnapshot({ config: makeConfig({ hooksDeclined }), setup: makeSetup(over) });
}

describe("SetupChecks", () => {
  it("shows a healthy setup", () => {
    render(<SetupChecks snap={snap({})} />);
    expect(screen.getByText("Claude Code 2.1.282 found")).toBeInTheDocument();
    expect(screen.getByText("Logged in with Claude Pro")).toBeInTheDocument();
    expect(screen.getByText("Watching your sessions")).toBeInTheDocument();
  });

  it("shows every problem", () => {
    render(
      <SetupChecks
        snap={snap({
          claudeError: "Claude Code wasn't found.",
          auth: { status: "refused", reason: "Ask only works with a Claude subscription login." },
          hooksInstalled: false,
          hookServerError: "Couldn't listen on port 1",
        })}
      />,
    );
    expect(screen.getByText("Claude Code wasn't found.")).toHaveClass("bad");
    expect(screen.getByText("Ask only works with a Claude subscription login.")).toHaveClass("bad");
    expect(screen.getByText("Hooks not installed")).toHaveClass("bad");
    expect(screen.getByText("Couldn't listen on port 1")).toHaveClass("bad");
  });

  it("treats skipped hooks as Ask-only, not an error", () => {
    render(<SetupChecks snap={snap({ hooksInstalled: false }, true)} />);
    expect(screen.getByText("Not watching sessions (Ask only)")).toHaveClass("neutral");
  });
});
