import type { Config, PendingApproval, SetupStatus, Snapshot, ThreadInfo } from "../shared/types";

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
    petPositionMigrated: true,
    notifications: true,
    launchAtLogin: false,
    petId: "perch",
    petScale: 0.6,
    threadsCollapsed: false,
    autoUpdate: true,
    lastUpdateCheck: null,
    watchApprovals: false,
    approvalHoldSecs: 60,
    approvalsIntroSeen: true,
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
        trusted: false,
      },
      {
        path: "C:\\code\\api",
        name: "api",
        lastSeen: 1,
        permissionMode: "read_only",
        askSessionId: null,
        transcriptPath: null,
        trusted: false,
      },
    ],
    running: [],
    setup: makeSetup(),
    update: { state: "idle" },
    usage: null,
    approvals: [],
    ...over,
  };
}

let approvalSeq = 0;
export function makeApproval(over: Partial<PendingApproval> = {}): PendingApproval {
  approvalSeq += 1;
  return {
    id: `a${approvalSeq}`,
    sessionId: `s-approval-${approvalSeq}`,
    project: "C:\\code\\app",
    projectName: "app",
    source: "watch",
    toolName: "Bash",
    rawToolName: null,
    summary: "npm test -- --watch=false",
    description: "Run the test suite",
    details: '{\n  "command": "npm test -- --watch=false",\n  "description": "Run the test suite"\n}',
    lossy: false,
    tooLong: false,
    risks: [],
    canAlwaysAllow: true,
    alwaysLabel: "Always allow",
    alwaysDetail: "Adds the rule Bash(npm test:*) to this project's local settings",
    expiresAt: null,
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
