import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ChatTurn, PermissionMode, PetEvent, Snapshot } from "./types";

export const api = {
  getSnapshot: () => invoke<Snapshot>("get_snapshot"),
  recheckSetup: () => invoke<Snapshot>("recheck_setup"),
  setPetName: (name: string) => invoke<Snapshot>("set_pet_name", { name }),
  installHooks: () => invoke<Snapshot>("install_hooks"),
  moveHooksPort: () => invoke<Snapshot>("move_hooks_port"),
  uninstallHooks: () => invoke<Snapshot>("uninstall_hooks"),
  declineHooks: () => invoke<Snapshot>("decline_hooks"),
  setClaudePath: (path: string | null) => invoke<Snapshot>("set_claude_path", { path }),
  addProject: (path: string) => invoke<Snapshot>("add_project", { path }),
  setPermissionMode: (project: string, mode: PermissionMode) =>
    invoke<Snapshot>("set_permission_mode", { project, mode }),
  newConversation: (project: string) => invoke<Snapshot>("new_conversation", { project }),
  loadConversation: (project: string) => invoke<ChatTurn[]>("load_conversation", { project }),
  ask: (project: string, prompt: string) => invoke<void>("ask", { project, prompt }),
  stopAsk: (project: string) => invoke<void>("stop_ask", { project }),
  markCreditsNoticeSeen: () => invoke<Snapshot>("mark_credits_notice_seen"),
  finishOnboarding: () => invoke<Snapshot>("finish_onboarding"),
  setNotifications: (enabled: boolean) => invoke<Snapshot>("set_notifications", { enabled }),
  setLaunchAtLogin: (enabled: boolean) => invoke<Snapshot>("set_launch_at_login", { enabled }),
  togglePanel: () => invoke<void>("toggle_panel"),
  closePanel: () => invoke<void>("close_panel"),
  showPetMenu: () => invoke<void>("show_pet_menu"),
  savePetPosition: (x: number, y: number) => invoke<void>("save_pet_position", { x, y }),
  onSnapshot: (cb: (s: Snapshot) => void) => listen<Snapshot>("snapshot", (e) => cb(e.payload)),
  onPetEvent: (cb: (e: PetEvent) => void) => listen<PetEvent>("pet-event", (e) => cb(e.payload)),
  onBubble: (cb: (text: string) => void) => listen<string>("pet-bubble", (e) => cb(e.payload)),
  onPanelView: (cb: (view: string) => void) => listen<string>("panel-view", (e) => cb(e.payload)),
};
