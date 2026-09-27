import type { Config, SetupStatus, Snapshot, ThreadInfo } from "../shared/types";

export function makeConfig(over: Partial<Config> = {}): Config {
  return {
    petName: "Mochi",
    onboarded: true,
    creditsNoticeSeen: true,
    hooksDeclined: false,
    hookPort: 1,
    hookToken: "t",
    claudePath: null,
    petPosition: null,
    notifications: true,
    launchAtLogin: false,
    petId: "perch",
    petScale: 0.6,
    threadsCollapsed: false,
    ...over,
  };
}

export function makeSetup(over: Partial<SetupStatus> = {}): SetupStatus {
  return {
    claudePath: "C:\\claude.exe",
    claudeVersion: "2.1.282",
    claudeError: null,
    hooksInstalled: true,
    auth: { status: "allowed", subscription: "pro" },
    hookServerError: null,
    setupHint: null,
    needsSetup: false,
    ...over,
  };
}

export function makeSnapshot(over: Partial<Snapshot> = {}): Snapshot {
  return {
    config: makeConfig(),
    petState: "idle",
    threads: [],
    projects: [
      {
        path: "C:\\code\\app",
        name: "app",
        lastSeen: 2,
        permissionMode: "edit_files",
        askSessionId: null,
        transcriptPath: null,
      },
      {
        path: "C:\\code\\api",
        name: "api",
        lastSeen: 1,
        permissionMode: "read_only",
        askSessionId: null,
        transcriptPath: null,
      },
    ],
    running: [],
    setup: makeSetup(),
    ...over,
  };
}

let seq = 0;
export function makeThread(over: Partial<ThreadInfo> = {}): ThreadInfo {
  seq += 1;
  return {
    sessionId: `s${seq}`,
    project: `C:\\code\\p${seq}`,
    projectName: `p${seq}`,
    source: "watch",
    status: "running",
    label: "Working",
    excerpt: null,
    updatedAt: 1_000,
    unread: false,
    ...over,
  };
}
