import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ControlBar } from "./ControlBar";
import type { HiddenCount } from "./hiddenCount";
import css from "./pet.css?raw";

const bar = (notifications: boolean, systemNotificationsOff?: boolean) =>
  render(
    <ControlBar
      visible
      composerOpen={false}
      notifications={notifications}
      systemNotificationsOff={systemNotificationsOff}
      collapsed={false}
      onCompose={() => {}}
      onToggleNotifications={() => {}}
      onToggleCollapsed={() => {}}
    />,
  );

describe("ControlBar", () => {
  it("says on the bell when Windows won't show notifications even though they're on", () => {
    bar(true, true);
    expect(screen.getByRole("button", { name: "Notifications" })).toHaveAttribute(
      "title",
      "Notifications on, but Windows has them turned off",
    );
  });

  it("keeps the plain wording otherwise", () => {
    const { unmount } = bar(true);
    expect(screen.getByRole("button", { name: "Notifications" })).toHaveAttribute("title", "Notifications on");
    unmount();
    bar(false, true);
    expect(screen.getByRole("button", { name: "Notifications" })).toHaveAttribute("title", "Notifications off");
  });
});

describe("ControlBar hidden-threads count", () => {
  const chevronBar = (collapsed: boolean, hidden: HiddenCount | null, onToggleCollapsed = () => {}) =>
    render(
      <ControlBar
        visible
        composerOpen={false}
        notifications
        collapsed={collapsed}
        hidden={hidden}
        onCompose={() => {}}
        onToggleNotifications={() => {}}
        onToggleCollapsed={onToggleCollapsed}
      />,
    );
  const chevron = () => screen.getByRole("toolbar", { name: "Pet controls" }).querySelectorAll("button")[2];

  it("shows how many threads are hidden next to the chevron, in the icon colour", () => {
    chevronBar(true, { threads: 2, tone: "neutral", count: 2 });
    const button = screen.getByRole("button", { name: "Show threads, 2 hidden" });
    expect(button).toBe(chevron());
    expect(button).toHaveTextContent(/^2$/);
    expect(button).toHaveAttribute("title", "Show threads, 2 hidden");
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(button).toHaveClass("icon-btn", "has-count", "is-neutral");
    expect(button.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    // The number comes first, then the chevron: "2 ⌃".
    expect(button.firstElementChild).toHaveClass("icon-btn-count");
    expect(button.lastElementChild?.tagName.toLowerCase()).toBe("svg");
  });

  it("turns amber and shows how many need you", () => {
    const { rerender } = chevronBar(true, { threads: 3, tone: "wait", count: 2 });
    const button = screen.getByRole("button", { name: "Show threads, 3 hidden, 2 need you" });
    expect(button).toHaveTextContent(/^2$/);
    expect(button).toHaveAttribute("title", "Show threads, 3 hidden, 2 need you");
    expect(button).toHaveClass("has-count", "is-wait");
    rerender(
      <ControlBar
        visible
        composerOpen={false}
        notifications
        collapsed
        hidden={{ threads: 0, tone: "wait", count: 1 }}
        onCompose={() => {}}
        onToggleNotifications={() => {}}
        onToggleCollapsed={() => {}}
      />,
    );
    expect(chevron()).toHaveAccessibleName("Show threads, 1 needs you");
    expect(chevron()).toHaveTextContent(/^1$/);
    expect(chevron()).toHaveClass("is-wait");
  });

  it("uses the error tone and shows how many are blocked", () => {
    chevronBar(true, { threads: 3, tone: "err", count: 1 });
    const button = screen.getByRole("button", { name: "Show threads, 3 hidden, 1 blocked" });
    expect(button).toHaveTextContent(/^1$/);
    expect(button).toHaveAttribute("title", "Show threads, 3 hidden, 1 blocked");
    expect(button).toHaveClass("has-count", "is-err");
  });

  it("is the plain chevron with nothing hidden, or while the cards show", () => {
    const plain = (name: string) => {
      const button = chevron();
      expect(button).toHaveAccessibleName(name);
      expect(button).toHaveAttribute("title", name);
      expect(button).toHaveAttribute("class", "icon-btn");
      expect(button).toHaveTextContent(/^$/);
      expect(button.children).toHaveLength(1);
    };
    const { unmount } = chevronBar(true, null);
    plain("Show threads");
    expect(chevron()).toHaveAttribute("aria-expanded", "false");
    unmount();
    // A stale count never shows once the cards are back.
    chevronBar(false, { threads: 2, tone: "wait", count: 1 });
    plain("Hide threads");
    expect(chevron()).toHaveAttribute("aria-expanded", "true");
  });

  it("still toggles the threads on click", async () => {
    const onToggleCollapsed = vi.fn();
    chevronBar(true, { threads: 2, tone: "neutral", count: 2 }, onToggleCollapsed);
    await userEvent.click(chevron());
    expect(onToggleCollapsed).toHaveBeenCalledTimes(1);
  });

  it("keeps Left/Right moving between the buttons", async () => {
    chevronBar(true, { threads: 2, tone: "err", count: 1 });
    screen.getByRole("button", { name: "New message" }).focus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(chevron()).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "New message" })).toHaveFocus();
  });

  it("only grows the button sideways: nothing changes its height or lifts it out of the bar", () => {
    const blocks = [...css.matchAll(/([^{}]*\.has-count[^{}]*){([^}]*)}/g)];
    expect(blocks.length).toBeGreaterThan(0);
    for (const [, selector, body] of blocks) {
      expect(selector).toMatch(/\.control-bar \.icon-btn\.has-count/);
      expect(body).not.toMatch(/(^|[\s;])(height|min-height|max-height|position|top|bottom|margin[\w-]*|transform|padding(-top|-bottom)?)\s*:/);
    }
    // The tones use the checked colour pairs (contrast.test.ts).
    expect(css).toMatch(/\.has-count\.is-wait\s*{[^}]*background:\s*var\(--wait\);[^}]*color:\s*var\(--on-wait\);/);
    expect(css).toMatch(/\.has-count\.is-err\s*{[^}]*background:\s*var\(--err-ink\);[^}]*color:\s*var\(--on-color\);/);
  });
});
