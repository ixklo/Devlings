import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { makeConfig, makeSnapshot } from "../test/fixtures";
import { SettingsApp } from "./SettingsApp";

describe("SettingsApp", () => {
  it("shows onboarding while not onboarded, even if told to show settings", async () => {
    const { emit } = fakeTransport({
      get_snapshot: () => makeSnapshot({ config: makeConfig({ onboarded: false }) }),
      list_pets: () => [],
    });
    render(<SettingsApp />);
    expect(await screen.findByRole("heading", { name: "Choose your pet" })).toBeInTheDocument();
    act(() => emit("settings-view", "settings"));
    expect(screen.getByRole("heading", { name: "Choose your pet" })).toBeInTheDocument();
  });

  it("shows Settings once onboarded, and onboarding when asked for it", async () => {
    const { emit } = fakeTransport({ get_snapshot: () => makeSnapshot(), list_pets: () => [] });
    render(<SettingsApp />);
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByText(/Not affiliated with Anthropic/)).toBeInTheDocument();
    act(() => emit("settings-view", "onboarding"));
    expect(screen.getByRole("heading", { name: "Choose your pet" })).toBeInTheDocument();
  });
});
