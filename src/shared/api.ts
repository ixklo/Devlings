import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  ChatTurn,
  HitRect,
  PermissionMode,
  PetEvent,
  PetInfo,
  PetOpen,
  SettingsView,
  Snapshot,
} from "./types";

type Unlisten = () => void;

/**
 * Everything the UI needs from the host. The Tauri transport is the real one;
 * `shared/mock.ts` swaps in a fake so the UI renders in a plain browser.
 */
export interface Transport {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, cb: (payload: T) => void): Promise<Unlisten>;
  startDragging(): Promise<void>;
  onMoved(cb: (pos: { x: number; y: number }) => void): Promise<Unlisten>;
  chooseFolder(): Promise<string | null>;
  chooseFile(): Promise<string | null>;
  openUrl(url: string): Promise<void>;
  appVersion(): Promise<string>;
}

const tauriTransport: Transport = {
  invoke: (cmd, args) => invoke(cmd, args),
  listen: (event, cb) => listen(event, (e) => cb(e.payload as never)),
  startDragging: () => getCurrentWindow().startDragging(),
  onMoved: (cb) => getCurrentWindow().onMoved(({ payload }) => cb({ x: payload.x, y: payload.y })),
  chooseFolder: async () => {
    const dir = await open({ directory: true, multiple: false });
    return typeof dir === "string" ? dir : null;
  },
  chooseFile: async () => {
    const file = await open({ directory: false, multiple: false });
    return typeof file === "string" ? file : null;
  },
  openUrl: (url) => openUrl(url),
  appVersion: () => getVersion(),
};

let transport: Transport = tauriTransport;

export function setTransport(t: Transport) {
  transport = t;
}

const call = <T>(cmd: string, args?: Record<string, unknown>) => transport.invoke<T>(cmd, args);

export const api = {
  // Existing commands (unchanged in v0.2).
  getSnapshot: () => call<Snapshot>("get_snapshot"),
  recheckSetup: () => call<Snapshot>("recheck_setup"),
  setPetName: (name: string) => call<Snapshot>("set_pet_name", { name }),
  installHooks: () => call<Snapshot>("install_hooks"),
  moveHooksPort: () => call<Snapshot>("move_hooks_port"),
  uninstallHooks: () => call<Snapshot>("uninstall_hooks"),
  declineHooks: () => call<Snapshot>("decline_hooks"),
  setClaudePath: (path: string | null) => call<Snapshot>("set_claude_path", { path }),
  addProject: (path: string) => call<Snapshot>("add_project", { path }),
  setPermissionMode: (project: string, mode: PermissionMode) => call<Snapshot>("set_permission_mode", { project, mode }),
  newConversation: (project: string) => call<Snapshot>("new_conversation", { project }),
  loadConversation: (project: string) => call<ChatTurn[]>("load_conversation", { project }),
  ask: (project: string, prompt: string) => call<void>("ask", { project, prompt }),
  stopAsk: (project: string) => call<void>("stop_ask", { project }),
  markCreditsNoticeSeen: () => call<Snapshot>("mark_credits_notice_seen"),
  finishOnboarding: () => call<Snapshot>("finish_onboarding"),
  setNotifications: (enabled: boolean) => call<Snapshot>("set_notifications", { enabled }),
  setLaunchAtLogin: (enabled: boolean) => call<Snapshot>("set_launch_at_login", { enabled }),
  savePetPosition: (x: number, y: number) => call<void>("save_pet_position", { x, y }),
  showPetMenu: () => call<void>("show_pet_menu"),

  // New in v0.2.
  setHitRegions: (regions: HitRect[]) => call<void>("set_hit_regions", { regions }),
  listPets: () => call<PetInfo[]>("list_pets"),
  getPetSprite: (id: string) => call<string>("get_pet_sprite", { id }),
  setPet: (id: string) => call<Snapshot>("set_pet", { id }),
  setPetScale: (scale: number) => call<Snapshot>("set_pet_scale", { scale }),
  markViewed: (sessionId: string) => call<Snapshot>("mark_viewed", { sessionId }),
  setFocusedThread: (sessionId: string | null) => call<void>("set_focused_thread", { sessionId }),
  openProject: (path: string) => call<void>("open_project", { path }),
  setThreadsCollapsed: (collapsed: boolean) => call<Snapshot>("set_threads_collapsed", { collapsed }),
  resetPetPosition: () => call<void>("reset_pet_position"),
  movePetBy: (dx: number, dy: number) => call<void>("move_pet_by", { dx, dy }),
  openSettings: (view: SettingsView) => call<void>("open_settings", { view }),
  closeSettings: () => call<void>("close_settings"),
  openPetsFolder: () => call<void>("open_pets_folder"),

  // Events.
  onSnapshot: (cb: (s: Snapshot) => void) => transport.listen<Snapshot>("snapshot", cb),
  onPetEvent: (cb: (e: PetEvent) => void) => transport.listen<PetEvent>("pet-event", cb),
  onPetOpen: (cb: (o: PetOpen) => void) => transport.listen<PetOpen>("pet-open", cb),
  onPetPointer: (cb: (id: string | null) => void) => transport.listen<string | null>("pet-pointer", cb),
  onSettingsView: (cb: (v: SettingsView) => void) => transport.listen<SettingsView>("settings-view", cb),

  // Window and OS helpers.
  startDragging: () => transport.startDragging(),
  onMoved: (cb: (pos: { x: number; y: number }) => void) => transport.onMoved(cb),
  chooseFolder: () => transport.chooseFolder(),
  chooseFile: () => transport.chooseFile(),
  openUrl: (url: string) => transport.openUrl(url),
  appVersion: () => transport.appVersion(),
};

export type Api = typeof api;
