import { render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import { ThreadView } from "./ThreadView";
import type { Conversation } from "./useConversation";

function fakeConversation(send: (text: string) => Promise<void>): Conversation {
  return {
    project: "C:\\code\\app",
    turns: [],
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
