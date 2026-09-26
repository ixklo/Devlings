// Shapes shared with the Rust backend. Section 5 of
// docs/specs/2026-09-26-perch-v0.2-design.md is the contract; keep these
// field names exactly as the backend serializes them (camelCase).

export type Kind = "started" | "prompt" | "step" | "blocked" | "needs_you" | "reply_delta" | "done" | "failed" | "ended";
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
}

export type AuthVerdict = { status: "allowed"; subscription: string } | { status: "refused"; reason: string };

export interface SetupStatus {
  claudePath: string | null;
  claudeVersion: string | null;
  claudeError: string | null;
  hooksInstalled: boolean;
  auth: AuthVerdict | null;
  hookServerError: string | null;
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
  role: "user" | "assistant" | "note";
  text: string;
  pending?: boolean;
}
