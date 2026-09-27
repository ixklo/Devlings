import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeConfig, makeSnapshot } from "../test/fixtures";
import type { Snapshot, UpdateStatus } from "../shared/types";
import { LICENSES_URL, updateLine } from "./About";
import { Settings } from "./Settings";

type Handlers = Record<string, (a?: Record<string, unknown>) => unknown>;

function renderAbout(snap: Snapshot = makeSnapshot(), handlers: Handlers = {}) {
  const fake = fakeTransport({ list_pets: () => [], ...handlers });
  const view = render(<Settings snap={snap} />);
  const about = () => screen.getByRole("region", { name: "About" });
  return { ...fake, ...view, about };
}

describe("updateLine", () => {
  const line = (u: UpdateStatus, upToDate = false) => updateLine(u, upToDate);

  it("describes every update state", () => {
    expect(line({ state: "idle" })).toBeNull();
    expect(line({ state: "idle" }, true)).toBe("Perch is up to date.");
    expect(line({ state: "checking" })).toBe("Checking for updates…");
    expect(line({ state: "available", version: "1.0.1" })).toBe("Perch 1.0.1 is available. Downloading…");
    expect(line({ state: "downloading", version: "1.0.1" })).toBe("Downloading Perch 1.0.1…");
    expect(line({ state: "downloading", version: "1.0.1", progress: 42 })).toBe("Downloading Perch 1.0.1… 42%");
    expect(line({ state: "ready", version: "1.0.1" })).toBe("Perch 1.0.1 is ready. Restart to install.");
    expect(line({ state: "error", error: "Couldn't check for updates." })).toBe("Couldn't check for updates.");
    expect(line({ state: "error" })).toBe("Couldn't check for updates.");
    expect(line({ state: "disabled", error: "Updates are off in development builds." })).toBe(
      "Updates are off in development builds.",
    );
  });
});

describe("Settings → About", () => {
  it("shows the version", async () => {
    const { about } = renderAbout();
    expect(await within(about()).findByText("Perch v0.2.0")).toBeInTheDocument();
  });

  it("toggles automatic checks", async () => {
    const { about, calls } = renderAbout(makeSnapshot({ config: makeConfig({ autoUpdate: true }) }));
    const toggle = within(about()).getByRole("switch", { name: "Check for updates automatically" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await userEvent.click(toggle);
    expect(calls).toContainEqual(["set_auto_update", { enabled: false }]);
  });

  it("checks for updates and says when Perch is up to date", async () => {
    const { about, calls } = renderAbout(makeSnapshot(), { check_for_update: () => ({ state: "idle" }) });
    await userEvent.click(within(about()).getByRole("button", { name: "Check for updates" }));
    expect(calls).toContainEqual(["check_for_update", undefined]);
    expect(await within(about()).findByText("Perch is up to date.")).toBeInTheDocument();
  });

  it("stops saying up to date once the status moves on", async () => {
    const { about, rerender } = renderAbout(makeSnapshot(), { check_for_update: () => ({ state: "idle" }) });
    await userEvent.click(within(about()).getByRole("button", { name: "Check for updates" }));
    expect(await within(about()).findByText("Perch is up to date.")).toBeInTheDocument();
    // A later background check that fails quietly lands back on idle; that isn't "up to date".
    rerender(<Settings snap={makeSnapshot({ update: { state: "checking" } })} />);
    rerender(<Settings snap={makeSnapshot({ update: { state: "idle" } })} />);
    expect(within(about()).queryByText("Perch is up to date.")).not.toBeInTheDocument();
  });

  it("keeps up to date when the check's own snapshot arrives after its result", async () => {
    let finish: (s: UpdateStatus) => void = () => {};
    const { about, rerender } = renderAbout(makeSnapshot(), {
      check_for_update: () => new Promise<UpdateStatus>((resolve) => (finish = resolve)),
    });
    await userEvent.click(within(about()).getByRole("button", { name: "Check for updates" }));
    rerender(<Settings snap={makeSnapshot({ update: { state: "checking" } })} />);
    await act(async () => finish({ state: "idle" }));
    rerender(<Settings snap={makeSnapshot({ update: { state: "idle" } })} />);
    expect(within(about()).getByText("Perch is up to date.")).toBeInTheDocument();
  });

  it("says when it couldn't check", async () => {
    const { about, rerender } = renderAbout(makeSnapshot(), {
      check_for_update: () => ({ state: "error", error: "Couldn't check for updates." }),
    });
    await userEvent.click(within(about()).getByRole("button", { name: "Check for updates" }));
    // The backend's snapshot follows the command's result.
    rerender(<Settings snap={makeSnapshot({ update: { state: "error", error: "Couldn't check for updates." } })} />);
    expect(await within(about()).findByText("Couldn't check for updates.")).toHaveClass("is-error");
  });

  it("follows the download and offers a restart when the update is ready", async () => {
    const { about, rerender, calls } = renderAbout(makeSnapshot({ update: { state: "checking" } }));
    expect(within(about()).getByText("Checking for updates…")).toBeInTheDocument();
    expect(within(about()).getByRole("button", { name: "Check for updates" })).toBeDisabled();
    rerender(<Settings snap={makeSnapshot({ update: { state: "downloading", version: "1.0.1", progress: 42 } })} />);
    expect(within(about()).getByText("Downloading Perch 1.0.1… 42%")).toBeInTheDocument();
    rerender(<Settings snap={makeSnapshot({ update: { state: "ready", version: "1.0.1" } })} />);
    expect(within(about()).getByText("Perch 1.0.1 is ready. Restart to install.")).toBeInTheDocument();
    await userEvent.click(within(about()).getByRole("button", { name: "Restart to update" }));
    expect(calls).toContainEqual(["install_update", undefined]);
  });

  it("shows why a restart was refused", async () => {
    const { about } = renderAbout(makeSnapshot({ update: { state: "ready", version: "1.0.1" } }), {
      install_update: () => {
        throw "Finish or stop the running ask first.";
      },
    });
    await userEvent.click(within(about()).getByRole("button", { name: "Restart to update" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Finish or stop the running ask first.");
  });

  it("disables checking in builds without updates", () => {
    const { about } = renderAbout(
      makeSnapshot({ update: { state: "disabled", error: "Updates are off in development builds." } }),
    );
    expect(within(about()).getByText("Updates are off in development builds.")).toBeInTheDocument();
    expect(within(about()).getByRole("button", { name: "Check for updates" })).toBeDisabled();
  });

  it("copies diagnostics to the clipboard", async () => {
    const user = userEvent.setup();
    const { about, calls } = renderAbout(makeSnapshot(), { get_diagnostics: () => "Perch 0.2.0\nOS: Windows" });
    await user.click(within(about()).getByRole("button", { name: "Copy diagnostics" }));
    expect(calls).toContainEqual(["get_diagnostics", undefined]);
    expect(await within(about()).findByRole("button", { name: "Copied" })).toBeInTheDocument();
    await expect(navigator.clipboard.readText()).resolves.toBe("Perch 0.2.0\nOS: Windows");
  });

  it("goes back to Copy diagnostics after a moment", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const original = Object.getOwnPropertyDescriptor(navigator, "clipboard");
    try {
      const writeText = vi.fn(async () => {});
      Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
      const { about } = renderAbout(makeSnapshot(), { get_diagnostics: () => "diag" });
      await act(async () => {
        within(about()).getByRole("button", { name: "Copy diagnostics" }).click();
      });
      expect(writeText).toHaveBeenCalledWith("diag");
      expect(within(about()).getByRole("button", { name: "Copied" })).toBeInTheDocument();
      act(() => vi.advanceTimersByTime(3000));
      expect(within(about()).getByRole("button", { name: "Copy diagnostics" })).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
      if (original) Object.defineProperty(navigator, "clipboard", original);
      else delete (navigator as { clipboard?: unknown }).clipboard;
    }
  });

  it("opens the log folder and the third-party licenses", async () => {
    const { about, calls, transport } = renderAbout();
    await userEvent.click(within(about()).getByRole("button", { name: "Open log folder" }));
    expect(calls).toContainEqual(["open_log_folder", undefined]);
    await userEvent.click(within(about()).getByRole("button", { name: /Third-party licenses/ }));
    expect(transport.openUrl).toHaveBeenCalledWith(LICENSES_URL);
    expect(LICENSES_URL).toBe("https://github.com/yeetstick/perch/blob/main/THIRD_PARTY_NOTICES.md");
  });
});
