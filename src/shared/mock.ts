// Browser preview: a fake backend so `npm run dev` renders the real UI in a
// normal browser (/?window=pet, /?window=settings). It implements the same
// command and event names as the Rust side, with made-up threads and a
// streamed reply.
//
// Preview switches (query string): open=compose|thread, setup=1,
// collapsed=1, empty=1, onboarding=1, credits=seen, scale=0.45|0.6|0.8,
// debug=hits (outlines the click-through regions).

import { placeholderAtlas } from "../sprite/placeholderAtlas";
import { samePath } from "./paths";
import { sortThreads } from "./threads";
import type { Transport } from "./api";
import type {
  ChatTurn,
  Config,
  HitRect,
  PermissionMode,
  PetEvent,
  PetInfo,
  PetState,
  ProjectEntry,
  SetupStatus,
  Snapshot,
  ThreadInfo,
} from "./types";

type Listener = (payload: unknown) => void;

const params = new URLSearchParams(typeof window !== "undefined" ? window.location.search : "");
const MIN = 60_000;
const ROOT = "C:\\Users\\you\\code\\";

const PETS: PetInfo[] = [
  { id: "perch", displayName: "Perch", description: "A teal songbird that keeps an eye on your sessions.", source: "bundled" },
  { id: "ember", displayName: "Ember", description: "A warm orange finch with a lot of energy.", source: "bundled" },
  { id: "plum", displayName: "Plum", description: "A calm purple bird, unbothered by long builds.", source: "bundled" },
  { id: "sprout", displayName: "Sprout", description: "A green bird from your Codex pets folder.", source: "codex" },
];

const REPLY = `The flaky test comes from a race in \`useSnapshot\`: the first \`get_snapshot\` can resolve **after** a newer \`snapshot\` event, so the stale value wins.

Fix it by ignoring the initial result once an event has arrived:

\`\`\`ts
let fresh = false;
api.getSnapshot().then((s) => alive && !fresh && setSnap(s));
const un = api.onSnapshot((s) => {
  fresh = true;
  setSnap(s);
});
\`\`\`

I made that change in \`src/shared/useSnapshot.ts\` and the suite now passes 20 runs in a row.`;

const HISTORY: Record<string, ChatTurn[]> = {
  [`${ROOT}perch`]: [
    { role: "user", text: "Why does the settings window flash white on open?" },
    {
      role: "assistant",
      text: "The window is created visible before the webview paints. Create it hidden and call `show()` after the first frame, or set `backgroundColor` to match the theme.",
    },
    { role: "user", text: "The snapshot test is flaky on CI. Can you find out why?" },
  ],
};

function project(name: string, minsAgo: number, mode: PermissionMode = "edit_files", ask = false): ProjectEntry {
  return {
    path: ROOT + name,
    name,
    lastSeen: Date.now() - minsAgo * MIN,
    permissionMode: mode,
    askSessionId: ask ? `ask-${name}` : null,
    transcriptPath: null,
  };
}

function thread(
  name: string,
  source: ThreadInfo["source"],
  status: ThreadInfo["status"],
  minsAgo: number,
  label: string | null,
  excerpt: string | null = null,
): ThreadInfo {
  return {
    sessionId: source === "ask" ? `ask-${name}` : `watch-${name}`,
    project: ROOT + name,
    projectName: name,
    source,
    status,
    label,
    excerpt,
    updatedAt: Date.now() - minsAgo * MIN,
    unread: status === "ready" || status === "blocked",
  };
}

function initialThreads(): ThreadInfo[] {
  if (params.get("empty")) return [];
  return [
    thread("billing-service", "watch", "needs_input", 1, "Wants to run: npm install stripe@18"),
    thread("perch", "ask", "running", 0, "Reading src/shared/useSnapshot.ts"),
    thread("docs-site", "watch", "ready", 4, "Done", "Updated the install guide and fixed 3 broken links in the FAQ."),
    thread("ml-notebooks", "watch", "blocked", 12, "Failed", "Plan limit reached. Resets in 1h 12m."),
    thread("dotfiles", "watch", "running", 2, "Editing .config/nvim/init.lua"),
  ];
}

function healthySetup(): SetupStatus {
  return {
    claudePath: "C:\\Users\\you\\.local\\bin\\claude.exe",
    claudeVersion: "2.1.282",
    claudeError: null,
    hooksInstalled: true,
    auth: { status: "allowed", subscription: "max" },
    hookServerError: null,
    setupHint: null,
    needsSetup: false,
  };
}

function brokenSetup(): SetupStatus {
  return {
    ...healthySetup(),
    claudePath: null,
    claudeVersion: null,
    claudeError: "Claude Code wasn't found. Install it, or choose the file in Settings.",
    auth: null,
    hooksInstalled: false,
    needsSetup: true,
  };
}

const PRIORITY: PetState[] = ["needs_input", "blocked", "ready", "running"];

class MockBackend {
  listeners = new Map<string, Set<Listener>>();
  config: Config = {
    petName: "Perch",
    onboarded: !params.get("onboarding"),
    creditsNoticeSeen: params.get("credits") === "seen",
    hooksDeclined: false,
    hookPort: 49152,
    hookToken: "preview",
    claudePath: null,
    petPosition: null,
    notifications: true,
    launchAtLogin: false,
    petId: "perch",
    petScale: Number(params.get("scale")) || 0.6,
    threadsCollapsed: !!params.get("collapsed"),
  };
  projects: ProjectEntry[] = [
    project("perch", 0, "edit_files", true),
    project("billing-service", 1),
    project("docs-site", 4, "read_only"),
    project("dotfiles", 30, "auto"),
    project("ml-notebooks", 60 * 5),
  ];
  threads: ThreadInfo[] = initialThreads();
  running: string[] = params.get("empty") ? [] : [`${ROOT}perch`];
  setup: SetupStatus = params.get("setup") ? brokenSetup() : healthySetup();
  timers: number[] = [];

  constructor() {
    // Keep the preview alive: the running watch thread keeps changing steps.
    const steps = ["Editing .config/nvim/init.lua", "Running: nvim --headless +checkhealth", "Reading lua/plugins.lua"];
    let i = 0;
    window.setInterval(() => {
      const t = this.threads.find((x) => x.sessionId === "watch-dotfiles" && x.status === "running");
      if (!t) return;
      t.label = steps[++i % steps.length];
      t.updatedAt = Date.now();
      this.publish();
    }, 7000);
    // The Ask that's already running in the preview finishes after a while.
    if (this.running.length) this.stream(`${ROOT}perch`, "ask-perch", 2600, false);
  }

  snapshot(): Snapshot {
    const visible = sortThreads(this.threads.filter((t) => t.status !== "idle"));
    const top = PRIORITY.find((s) => visible.some((t) => t.status === s));
    const petState: PetState = this.setup.needsSetup ? "setup" : (top ?? "idle");
    return {
      config: { ...this.config },
      petState,
      threads: visible.map((t) => ({ ...t })),
      projects: this.projects.map((p) => ({ ...p })),
      running: [...this.running],
      setup: { ...this.setup },
    };
  }

  emit(event: string, payload: unknown) {
    this.listeners.get(event)?.forEach((cb) => cb(payload));
  }

  publish() {
    const s = this.snapshot();
    this.emit("snapshot", s);
    return s;
  }

  petEvent(ev: Omit<PetEvent, "at" | "source">) {
    this.emit("pet-event", { ...ev, source: "ask", at: Date.now() });
  }

  upsertThread(t: ThreadInfo) {
    this.threads = [t, ...this.threads.filter((x) => x.sessionId !== t.sessionId)];
  }

  stream(path: string, sessionId: string, delay = 700, announce = true) {
    const name = path.split("\\").pop() ?? path;
    const later = (ms: number, fn: () => void) => this.timers.push(window.setTimeout(fn, ms));
    if (announce) later(150, () => this.petEvent({ sessionId, project: path, kind: "prompt" }));
    later(delay * 0.5, () => {
      this.petEvent({ sessionId, project: path, kind: "step", label: "Reading src/shared/useSnapshot.ts" });
      const t = this.threads.find((x) => x.sessionId === sessionId);
      if (t) (t.label = "Reading src/shared/useSnapshot.ts"), (t.updatedAt = Date.now()), this.publish();
    });
    const words = REPLY.split(/(?<=\s)/);
    let at = delay;
    for (let i = 0; i < words.length; i += 3) {
      const chunk = words.slice(i, i + 3).join("");
      at += 45;
      later(at, () => this.running.some((r) => samePath(r, path)) && this.petEvent({ sessionId, project: path, kind: "reply_delta", text: chunk }));
    }
    later(at + 200, () => {
      if (!this.running.some((r) => samePath(r, path))) return;
      this.running = this.running.filter((r) => !samePath(r, path));
      this.petEvent({ sessionId, project: path, kind: "done", label: "Done", text: REPLY });
      this.upsertThread({
        sessionId,
        project: path,
        projectName: name,
        source: "ask",
        status: "ready",
        label: "Done",
        excerpt: "Fixed the race in useSnapshot; the suite passes 20 runs in a row.",
        updatedAt: Date.now(),
        unread: true,
      });
      this.publish();
    });
  }

  invoke(cmd: string, a: Record<string, unknown> = {}): unknown {
    const c = this.config;
    switch (cmd) {
      case "get_snapshot":
        return this.snapshot();
      case "recheck_setup":
        this.setup = healthySetup();
        return this.publish();
      case "set_pet_name": {
        const name = String(a.name ?? "").trim();
        if (!name) throw "Give your pet a name.";
        if (name.length > 24) throw "Keep the name under 24 characters.";
        c.petName = name;
        return this.publish();
      }
      case "install_hooks":
        this.setup.hooksInstalled = true;
        c.hooksDeclined = false;
        return this.publish();
      case "move_hooks_port":
        c.hookPort = 49153;
        this.setup.hookServerError = null;
        return this.publish();
      case "uninstall_hooks":
        this.setup.hooksInstalled = false;
        return this.publish();
      case "decline_hooks":
        c.hooksDeclined = true;
        return this.publish();
      case "set_claude_path":
        c.claudePath = (a.path as string | null) ?? null;
        this.setup = healthySetup();
        return this.publish();
      case "add_project": {
        const path = String(a.path);
        if (!this.projects.some((p) => samePath(p.path, path))) {
          this.projects.push({ ...project(path.split(/[\\/]/).pop() || path, 0), path });
        }
        return this.publish();
      }
      case "set_permission_mode": {
        const p = this.projects.find((x) => samePath(x.path, String(a.project)));
        if (p) p.permissionMode = a.mode as PermissionMode;
        return this.publish();
      }
      case "new_conversation":
        delete HISTORY[String(a.project)];
        return this.publish();
      case "load_conversation": {
        const key = Object.keys(HISTORY).find((k) => samePath(k, String(a.project)));
        return key ? HISTORY[key].map((t) => ({ ...t })) : [];
      }
      case "ask": {
        const path = String(a.project);
        if (!String(a.prompt ?? "").trim()) throw "Type something first.";
        if (!c.creditsNoticeSeen) throw "credits_notice";
        if (this.running.some((r) => samePath(r, path))) throw "Already working on this project.";
        const p = this.projects.find((x) => samePath(x.path, path));
        if (p) p.lastSeen = Date.now();
        const sessionId = p?.askSessionId ?? `ask-${Date.now()}`;
        if (p) p.askSessionId = sessionId;
        this.running.push(path);
        this.upsertThread({
          sessionId,
          project: path,
          projectName: p?.name ?? path,
          source: "ask",
          status: "running",
          label: "Thinking…",
          excerpt: null,
          updatedAt: Date.now(),
          unread: false,
        });
        this.publish();
        this.stream(path, sessionId);
        return null;
      }
      case "stop_ask": {
        const path = String(a.project);
        if (!this.running.some((r) => samePath(r, path))) return null;
        this.running = this.running.filter((r) => !samePath(r, path));
        const t = this.threads.find((x) => x.source === "ask" && samePath(x.project, path));
        if (t) (t.status = "idle"), (t.updatedAt = Date.now());
        this.petEvent({ sessionId: t?.sessionId ?? "", project: path, kind: "ended", label: "Stopped" });
        this.publish();
        return null;
      }
      case "mark_credits_notice_seen":
        c.creditsNoticeSeen = true;
        return this.publish();
      case "finish_onboarding":
        c.onboarded = true;
        return this.publish();
      case "set_notifications":
        c.notifications = !!a.enabled;
        return this.publish();
      case "set_launch_at_login":
        c.launchAtLogin = !!a.enabled;
        return this.publish();
      case "save_pet_position":
        c.petPosition = [Number(a.x), Number(a.y)];
        return null;
      case "show_pet_menu":
        console.info("[preview] show_pet_menu: Settings, Change pet, Hide for 1 hour, Quit");
        return null;
      case "set_hit_regions":
        drawHits(a.regions as HitRect[]);
        return null;
      case "list_pets":
        return PETS.map((p) => ({ ...p }));
      case "get_pet_sprite": {
        const url = placeholderAtlas(String(a.id));
        if (!url) throw "Couldn't draw the preview sprite.";
        return url;
      }
      case "set_pet":
        if (!PETS.some((p) => p.id === a.id)) throw "That pet isn't installed.";
        c.petId = String(a.id);
        return this.publish();
      case "set_pet_scale":
        c.petScale = Math.min(1, Math.max(0.4, Number(a.scale)));
        return this.publish();
      case "mark_viewed": {
        const t = this.threads.find((x) => x.sessionId === a.sessionId);
        if (t) {
          t.unread = false;
          if (t.status === "ready" || t.status === "blocked") t.status = "idle";
        }
        return this.publish();
      }
      case "set_focused_thread":
      case "reset_pet_position":
      case "move_pet_by":
      case "close_settings":
        return null;
      case "open_project":
        console.info(`[preview] open_project ${String(a.path)}`);
        return null;
      case "set_threads_collapsed":
        c.threadsCollapsed = !!a.collapsed;
        return this.publish();
      case "open_settings":
        window.open(`/?window=settings${a.view === "onboarding" ? "&onboarding=1" : ""}`, "perch-settings", "width=480,height=680");
        return null;
      case "open_pets_folder":
        console.info("[preview] open_pets_folder");
        return null;
      default:
        throw `Unknown command: ${cmd}`;
    }
  }
}

let hitLayer: HTMLDivElement | null = null;
function drawHits(regions: HitRect[]) {
  if (params.get("debug") !== "hits") return;
  if (!hitLayer) {
    hitLayer = document.createElement("div");
    hitLayer.style.cssText = "position:fixed;inset:0;pointer-events:none;z-index:99";
    document.body.appendChild(hitLayer);
  }
  hitLayer.innerHTML = regions
    .map(
      (r) =>
        `<div style="position:absolute;left:${r.x - 6}px;top:${r.y - 6}px;width:${r.w + 12}px;height:${r.h + 12}px;outline:1px dashed #e5484d;background:rgb(229 72 77 / .06)"></div>`,
    )
    .join("");
}

export function createMockTransport(): Transport {
  const backend = new MockBackend();
  const delay = <T,>(fn: () => T) =>
    new Promise<T>((resolve, reject) =>
      window.setTimeout(() => {
        try {
          resolve(fn());
        } catch (e) {
          reject(e);
        }
      }, 40),
    );

  let pendingOpen = params.get("open");
  let lastMove: { x: number; y: number } | null = null;
  const moveListeners = new Set<(p: { x: number; y: number }) => void>();

  const transport: Transport = {
    invoke: <T,>(cmd: string, args?: Record<string, unknown>) => delay(() => backend.invoke(cmd, args) as T),
    listen: async <T,>(event: string, cb: (payload: T) => void) => {
      const set = backend.listeners.get(event) ?? new Set<Listener>();
      // A fresh wrapper per call, like Tauri's per-listen handler ids, so
      // registering the same callback twice (StrictMode) stays two entries.
      const entry: Listener = (payload) => cb(payload as T);
      set.add(entry);
      backend.listeners.set(event, set);
      // `?open=` acts like the tray: fire once the pet is listening and has a snapshot.
      if (event === "pet-open" && pendingOpen) {
        const view = pendingOpen;
        pendingOpen = null;
        window.setTimeout(() => {
          backend.emit("pet-open", view === "thread" ? { view: "thread", sessionId: "ask-perch" } : { view: "compose" });
        }, 400);
      }
      return () => void set.delete(entry);
    },
    // Dragging can't move a browser tab; report pointer moves as window moves
    // so the running animation can still be previewed.
    startDragging: async () => {
      const onMove = (e: PointerEvent) => {
        lastMove = { x: e.screenX, y: e.screenY };
        moveListeners.forEach((cb) => cb(lastMove!));
      };
      const onUp = () => {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
      };
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    },
    onMoved: async (cb) => {
      const entry = (p: { x: number; y: number }) => cb(p);
      moveListeners.add(entry);
      return () => void moveListeners.delete(entry);
    },
    chooseFolder: async () => `${ROOT}new-project`,
    chooseFile: async () => "C:\\Users\\you\\.local\\bin\\claude.exe",
    openUrl: async (url) => {
      window.open(url, "_blank", "noopener");
    },
    appVersion: async () => "0.2.0",
  };

  return transport;
}
