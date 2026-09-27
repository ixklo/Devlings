import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ControlBar } from "./ControlBar";

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
