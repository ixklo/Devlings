export type Mood = "setup" | "needs_you" | "failed" | "working" | "done" | "listening" | "sleeping" | "idle";
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
}

export interface SessionInfo {
  sessionId: string;
  project: string;
  source: Source;
  state: Kind;
  label: string | null;
  lastAt: number;
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
  mood: Mood;
  sessions: SessionInfo[];
  projects: ProjectEntry[];
  running: string[];
  setup: SetupStatus;
}

export interface ChatTurn {
  role: "user" | "assistant" | "note";
  text: string;
  pending?: boolean;
}
