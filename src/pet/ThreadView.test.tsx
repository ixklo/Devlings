import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import type { ChatTurn } from "../shared/types";
import { ThreadView } from "./ThreadView";
import type { Conversation } from "./useConversation";

function fakeConversation(send: (text: string) => Promise<void>, turns: ChatTurn[] = []): Conversation {
  return {
    project: "C:\\code\\app",
    turns,
    loading: false,
    sessionId: null,
    open: () => {},
    send,
    reset: async () => {},
  };
}

describe("ThreadView initial prompt", () => {
  it("sends the composer's prompt exactly once, even under StrictMode", async () => {
    fakeTransport();
    const send = vi.fn(async () => {});
    render(
      <StrictMode>
        <ThreadView
          snap={makeSnapshot()}
          project="C:\code\app"
          initialSessionId={null}
          initialPrompt="why is CI red?"
          conversation={fakeConversation(send)}
          onClose={() => {}}
        />
      </StrictMode>,
    );
    await waitFor(() => expect(send).toHaveBeenCalledTimes(1));
    expect(send).toHaveBeenCalledWith("why is CI red?");
  });

  it("shows the credits notice in the thread when the first send needs it", async () => {
    fakeTransport();
    const send = vi.fn(async () => {
      throw "credits_notice";
    });
    render(
      <ThreadView
        snap={makeSnapshot()}
        project="C:\code\app"
        initialSessionId={null}
        initialPrompt="first ask"
        conversation={fakeConversation(send)}
        onClose={() => {}}
      />,
    );
    expect(await screen.findByRole("button", { name: /got it/i })).toBeInTheDocument();
  });
});

describe("ThreadView untrusted-folder notice", () => {
  const HEADLINE = "This folder isn't trusted in Claude Code yet, so Mochi ran without its project settings.";
  const SKIPPED =
    "Skipped: hooks (PreToolUse, Stop) · environment variables (ANTHROPIC_BASE_URL, FOO) · MCP servers (db, github) · apiKeyHelper · 2 project skills.";
  const turns: ChatTurn[] = [
    { role: "user", text: "what does this repo do?" },
    { role: "untrusted", text: HEADLINE, detail: SKIPPED },
    { role: "assistant", text: "It builds a CLI." },
  ];

  function snapWithTrust(trusted: boolean) {
    const snap = makeSnapshot();
    snap.projects = snap.projects.map((p) => (p.name === "app" ? { ...p, trusted } : p));
    return snap;
  }

  function show(trusted: boolean) {
    return render(
      <ThreadView
        snap={snapWithTrust(trusted)}
        project="C:\code\app"
        initialSessionId={null}
        conversation={fakeConversation(async () => {}, turns)}
        onClose={() => {}}
      />,
    );
  }

  it("shows what was skipped as its own row, not as Claude's text", () => {
    fakeTransport();
    show(false);
    const row = screen.getByRole("group", { name: "Untrusted folder" });
    expect(row).toHaveTextContent(HEADLINE);
    for (const item of ["hooks (PreToolUse, Stop)", "environment variables (ANTHROPIC_BASE_URL, FOO)", "MCP servers (db, github)", "apiKeyHelper", "2 project skills"]) {
      expect(row).toHaveTextContent(item);
    }
    expect(row.closest(".msg-assistant")).toBeNull();
    expect(within(row).getByRole("button", { name: "Trust this folder in Mochi" })).toBeInTheDocument();
  });

  it("trusts the folder through trust_project", async () => {
    const { calls } = fakeTransport({ trust_project: () => snapWithTrust(true) });
    show(false);
    await userEvent.click(screen.getByRole("button", { name: "Trust this folder in Mochi" }));
    expect(calls).toContainEqual(["trust_project", { project: "C:\\code\\app" }]);
  });

  it("says the next Ask uses the folder's settings once trusted, and can undo it", async () => {
    const { calls } = fakeTransport({ untrust_project: () => snapWithTrust(false) });
    show(true);
    const row = screen.getByRole("group", { name: "Untrusted folder" });
    expect(row).toHaveTextContent("Trusted in Mochi. The next Ask uses this folder's settings.");
    expect(within(row).queryByRole("button", { name: /Trust this folder/ })).not.toBeInTheDocument();
    await userEvent.click(within(row).getByRole("button", { name: "Stop trusting" }));
    expect(calls).toContainEqual(["untrust_project", { project: "C:\\code\\app" }]);
  });

  it("shows why trusting failed", async () => {
    fakeTransport({
      trust_project: () => {
        throw "Couldn't save that: disk full";
      },
    });
    show(false);
    await userEvent.click(screen.getByRole("button", { name: "Trust this folder in Mochi" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't save that: disk full");
  });
});
