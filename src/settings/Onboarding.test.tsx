import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Snapshot } from "../shared/types";
import { fakeTransport } from "../test/fakeTransport";
import { makeConfig, makeSetup, makeSnapshot } from "../test/fixtures";
import { COST_URL, Onboarding, STEPS } from "./Onboarding";

function firstRun(over: Partial<Snapshot> = {}): Snapshot {
  return makeSnapshot({
    config: makeConfig({ onboarded: false, approvalsIntroSeen: false }),
    setup: makeSetup({ hooksInstalled: false }),
    ...over,
  });
}

function start(snap = firstRun()) {
  const fake = fakeTransport({ list_pets: () => [] });
  const onDone = vi.fn();
  const utils = render(<Onboarding snap={snap} onDone={onDone} />);
  return { ...fake, ...utils, onDone };
}

const continueButton = () => screen.getByRole("button", { name: "Continue" });

describe("Onboarding", () => {
  it("has at most three steps", () => {
    start();
    expect(STEPS.length).toBeLessThanOrEqual(3);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuemax", String(STEPS.length));
    expect(screen.getByText(`Step 1 of ${STEPS.length} · Watch`)).toBeInTheDocument();
  });

  it("step 1 (Watch): picks and names the pet and explains the hooks, with the install button", async () => {
    const { calls } = start();
    expect(screen.getByRole("heading", { name: "Choose your pet" })).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Pet" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Watch your sessions" })).toBeInTheDocument();
    expect(screen.getByText(/shows every Claude Code session as a card/)).toBeInTheDocument();
    expect(screen.getByText(/backed up first/)).toBeInTheDocument();
    expect(screen.getByText(/remove them any time in Settings/)).toBeInTheDocument();

    await userEvent.clear(screen.getByRole("textbox", { name: "Name" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "Pip");
    // The explanation uses the name as it's typed.
    expect(screen.getByText(/Pip shows every Claude Code session/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Install hooks" }));
    expect(calls.map(([c]) => c)).toEqual(expect.arrayContaining(["set_pet_name", "install_hooks"]));
    expect(calls).toContainEqual(["set_pet_name", { name: "Pip" }]);
    expect(await screen.findByText("Step 2 of 3 · Ask")).toBeInTheDocument();
  });

  it("step 1: the Name box follows the pet's own name until you type in it", async () => {
    const { calls } = fakeTransport({ list_pets: () => [] });
    const snap = firstRun({ config: makeConfig({ onboarded: false, approvalsIntroSeen: false, petName: "Perch" }) });
    const { rerender } = render(<Onboarding snap={snap} onDone={vi.fn()} />);
    const box = screen.getByRole("textbox", { name: "Name" });
    expect(box).toHaveValue("Perch");
    // Picking the fox gave it the fox's name (a default name follows the pet).
    rerender(<Onboarding snap={{ ...snap, config: { ...snap.config, petId: "fox", petName: "Pip" } }} onDone={vi.fn()} />);
    expect(box).toHaveValue("Pip");
    expect(screen.getByText(/Pip shows every Claude Code session/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(await screen.findByText("Step 2 of 3 · Ask")).toBeInTheDocument();
    expect(calls.filter(([c]) => c === "set_pet_name")).toEqual([]);
  });

  it("step 1: a name you typed stays when you pick another pet", async () => {
    const { calls } = fakeTransport({ list_pets: () => [] });
    const snap = firstRun({ config: makeConfig({ onboarded: false, approvalsIntroSeen: false, petName: "Perch" }) });
    const { rerender } = render(<Onboarding snap={snap} onDone={vi.fn()} />);
    const box = screen.getByRole("textbox", { name: "Name" });
    await userEvent.clear(box);
    await userEvent.type(box, "Biscuit");
    rerender(<Onboarding snap={{ ...snap, config: { ...snap.config, petId: "cat", petName: "Miso" } }} onDone={vi.fn()} />);
    expect(box).toHaveValue("Biscuit");
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(calls).toContainEqual(["set_pet_name", { name: "Biscuit" }]);
  });

  it("step 1: Skip declines the hooks and still moves on", async () => {
    const { commands } = start();
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(commands()).toContain("decline_hooks");
    expect(commands()).not.toContain("install_hooks");
    expect(await screen.findByText("Step 2 of 3 · Ask")).toBeInTheDocument();
  });

  it("step 1: with hooks already installed (onboarding re-run), just continues", async () => {
    const { commands } = start(firstRun({ setup: makeSetup({ hooksInstalled: true }) }));
    expect(screen.queryByRole("button", { name: "Install hooks" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Skip" })).not.toBeInTheDocument();
    await userEvent.click(continueButton());
    expect(commands()).not.toContain("install_hooks");
    expect(await screen.findByText("Step 2 of 3 · Ask")).toBeInTheDocument();
  });

  it("step 2 (Ask): subscription, never an API key, the Agent SDK credit with a link, and the Claude Code check", async () => {
    const { calls, transport } = start();
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(await screen.findByRole("heading", { name: "Ask Claude Code from Mochi" })).toHaveFocus();
    expect(screen.getByText(/runs your own Claude Code on your subscription and never uses an API key/)).toBeInTheDocument();
    expect(screen.getByText(/stops it/)).toBeInTheDocument();
    expect(screen.getByText(/Agent SDK credit/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /How Asks are counted/ }));
    expect(transport.openUrl).toHaveBeenCalledWith(COST_URL);
    expect(COST_URL).toMatch(/#cost$/);
    expect(screen.getByText("Claude Code 2.1.282 found")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(calls).toContainEqual(["recheck_setup", undefined]);
  });

  it("step 2: says setup can wait when something's wrong", async () => {
    start(firstRun({ setup: makeSetup({ hooksInstalled: false, needsSetup: true, claudeError: "Claude Code wasn't found." }) }));
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(await screen.findByText(/fix this later from Settings/)).toBeInTheDocument();
  });

  it("step 3 (Permission prompts): explains the cards, and the toggle is Settings → Approvals, off by default", async () => {
    const { calls } = start(firstRun({ setup: makeSetup({ hooksInstalled: true }) }));
    await userEvent.click(continueButton());
    await userEvent.click(await screen.findByRole("button", { name: "Continue" }));
    expect(await screen.findByRole("heading", { name: "Answer permission prompts" })).toHaveFocus();
    expect(screen.getByText(/Allow and Deny/)).toBeInTheDocument();
    expect(screen.getByText(/On for Asks you start from Mochi/)).toBeInTheDocument();
    expect(screen.getByText(/Claude Code's own prompt still works too/)).toBeInTheDocument();
    const toggle = screen.getByRole("switch", { name: "Answer prompts from your other sessions too" });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(toggle).toHaveAccessibleDescription(/Settings → Approvals/);
    await userEvent.click(toggle);
    expect(calls).toContainEqual(["set_watch_approvals", { enabled: true }]);
  });

  it("step 3: the toggle waits for the hooks when they were skipped", async () => {
    start();
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    await userEvent.click(await screen.findByRole("button", { name: "Continue" }));
    const toggle = await screen.findByRole("switch", { name: "Answer prompts from your other sessions too" });
    expect(toggle).toBeDisabled();
    expect(toggle).toHaveAccessibleDescription(/Needs the hooks from step 1/);
  });

  it("Finish marks onboarding (and with it the approvals intro) as seen, then closes", async () => {
    const { commands, onDone } = start(firstRun({ setup: makeSetup({ hooksInstalled: true }) }));
    await userEvent.click(continueButton());
    await userEvent.click(await screen.findByRole("button", { name: "Continue" }));
    await userEvent.click(await screen.findByRole("button", { name: "Finish" }));
    // finish_onboarding sets both `onboarded` and `approvalsIntroSeen` (commands.rs).
    expect(commands()).toContain("finish_onboarding");
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(commands()).toContain("close_settings");
  });

  it("Back returns to the previous step", async () => {
    start();
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    await userEvent.click(await screen.findByRole("button", { name: "Back" }));
    expect(await screen.findByText("Step 1 of 3 · Watch")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Choose your pet" })).toHaveFocus();
  });
});
