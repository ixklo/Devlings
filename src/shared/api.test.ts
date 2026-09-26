import { describe, expect, it } from "vitest";
import { fakeTransport } from "../test/fakeTransport";
import { api } from "./api";

// Command and argument names are the contract with the Rust side (spec section 5).
describe("api contract", () => {
  it("sends v0.2 commands with camelCase args", async () => {
    const { calls } = fakeTransport();
    await api.setHitRegions([{ x: 1, y: 2, w: 3, h: 4 }]);
    await api.listPets();
    await api.getPetSprite("perch");
    await api.setPet("ember");
    await api.setPetScale(0.8);
    await api.markViewed("s1");
    await api.setFocusedThread(null);
    await api.openProject("C:\\code\\app");
    await api.setThreadsCollapsed(true);
    await api.resetPetPosition();
    await api.movePetBy(20, 0);
    await api.openSettings("onboarding");
    await api.closeSettings();
    await api.openPetsFolder();
    await api.showPetMenu();
    expect(calls).toEqual([
      ["set_hit_regions", { regions: [{ x: 1, y: 2, w: 3, h: 4 }] }],
      ["list_pets", undefined],
      ["get_pet_sprite", { id: "perch" }],
      ["set_pet", { id: "ember" }],
      ["set_pet_scale", { scale: 0.8 }],
      ["mark_viewed", { sessionId: "s1" }],
      ["set_focused_thread", { sessionId: null }],
      ["open_project", { path: "C:\\code\\app" }],
      ["set_threads_collapsed", { collapsed: true }],
      ["reset_pet_position", undefined],
      ["move_pet_by", { dx: 20, dy: 0 }],
      ["open_settings", { view: "onboarding" }],
      ["close_settings", undefined],
      ["open_pets_folder", undefined],
      ["show_pet_menu", undefined],
    ]);
  });

  it("keeps the v0.1 commands unchanged", async () => {
    const { calls } = fakeTransport();
    await api.ask("p", "hi");
    await api.stopAsk("p");
    await api.setPermissionMode("p", "auto");
    await api.savePetPosition(4, 5);
    await api.setNotifications(false);
    expect(calls).toEqual([
      ["ask", { project: "p", prompt: "hi" }],
      ["stop_ask", { project: "p" }],
      ["set_permission_mode", { project: "p", mode: "auto" }],
      ["save_pet_position", { x: 4, y: 5 }],
      ["set_notifications", { enabled: false }],
    ]);
  });

  it("listens on the spec's event names", async () => {
    const { transport } = fakeTransport();
    await api.onSnapshot(() => {});
    await api.onPetEvent(() => {});
    await api.onPetOpen(() => {});
    await api.onPetPointer(() => {});
    await api.onSettingsView(() => {});
    expect((transport.listen as unknown as { mock: { calls: unknown[][] } }).mock.calls.map((c) => c[0])).toEqual([
      "snapshot",
      "pet-event",
      "pet-open",
      "pet-pointer",
      "settings-view",
    ]);
  });
});
