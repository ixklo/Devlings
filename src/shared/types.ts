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
  /** Whether `petPosition` is already the sprite's anchor (design D9), vs. an unmigrated v0.2 value. */
  petPositionMigrated: boolean;
  notifications: boolean;
  launchAtLogin: boolean;
  petId: string;
  petScale: number;
  threadsCollapsed: boolean;
  /** Check for updates at launch and daily. */
  autoUpdate: boolean;
  /** Epoch ms of the last update check. */
  lastUpdateCheck: number | null;
  /** Answer permission requests of watched sessions from the pet. Off by default. */
  watchApprovals: boolean;
  /** How long a watched request is held for an answer in the pet: 30, 60, 120 or 240. */
  approvalHoldSecs: number;
  /** The one-time "answer permission prompts" intro card was answered. */
  approvalsIntroSeen: boolean;
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
  /** Trusted in Devlings: Asks here use the folder's own Claude Code settings (v1.0 D6). */
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
  /** Advice that doesn't block setup, e.g. "Move Devlings to Applications…" when it runs from a disk image. */
  setupHint: string | null;
  needsSetup: boolean;
  /** The system won't show Devlings' notifications (Windows: turned off in its Settings), whatever the switch says. */
  systemNotificationsOff: boolean;
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
  /** Permission requests the user can answer from the pet, oldest first. */
  approvals: PendingApproval[];
  /** The latest plan usage an Ask run reported, or null before the first one since launch (v1.0 S4). */
  usage: UsageInfo | null;
}

/**
 * Plan usage from the last `rate_limit_event` of an Ask run (v1.0 S4). Unset fields are absent. It's a snapshot of
 * that moment, never live: always show it with its `seenAt` time.
 */
export interface UsageInfo {
  /** `allowed`, `allowed_warning` or `rejected`; anything else is shown generically. */
  status: string;
  /** Epoch ms. */
  resetsAt?: number;
  /** Share of the limit used, 0-1. */
  utilization?: number;
  /** Which limit: `five_hour`, `seven_day`, `seven_day_opus`, ... */
  kind?: string;
  /** Epoch ms when Devlings saw it. */
  seenAt: number;
}

export type ApprovalDecision = "allow" | "deny" | "always";

/** A Claude Code permission request waiting for an answer (v1.0 spec section 4). */
export interface PendingApproval {
  id: string;
  sessionId: string;
  project: string;
  projectName: string;
  /** "watch": another Claude Code session; "ask": one of the pet's own Ask runs. */
  source: Source;
  toolName: string;
  /** Claude Code's own name for an MCP tool shown under a display name, e.g. "mcp__github__create_issue". */
  rawToolName: string | null;
  /** The exact command, file path or URL; otherwise the input as compact JSON. */
  summary: string;
  description: string | null;
  /** The whole tool input as pretty JSON (at most 16 KB). */
  details: string;
  /** The headline leaves something out, so the card opens Details by default. */
  lossy: boolean;
  /** Part of the request can't be shown, so it can only be denied here. */
  tooLong: boolean;
  /** Short warnings for risky flags, e.g. "Runs outside the sandbox". */
  risks: string[];
  canAlwaysAllow: boolean;
  /** "Allow for this session" or "Always allow"; null without canAlwaysAllow. */
  alwaysLabel: string | null;
  /** One line saying what the always button does. */
  alwaysDetail: string | null;
  /** Epoch ms when a watched request's hold ends; null for Ask requests. */
  expiresAt: number | null;
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

/**
 * How `.stage` should lay out for the sprite's current on-screen spot (design D9). Sent as the
 * `pet-placement` event whenever the backend (re)positions the window.
 */
export interface Placement {
  /** Cards open below the sprite (flipped) instead of above it. */
  cardsBelow: boolean;
  /** Logical px to shift `.stage` sideways (a CSS transform) so cards stay on screen near an edge. */
  shiftX: number;
  /** Logical px `.stage` actually has on screen; applied as its `max-height` so content shrinks or
   *  scrolls instead of being cut off when the window's top or bottom edge is off-screen. */
  stageRoom: number;
}

/**
 * `thread` opens a thread the way a click on its card does. A notification click (Windows, v1.0 S1) also carries the
 * thread's `project` and `source`, so it still works after the thread has left the cards.
 */
export type ThreadOpen = { view: "thread"; sessionId: string; project?: string; source?: Source };
export type PetOpen = { view: "compose" } | ThreadOpen;
export type SettingsView = "settings" | "onboarding";

export interface ChatTurn {
  /** `untrusted`: the untrusted-folder notice (`text` is its headline, `detail` what was skipped). */
  role: "user" | "assistant" | "note" | "untrusted";
  text: string;
  detail?: string;
  pending?: boolean;
}
