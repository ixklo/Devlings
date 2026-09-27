// Shapes shared with the Rust backend. Section 5 of
// docs/specs/2026-09-26-perch-v0.2-design.md is the contract; keep these
// field names exactly as the backend serializes them (camelCase).

/**
 * `untrusted` (v1.0, Ask only): the run skips an untrusted folder's project settings. `label` is the notice's
 * headline and `text` its "Skipped: …" line. It never changes a thread.
 */
export type Kind =
  | "started"
  | "prompt"
  | "step"
  | "blocked"
  | "needs_you"
  | "reply_delta"
  | "done"
  | "failed"
  | "ended"
  | "untrusted";
export type Source = "watch" | "ask";
export type PermissionMode = "read_only" | "edit_files" | "auto";

export interface PetEvent {
  sessionId: string;
  project: string;
  source: Source;
  kind: Kind;
  label?: string;
  text?: string;
  at: number;
}

export interface Config {
  petName: string;
  onboarded: boolean;
  creditsNoticeSeen: boolean;
  hooksDeclined: boolean;
  hookPort: number | null;
  hookToken: string | null;
  claudePath: string | null;
  petPosition: [number, number] | null;
  notifications: boolean;
  launchAtLogin: boolean;
  petId: string;
  petScale: number;
  threadsCollapsed: boolean;
  /** Check for updates at launch and daily. */
  autoUpdate: boolean;
  /** Epoch ms of the last update check. */
  lastUpdateCheck: number | null;
}

export type PetState = "idle" | "running" | "needs_input" | "ready" | "blocked" | "setup";
export type ThreadStatus = "running" | "needs_input" | "ready" | "blocked" | "idle";

export interface ThreadInfo {
  sessionId: string;
  project: string;
  projectName: string;
  source: Source;
  status: ThreadStatus;
  /** Latest step, e.g. "Editing app.tsx". */
  label: string | null;
  /** Latest final reply excerpt (≤ 200 chars) for ready/blocked. */
  excerpt: string | null;
  /** Epoch ms. */
  updatedAt: number;
  /** Ready/blocked and not yet viewed. */
  unread: boolean;
}

export interface ProjectEntry {
  path: string;
  name: string;
  lastSeen: number;
  permissionMode: PermissionMode;
  askSessionId: string | null;
  transcriptPath: string | null;
  /** Trusted in Perch: Asks here use the folder's own Claude Code settings (v1.0 D6). */
  trusted: boolean;
}

export type AuthVerdict = { status: "allowed"; subscription: string } | { status: "refused"; reason: string };

export interface SetupStatus {
  claudePath: string | null;
  claudeVersion: string | null;
  claudeError: string | null;
  hooksInstalled: boolean;
  auth: AuthVerdict | null;
  hookServerError: string | null;
  /** Advice that doesn't block setup, e.g. "Move Perch to Applications…" when it runs from a disk image. */
  setupHint: string | null;
  needsSetup: boolean;
}

export interface Snapshot {
  config: Config;
  petState: PetState;
  /** Visible threads, already sorted by the backend. */
  threads: ThreadInfo[];
  projects: ProjectEntry[];
  /** Projects with an active Ask run. */
  running: string[];
  setup: SetupStatus;
  update: UpdateStatus;
}

export type UpdateState = "idle" | "checking" | "available" | "downloading" | "ready" | "error" | "disabled";

/** In-app update progress (v1.0 spec section 4). Unset fields are absent. */
export interface UpdateStatus {
  state: UpdateState;
  version?: string;
  notes?: string;
  /** Whole percent, 0-100, while downloading; absent when the size is unknown. */
  progress?: number;
  error?: string;
}

export interface PetInfo {
  id: string;
  displayName: string;
  description: string;
  source: "bundled" | "perch" | "codex";
}

/** Window-relative CSS px. */
export interface HitRect {
  x: number;
  y: number;
  w: number;
  h: number;
  /** Set for elements whose hover matters ("pet", "bar"); the backend reports it back via `pet-pointer`. */
  id?: string;
}

export type PetOpen = { view: "compose" } | { view: "thread"; sessionId: string };
export type SettingsView = "settings" | "onboarding";

export interface ChatTurn {
  /** `untrusted`: the untrusted-folder notice (`text` is its headline, `detail` what was skipped). */
  role: "user" | "assistant" | "note" | "untrusted";
  text: string;
  detail?: string;
  pending?: boolean;
}
