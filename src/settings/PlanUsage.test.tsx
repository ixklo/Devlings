import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import type { UsageInfo } from "../shared/types";
import { Settings } from "./Settings";

const HOUR = 3_600_000;

function renderWith(usage: UsageInfo | null) {
  fakeTransport({ list_pets: () => [] });
  render(<Settings snap={makeSnapshot({ usage })} />);
  return screen.getByRole("region", { name: "Cost" });
}

describe("Settings → Cost: plan usage (v1.0 S4)", () => {
  it("shows the last Ask's usage, labelled with when it was seen", () => {
    const now = Date.now();
    const cost = renderWith({ status: "allowed", utilization: 0.42, kind: "five_hour", resetsAt: now + HOUR, seenAt: now - 60_000 });
    expect(
      within(cost).getByText(/^Plan usage: 42% of your 5-hour limit, resets .+ \(as of .+, from your last Ask\)$/),
    ).toBeInTheDocument();
  });

  it("says where the number will come from before the first Ask", () => {
    const cost = renderWith(null);
    expect(within(cost).getByText("Plan usage shows here after your next Ask.")).toBeInTheDocument();
  });

  it("leaves out what Claude Code didn't report", () => {
    const cost = renderWith({ status: "allowed", seenAt: Date.now() });
    expect(within(cost).getByText(/^Plan usage: within your plan limit \(as of .+, from your last Ask\)$/)).toBeInTheDocument();
  });
});
