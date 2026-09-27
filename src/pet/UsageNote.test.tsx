import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import type { UsageInfo } from "../shared/types";
import { ThreadView } from "./ThreadView";
import type { Conversation } from "./useConversation";

const HOUR = 3_600_000;

const conversation: Conversation = {
  project: "C:\code\app",
  turns: [],
  loading: false,
  sessionId: null,
  open: () => {},
  send: async () => {},
  reset: async () => {},
};

function renderWith(usage: UsageInfo | null) {
  fakeTransport();
  const { container } = render(
    <ThreadView
      snap={makeSnapshot({ usage })}
      project="C:\code\app"
      initialSessionId={null}
      conversation={conversation}
      onClose={() => {}}
    />,
  );
  return container;
}

describe("ThreadView footer: plan usage (v1.0 S4)", () => {
  it("says nothing while the plan is fine, or before any Ask", () => {
    const now = Date.now();
    expect(renderWith({ status: "allowed", utilization: 0.4, seenAt: now }).querySelector(".usage-note")).toBeNull();
    expect(renderWith(null).querySelector(".usage-note")).toBeNull();
  });

  it("notes when the plan is near its limit, with the as-of time", () => {
    const now = Date.now();
    renderWith({ status: "allowed_warning", utilization: 0.82, kind: "seven_day", resetsAt: now + 30 * HOUR, seenAt: now });
    expect(screen.getByText(/^Near your weekly limit \(82%\), resets .+ · as of .+$/)).toBeInTheDocument();
  });

  it("notes when the limit is reached", () => {
    const now = Date.now();
    renderWith({ status: "rejected", kind: "five_hour", resetsAt: now + HOUR, seenAt: now });
    expect(screen.getByText(/^5-hour limit reached, resets .+ · as of .+$/)).toBeInTheDocument();
  });
});
