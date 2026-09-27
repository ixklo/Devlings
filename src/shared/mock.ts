// Browser preview: a fake backend so `npm run dev` renders the real UI in a
// normal browser (/?window=pet, /?window=settings). It implements the same
// command and event names as the Rust side, with made-up threads and a
// streamed reply.
//
// Preview switches (query string): open=compose|thread, setup=1,
// collapsed=1, empty=1, onboarding=1, credits=seen, scale=0.45|0.6|0.8,
// update=ready|downloading|error|disabled|none (none: checks find nothing),
// untrusted=1 (every Ask shows the untrusted-folder notice; without it, only
// Asks in billing-service and ml-notebooks do, until trusted),
// approvals=none|mcp|ask (no sample permission card; or also an MCP tool
// card; or also one inline in the running Ask's chat), intro=1 (the one-time
// "answer permission prompts" card; hidden while watching is on, so it also
// starts with watching off),
// debug=hits (outlines the click-through regions).

import { placeholderAtlas } from "../sprite/placeholderAtlas";
import { samePath } from "./paths";
import { sortThreads } from "./threads";
import type { Transport } from "./api";
import type {
  ChatTurn,
  Config,
  HitRect,
  PendingApproval,
  PermissionMode,
  PetEvent,
  PetInfo,
  PetState,
  ProjectEntry,
  SetupStatus,
  Snapshot,
  ThreadInfo,
  UpdateStatus,
  UsageInfo,
} from "./types";

type Listener = (payload: unknown) => void;

const params = new URLSearchParams(typeof window !== "undefined" ? window.location.search : "");
const MIN = 60_000;
const ROOT = "C:\\Users\\you\\code\\";

const PETS: PetInfo[] = [
  { id: "perch", displayName: "Perch", description: "A teal songbird that keeps an eye on your sessions.", source: "bundled" },
  { id: "ember", displayName: "Ember", description: "A warm orange finch with a lot of energy.", source: "bundled" },
  { id: "plum", displayName: "Plum", description: "A calm purple bird, unbothered by long builds.", source: "bundled" },
  { id: "fox", displayName: "Pip", description: "A russet fox with a white-tipped tail.", source: "bundled" },
  { id: "cat", displayName: "Miso", description: "A charcoal cat with amber eyes.", source: "bundled" },
  { id: "axolotl", displayName: "Nori", description: "A pink axolotl with fluttering frills.", source: "bundled" },
  { id: "capybara", displayName: "Bean", description: "A calm capybara with an orange on its head.", source: "bundled" },
  { id: "robot", displayName: "Bolt", description: "A steel robot with a blue visor.", source: "bundled" },
  { id: "ghost", displayName: "Wisp", description: "A soft little floating ghost.", source: "bundled" },
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
  [`${ROOT}devlings`]: [
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
    trusted: false,
  };
}

// What the preview's untrusted folders "have", as the backend would list it (v1.0 D6).
const UNTRUSTED_FINDINGS: Record<string, string> = {
  "billing-service":
    "Skipped: hooks (PreToolUse, Stop) · environment variables (STRIPE_API_BASE) · MCP servers (github, postgres) · apiKeyHelper · 2 project skills.",
  "ml-notebooks": "Skipped: MCP servers (jupyter).",
};
const DEFAULT_FINDINGS = "Skipped: hooks (SessionStart) · MCP servers (github).";

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
    thread("devlings", "ask", "running", 0, "Reading src/shared/useSnapshot.ts"),
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
    systemNotificationsOff: false,
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
const NEXT_VERSION = "1.0.1";
const HOLDS = [30, 60, 120, 240];

/** A watched session's permission request (billing-service already shows as "needs input"), and optionally the running Ask's. */
function initialApprovals(holdSecs: number): PendingApproval[] {
  if (params.get("empty") || params.get("approvals") === "none") return [];
  const list: PendingApproval[] = [
    {
      id: "preview-watch-1",
      sessionId: "watch-billing-service",
      project: `${ROOT}billing-service`,
      projectName: "billing-service",
      source: "watch",
      toolName: "Bash",
      rawToolName: null,
      summary: "npm install stripe@18",
      description: "Install the Stripe SDK",
      details: JSON.stringify({ command: "npm install stripe@18", description: "Install the Stripe SDK" }, null, 2),
      lossy: false,
      tooLong: false,
      risks: [],
      canAlwaysAllow: true,
      alwaysLabel: "Always allow",
      alwaysDetail: "Adds the rule Bash(npm install:*) to this project's local settings",
      expiresAt: Date.now() + holdSecs * 1000,
    },
  ];
  if (params.get("approvals") === "mcp") {
    const input = { owner: "you", repo: "billing-service", title: "Flaky webhook test", body: "Seen twice on CI today." };
    list.push({
      id: "preview-watch-2",
      sessionId: "watch-docs-site",
      project: `${ROOT}docs-site`,
      projectName: "docs-site",
      source: "watch",
      toolName: "Create issue",
      rawToolName: "mcp__github__create_issue",
      summary: JSON.stringify(input),
      description: null,
      details: JSON.stringify(input, null, 2),
      lossy: true,
      tooLong: false,
      risks: [],
      canAlwaysAllow: false,
      alwaysLabel: null,
      alwaysDetail: null,
      expiresAt: Date.now() + holdSecs * 1000,
    });
  }
  if (params.get("approvals") === "ask") {
    const file = `${ROOT}devlings\\src\\shared\\useSnapshot.ts`;
    list.push({
      id: "preview-ask-1",
      sessionId: "ask-devlings",
      project: `${ROOT}devlings`,
      projectName: "devlings",
      source: "ask",
      toolName: "Edit",
      rawToolName: null,
      summary: file,
      description: null,
      details: JSON.stringify({ file_path: file, old_string: "let fresh = false;", new_string: "let fresh = true;" }, null, 2),
      lossy: false,
      tooLong: false,
      risks: [],
      canAlwaysAllow: true,
      alwaysLabel: "Allow for this session",
      alwaysDetail: "Lets Claude Code edit files for the rest of this session",
      expiresAt: null,
    });
  }
  return list;
}

function initialUpdate(): UpdateStatus {
  switch (params.get("update")) {
    case "ready":
      return { state: "ready", version: NEXT_VERSION, notes: "Bug fixes." };
    case "downloading":
      return { state: "downloading", version: NEXT_VERSION, progress: 42 };
    case "error":
      return { state: "error", error: "Couldn't check for updates." };
    case "disabled":
      return { state: "disabled", error: "Updates are off in development builds." };
    default:
      return { state: "idle" };
  }
}

const DIAGNOSTICS = `Devlings 0.2.0 (browser preview)
OS: Windows 11 (x86_64)
Claude Code: 2.1.282 at ~/.local/bin/claude.exe
Hooks: installed on port 49152
Auto-update: on`;

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
    petPositionMigrated: true,
    notifications: true,
    launchAtLogin: false,
    petId: "perch",
    petScale: Number(params.get("scale")) || 0.6,
    threadsCollapsed: !!params.get("collapsed"),
    autoUpdate: true,
    lastUpdateCheck: null,
    watchApprovals: !params.get("intro"),
    approvalHoldSecs: 60,
    approvalsIntroSeen: !params.get("intro"),
  };
  approvals: PendingApproval[] = initialApprovals(this.config.approvalHoldSecs);
  projects: ProjectEntry[] = [
    project("devlings", 0, "edit_files", true),
    project("billing-service", 1),
    project("docs-site", 4, "read_only"),
    project("dotfiles", 30, "auto"),
    project("ml-notebooks", 60 * 5),
  ];
  threads: ThreadInfo[] = initialThreads();
  running: string[] = params.get("empty") ? [] : [`${ROOT}devlings`];
  setup: SetupStatus = params.get("setup") ? brokenSetup() : healthySetup();
  update: UpdateStatus = initialUpdate();
  // Plan usage from "the last Ask" (v1.0 S4); ?usage=warning or ?usage=rejected previews the mini chat's note.
  usage: UsageInfo | null = params.get("empty")
    ? null
    : {
        status: params.get("usage") ?? "allowed",
        utilization: params.get("usage") === "rejected" ? 1 : params.get("usage") === "warning" ? 0.82 : 0.42,
        kind: "five_hour",
        resetsAt: Date.now() + 65 * 60_000,
        seenAt: Date.now() - 25 * 60_000,
      };
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
    // Like a watch hold ending: the card goes and the session keeps its "needs input" card.
    window.setInterval(() => {
      const now = Date.now();
      const left = this.approvals.filter((a) => a.expiresAt === null || a.expiresAt > now);
      if (left.length === this.approvals.length) return;
      this.approvals = left;
      this.publish();
    }, 1000);
    // The Ask that's already running in the preview finishes after a while.
    if (this.running.length) this.stream(`${ROOT}devlings`, "ask-devlings", 2600, false);
  }

  snapshot(): Snapshot {
    const visible = sortThreads(this.threads.filter((t) => t.status !== "idle"));
    const top = PRIORITY.find((s) => visible.some((t) => t.status === s));
    const petState: PetState = this.setup.needsSetup ? "setup" : this.approvals.length ? "needs_input" : (top ?? "idle");
    return {
      config: { ...this.config },
      petState,
      threads: visible.map((t) => ({ ...t })),
      projects: this.projects.map((p) => ({ ...p })),
      running: [...this.running],
      setup: { ...this.setup },
      update: { ...this.update },
      approvals: this.approvals.map((a) => ({ ...a })),
      usage: this.usage && { ...this.usage },
    };
  }

  /** Like `answer_approval`: the first answer wins, and the thread goes back to work. */
  answerApproval(id: string, decision: string) {
    const a = this.approvals.find((x) => x.id === id);
    if (!a) throw "That request was already answered or is no longer waiting.";
    if (decision === "always" && !a.canAlwaysAllow) throw "Claude Code didn't offer a rule for this request.";
    this.approvals = this.approvals.filter((x) => x.id !== id);
    const t = this.threads.find((x) => x.sessionId === a.sessionId);
    if (t && !this.approvals.some((x) => x.sessionId === a.sessionId)) {
      t.status = "running";
      t.label = decision === "deny" ? `Declined: ${a.toolName}` : `Running ${a.summary}`;
      t.updatedAt = Date.now();
    }
    console.info(`[preview] answer_approval ${decision}: ${a.summary}`);
    return this.publish();
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

  /** The untrusted-folder notice's "Skipped: …" line for an Ask in `path`, or null when it runs normally. */
  untrustedFindings(path: string): string | null {
    const p = this.projects.find((x) => samePath(x.path, path));
    if (p?.trusted) return null;
    const name = path.split(/[\\/]/).pop() ?? path;
    return UNTRUSTED_FINDINGS[name] ?? (params.get("untrusted") ? DEFAULT_FINDINGS : null);
  }

  stream(path: string, sessionId: string, delay = 700, announce = true) {
    const name = path.split("\\").pop() ?? path;
    const later = (ms: number, fn: () => void) => this.timers.push(window.setTimeout(fn, ms));
    const skipped = this.untrustedFindings(path);
    if (skipped) {
      // Like the backend: sent once per run, before anything the run says. The preview's already-running Ask
      // waits until its thread view (?open=thread) is listening.
      later(announce ? 60 : delay * 0.3, () =>
        this.petEvent({
          sessionId: "",
          project: path,
          kind: "untrusted",
          label: `This folder isn't trusted in Claude Code yet, so ${this.config.petName} ran without its project settings.`,
          text: skipped,
        }),
      );
    }
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

  setUpdate(update: UpdateStatus) {
    this.update = update;
    this.publish();
  }

  /** Like the backend: the command resolves after the check; the download carries on in the background. */
  checkForUpdate(): Promise<UpdateStatus> | UpdateStatus {
    if (["checking", "available", "downloading", "ready", "disabled"].includes(this.update.state)) return { ...this.update };
    this.config.lastUpdateCheck = Date.now();
    this.setUpdate({ state: "checking" });
    return new Promise((resolve) =>
      window.setTimeout(() => {
        if (params.get("update") === "none") {
          this.setUpdate({ state: "idle" });
          return resolve({ ...this.update });
        }
        this.setUpdate({ state: "available", version: NEXT_VERSION, notes: "Bug fixes." });
        resolve({ ...this.update });
        let progress = 0;
        const tick = window.setInterval(() => {
          progress += 10;
          if (progress > 100) {
            window.clearInterval(tick);
            this.setUpdate({ state: "ready", version: NEXT_VERSION, notes: "Bug fixes." });
            return;
          }
          this.setUpdate({ state: "downloading", version: NEXT_VERSION, notes: "Bug fixes.", progress });
        }, 200);
      }, 900),
    );
  }

  invoke(cmd: string, a: Record<string, unknown> = {}): unknown {
    const c = this.config;
    switch (cmd) {
      case "get_snapshot":
        return this.snapshot();
      case "get_pet_placement":
        return null;
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
        this.approvals = this.approvals.filter((x) => !(x.source === "ask" && samePath(x.project, String(a.project))));
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
        // Stop denies the run's pending requests first.
        this.approvals = this.approvals.filter((x) => !(x.source === "ask" && samePath(x.project, path)));
        const t = this.threads.find((x) => x.source === "ask" && samePath(x.project, path));
        if (t) (t.status = "idle"), (t.updatedAt = Date.now());
        this.petEvent({ sessionId: t?.sessionId ?? "", project: path, kind: "ended", label: "Stopped" });
        this.publish();
        return null;
      }
      case "trust_project":
      case "untrust_project": {
        const p = this.projects.find((x) => samePath(x.path, String(a.project)));
        if (!p) throw "Unknown project.";
        p.trusted = cmd === "trust_project";
        return this.publish();
      }
      case "mark_credits_notice_seen":
        c.creditsNoticeSeen = true;
        return this.publish();
      case "finish_onboarding":
        c.onboarded = true;
        c.approvalsIntroSeen = true;
        return this.publish();
      case "answer_approval":
        return this.answerApproval(String(a.id), String(a.decision));
      case "set_watch_approvals":
        c.watchApprovals = !!a.enabled;
        if (c.watchApprovals) c.approvalsIntroSeen = true;
        // Turning it off answers every held request "no decision"; the threads keep "needs input".
        if (!c.watchApprovals) this.approvals = this.approvals.filter((x) => x.source !== "watch");
        return this.publish();
      case "set_approval_hold":
        if (!HOLDS.includes(Number(a.secs))) throw "Choose 30 seconds, 1 minute, 2 minutes or 4 minutes.";
        c.approvalHoldSecs = Number(a.secs);
        return this.publish();
      case "mark_approvals_intro_seen":
        c.approvalsIntroSeen = true;
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
      case "set_pet": {
        const next = PETS.find((p) => p.id === a.id);
        if (!next) throw "That pet isn't installed.";
        // As in Rust: a name the user chose stays, a default name follows the pet.
        const old = PETS.find((p) => p.id === c.petId);
        if (c.petName === "Perch" || c.petName === old?.displayName) c.petName = next.displayName;
        c.petId = next.id;
        return this.publish();
      }
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
        window.open(`/?window=settings${a.view === "onboarding" ? "&onboarding=1" : ""}`, "devlings-settings", "width=480,height=680");
        return null;
      case "open_pets_folder":
        console.info("[preview] open_pets_folder");
        return null;
      case "check_for_update":
        return this.checkForUpdate();
      case "install_update":
        if (this.update.state !== "ready") throw "No update is ready to install yet.";
        if (this.running.length) throw "Finish or stop the running ask first.";
        console.info(`[preview] install_update: installing ${this.update.version} and restarting`);
        this.setUpdate({ state: "idle" });
        return null;
      case "set_auto_update":
        c.autoUpdate = !!a.enabled;
        return this.publish();
      case "get_diagnostics":
        return DIAGNOSTICS;
      case "open_log_folder":
        console.info("[preview] open_log_folder");
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
          backend.emit("pet-open", view === "thread" ? { view: "thread", sessionId: "ask-devlings" } : { view: "compose" });
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
