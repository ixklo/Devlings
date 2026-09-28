// Browser preview scenes (`?scene=`): fixed states the README and website images are captured from
// (scripts/pets/capture-ui.mjs). Every project, path and reply here is made up.
//
//   cards     a finished Ask and two running sessions (the pet inspects the result)
//   approval  one permission request, for the Deny / Allow card
//   chat      a running Ask that streams its answer; add &open=thread for the mini chat
//   demo      idle, then two sessions start, one finishes, and the cards are tucked away behind a count

import type { ChatTurn, PendingApproval, ProjectEntry, ThreadInfo } from "./types";

export const SCENES = ["cards", "approval", "chat", "demo"] as const;
export type SceneName = (typeof SCENES)[number];

export interface SceneAsk {
  project: string;
  sessionId: string;
  /** The step shown while it reads, before the reply streams. */
  step: string;
  reply: string;
  /** The card's line once it's done. */
  excerpt: string;
}

export interface SceneStep {
  /** Milliseconds after the preview loads. */
  at: number;
  /** Replaces every thread. */
  threads?: ThreadInfo[];
  /** Collapses (or brings back) the cards, as the Show threads button would. */
  collapsed?: boolean;
}

export interface Scene {
  projects: ProjectEntry[];
  threads: ThreadInfo[];
  approvals: PendingApproval[];
  /** An Ask already running when the preview opens. */
  ask: SceneAsk | null;
  history: Record<string, ChatTurn[]>;
  timeline: SceneStep[];
}

const MIN = 60_000;
const ROOT = "C:\\Users\\you\\code\\";
const WEATHER = `${ROOT}weather-app`;

const FORECAST_DONE = "Added a 5-day forecast under today's weather. All 24 tests pass.";

const CHAT_REPLY = `The forecast groups hours by their **UTC** date, so just after midnight the first day is still yesterday's.

Group them by the local date instead:

\`\`\`ts
const days = groupBy(hours, localDay);
\`\`\`

I changed \`src/forecast.ts\` and added a test for 00:30 local time. All 25 tests pass.`;

function project(name: string, minsAgo: number, now: number, ask = false): ProjectEntry {
  return {
    path: ROOT + name,
    name,
    lastSeen: now - minsAgo * MIN,
    permissionMode: "edit_files",
    askSessionId: ask ? `ask-${name}` : null,
    transcriptPath: null,
    trusted: true,
  };
}

function thread(
  name: string,
  source: ThreadInfo["source"],
  status: ThreadInfo["status"],
  updatedAt: number,
  label: string | null,
  excerpt: string | null = null,
): ThreadInfo {
  return {
    sessionId: `${source}-${name}`,
    project: ROOT + name,
    projectName: name,
    source,
    status,
    label,
    excerpt,
    updatedAt,
    unread: status === "ready" || status === "blocked",
  };
}

function projects(now: number): ProjectEntry[] {
  return [project("weather-app", 0, now, true), project("blog", 3, now), project("recipe-box", 20, now), project("portfolio", 90, now)];
}

/** The scene called `name`, or null for the preview's usual made-up workspace. */
export function sceneFor(name: string | null, now: number, holdSecs: number): Scene | null {
  const base: Scene = { projects: projects(now), threads: [], approvals: [], ask: null, history: {}, timeline: [] };
  switch (name) {
    case "cards":
      return {
        ...base,
        threads: [
          thread("weather-app", "ask", "ready", now - MIN, "Done", FORECAST_DONE),
          thread("blog", "watch", "running", now, "Editing posts/spring-garden.md"),
          thread("portfolio", "watch", "running", now - 2 * MIN, "Running npm run build"),
        ],
      };
    case "approval":
      return {
        ...base,
        threads: [
          thread("recipe-box", "watch", "needs_input", now, "Wants to run: npm install date-fns"),
          thread("blog", "watch", "running", now - MIN, "Editing posts/spring-garden.md"),
        ],
        approvals: [
          {
            id: "scene-approval-1",
            sessionId: "watch-recipe-box",
            project: `${ROOT}recipe-box`,
            projectName: "recipe-box",
            source: "watch",
            toolName: "Bash",
            rawToolName: null,
            summary: "npm install date-fns",
            description: "Install date-fns for the meal planner's week view",
            details: JSON.stringify({ command: "npm install date-fns", description: "Install date-fns for the meal planner's week view" }, null, 2),
            lossy: false,
            tooLong: false,
            risks: [],
            canAlwaysAllow: true,
            alwaysLabel: "Always allow",
            alwaysDetail: "Adds the rule Bash(npm install:*) to this project's local settings",
            expiresAt: now + holdSecs * 1000,
          },
        ],
      };
    case "chat":
      return {
        ...base,
        threads: [thread("weather-app", "ask", "running", now, "Thinking…")],
        ask: {
          project: WEATHER,
          sessionId: "ask-weather-app",
          step: "Reading src/forecast.ts",
          reply: CHAT_REPLY,
          excerpt: "Grouped the forecast by local date. All 25 tests pass.",
        },
        history: {
          [WEATHER]: [{ role: "user", text: "The forecast shows the wrong day just after midnight. Can you find out why?" }],
        },
      };
    case "demo": {
      // Times are relative to load; each step stamps its threads when it runs (see the mock's timeline player).
      const blog = (at: number) => thread("blog", "watch", "running", now + at, "Editing posts/spring-garden.md");
      return {
        ...base,
        timeline: [
          { at: 1200, threads: [thread("weather-app", "watch", "running", now + 1200, "Running npm test")] },
          { at: 2600, threads: [thread("weather-app", "watch", "running", now + 1200, "Running npm test"), blog(2600)] },
          { at: 5200, threads: [thread("weather-app", "watch", "ready", now + 5200, "Done", FORECAST_DONE), blog(2600)] },
          { at: 8200, collapsed: true },
        ],
      };
    }
    default:
      return null;
  }
}
