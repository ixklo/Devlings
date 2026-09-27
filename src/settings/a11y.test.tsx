import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { PetInfo } from "../shared/types";
import { fakeTransport } from "../test/fakeTransport";
import { makeConfig, makeSetup, makeSnapshot } from "../test/fixtures";
import { Settings } from "./Settings";

const PETS: PetInfo[] = [
  { id: "perch", displayName: "Perch", description: "Teal bird", source: "bundled" },
  { id: "ember", displayName: "Ember", description: "Orange", source: "bundled" },
  { id: "plum", displayName: "Plum", description: "Purple", source: "bundled" },
];

describe("Settings keyboard and names", () => {
  it("makes each segmented control one Tab stop that arrow keys move through and choose", async () => {
    const { calls } = fakeTransport({ list_pets: () => [] });
    render(<Settings snap={makeSnapshot({ config: makeConfig({ approvalHoldSecs: 60 }) })} />);
    const oneMin = screen.getByRole("radio", { name: "1 min" });
    expect(oneMin).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("radio", { name: "30 s" })).toHaveAttribute("tabindex", "-1");
    oneMin.focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("radio", { name: "2 min" })).toHaveFocus();
    expect(calls).toContainEqual(["set_approval_hold", { secs: 120 }]);
    await userEvent.keyboard("{Home}");
    expect(calls).toContainEqual(["set_approval_hold", { secs: 30 }]);
    await userEvent.keyboard("{ArrowLeft}");
    expect(screen.getByRole("radio", { name: "4 min" })).toHaveFocus();
    expect(calls).toContainEqual(["set_approval_hold", { secs: 240 }]);
  });

  it("lets arrow keys choose a pet", async () => {
    const { calls } = fakeTransport({ list_pets: () => PETS });
    render(<Settings snap={makeSnapshot()} />);
    const perch = await screen.findByRole("radio", { name: "Perch" });
    expect(perch).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("radio", { name: "Ember" })).toHaveAttribute("tabindex", "-1");
    perch.focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(screen.getByRole("radio", { name: "Ember" })).toHaveFocus();
    await waitFor(() => expect(calls).toContainEqual(["set_pet", { id: "ember" }]));
  });

  it("describes each switch with its row text", () => {
    fakeTransport({ list_pets: () => [] });
    render(<Settings snap={makeSnapshot()} />);
    expect(screen.getByRole("switch", { name: "Notifications" })).toHaveAccessibleDescription(
      "When a session finishes, needs you, or gets stuck.",
    );
  });

  it("names the hooks buttons with what they act on", () => {
    fakeTransport({ list_pets: () => [] });
    const { rerender } = render(<Settings snap={makeSnapshot()} />);
    expect(screen.getByRole("button", { name: "Remove hooks" })).toHaveTextContent("Remove");
    rerender(<Settings snap={makeSnapshot({ setup: makeSetup({ hooksInstalled: false }) })} />);
    expect(screen.getByRole("button", { name: "Install hooks" })).toHaveTextContent("Install");
  });
});
