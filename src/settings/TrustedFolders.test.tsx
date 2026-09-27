import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeSnapshot } from "../test/fixtures";
import type { Snapshot } from "../shared/types";
import { Settings } from "./Settings";

function withTrusted(...names: string[]): Snapshot {
  const snap = makeSnapshot();
  snap.projects = snap.projects.map((p) => ({ ...p, trusted: names.includes(p.name) }));
  return snap;
}

describe("Settings → Trusted folders", () => {
  it("is hidden while no folder is trusted in Devlings", () => {
    fakeTransport({ list_pets: () => [] });
    render(<Settings snap={withTrusted()} />);
    expect(screen.queryByRole("region", { name: "Trusted folders" })).not.toBeInTheDocument();
  });

  it("lists folders trusted in Devlings, and Stop trusting calls untrust_project", async () => {
    const { calls } = fakeTransport({ list_pets: () => [], untrust_project: () => withTrusted() });
    render(<Settings snap={withTrusted("app")} />);
    const section = screen.getByRole("region", { name: "Trusted folders" });
    expect(within(section).getByText("app")).toBeInTheDocument();
    expect(within(section).queryByText("api")).not.toBeInTheDocument();
    await userEvent.click(within(section).getByRole("button", { name: "Stop trusting app" }));
    expect(calls).toContainEqual(["untrust_project", { project: "C:\\code\\app" }]);
  });
});
