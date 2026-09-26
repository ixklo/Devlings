import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Snapshot } from "../shared/types";
import { SetupChecks } from "./SetupChecks";

function snap(over: Partial<Snapshot["setup"]>, hooksDeclined = false): Snapshot {
  return {
    config: {
      petName: "Mochi", onboarded: true, creditsNoticeSeen: true, hooksDeclined, hookPort: 1, hookToken: "t",
      claudePath: null, petPosition: null, notifications: true, launchAtLogin: false,
    },
    mood: "idle",
    sessions: [],
    projects: [],
    running: [],
    setup: {
      claudePath: "C:\\claude.exe", claudeVersion: "2.1.282", claudeError: null, hooksInstalled: true,
      auth: { status: "allowed", subscription: "pro" }, hookServerError: null, needsSetup: false, ...over,
    },
  };
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
    expect(screen.getByText("Claude Code wasn't found.")).toBeInTheDocument();
    expect(screen.getByText("Ask only works with a Claude subscription login.")).toBeInTheDocument();
    expect(screen.getByText("Hooks not installed")).toBeInTheDocument();
    expect(screen.getByText("Couldn't listen on port 1")).toBeInTheDocument();
  });

  it("treats skipped hooks as Ask-only, not an error", () => {
    render(<SetupChecks snap={snap({ hooksInstalled: false }, true)} />);
    expect(screen.getByText("Not watching sessions (Ask only)")).toHaveClass("neutral");
  });
});
