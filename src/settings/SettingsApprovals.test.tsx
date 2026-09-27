import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeConfig, makeSnapshot } from "../test/fixtures";
import { Settings } from "./Settings";

describe("Settings: Approvals", () => {
  it("turns answering permission prompts on and off", async () => {
    const { calls } = fakeTransport({ list_pets: () => [] });
    const { rerender } = render(<Settings snap={makeSnapshot()} />);
    const toggle = screen.getByRole("switch", { name: "Answer permission prompts from Mochi" });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await userEvent.click(toggle);
    expect(calls).toContainEqual(["set_watch_approvals", { enabled: true }]);
    rerender(<Settings snap={makeSnapshot({ config: makeConfig({ watchApprovals: true }) })} />);
    expect(screen.getByRole("switch", { name: "Answer permission prompts from Mochi" })).toHaveAttribute("aria-checked", "true");
    await userEvent.click(screen.getByRole("switch", { name: "Answer permission prompts from Mochi" }));
    expect(calls).toContainEqual(["set_watch_approvals", { enabled: false }]);
  });

  it("says Claude Code's own prompt stays available", () => {
    fakeTransport({ list_pets: () => [] });
    render(<Settings snap={makeSnapshot()} />);
    expect(screen.getByText(/Claude Code's own prompt stays available/)).toBeInTheDocument();
  });

  it("chooses how long a request stays open", async () => {
    const { calls } = fakeTransport({ list_pets: () => [] });
    render(<Settings snap={makeSnapshot({ config: makeConfig({ approvalHoldSecs: 60 }) })} />);
    const group = screen.getByRole("radiogroup", { name: "Keep a request open for" });
    const options = Array.from(group.querySelectorAll("[role=radio]")).map((o) => o.textContent);
    expect(options).toEqual(["30 s", "1 min", "2 min", "4 min"]);
    expect(screen.getByRole("radio", { name: "1 min" })).toHaveAttribute("aria-checked", "true");
    await userEvent.click(screen.getByRole("radio", { name: "4 min" }));
    expect(calls).toContainEqual(["set_approval_hold", { secs: 240 }]);
    await userEvent.click(screen.getByRole("radio", { name: "30 s" }));
    expect(calls).toContainEqual(["set_approval_hold", { secs: 30 }]);
  });
});
