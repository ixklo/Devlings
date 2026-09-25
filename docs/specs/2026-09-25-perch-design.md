# Perch — design spec

Date: 2026-09-25
Status: draft for review
Working name: **Perch** (renameable; must not use "Claude" or Anthropic's mascot)

## 1. What it is

A small animated pet that sits on the desktop, always on top. It shows what Claude Code is doing in your projects, and you can give it work without opening VS Code or a terminal.

- **Watch:** the pet reports on the Claude Code sessions you already run (VS Code extension, terminal, desktop app). It shows when Claude is working, when it needs you, and when it's done.
- **Ask:** click the pet, pick a project folder, type a prompt. The pet runs the user's own installed Claude Code in that folder and streams progress back.

Free, open source (MIT), distributed through GitHub Releases. Not affiliated with Anthropic.

## 2. Goals and non-goals

**Goals**
- Always-visible pet with distinct animations for each state.
- Live status for every running Claude Code session, grouped by project.
- Delegate a prompt to a chosen project and follow up in the same conversation.
- OS notification when a session finishes or needs approval.
- **Costs no money.** Ask prompts use tokens from the user's Claude subscription like any other prompt, and Perch never causes a charge. It runs only on a claude.ai subscription login and refuses API-key or cloud-provider auth (§4.4).
- No login of its own. It uses the claude.ai login Claude Code already has.
- The user names their pet.
- One-click install and one-click removal of the hooks it adds.

**Non-goals (v1)**
- No claude.ai wrapping, no own Anthropic auth, no direct API calls.
- No file browser, diff viewer, or code editing UI.
- No answering permission prompts from the pet (Ask runs never prompt; see §4.2).
- No multiple pets, no changing the pet's appearance (name only), no cloud sync.

## 3. Platform and stack

- **Tauri v2.** Rust core plus a web frontend, with small installers (roughly 10 MB against about 100 MB for Electron). Supports transparent, frameless, always-on-top windows.
- **Frontend:** TypeScript, React, Vite.
- **Targets:** Windows first (the dev machine). macOS and Linux get CI builds, best-effort for v1.
- **Minimum Claude Code:** v2.1.259 (needed for `--permission-prompts`). Checked at startup.

## 4. How it talks to Claude Code

Both sources are converted into one internal event type (§5.2), so the rest of the app doesn't care where an event came from.

### 4.1 Watch — HTTP hooks

On first run, with the user's consent, Perch adds `http` hooks to `~/.claude/settings.json` for these events:

`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `Notification`, `Stop`, `StopFailure`, `SessionEnd`

Each hook POSTs the hook JSON to `http://127.0.0.1:<port>/hook/<token>` with `timeout: 2`.

- `port` is fixed at install time and saved in Perch config. On a port conflict at startup, Perch shows an error and offers "reinstall hooks on a new port".
- `token` is a random 32-byte hex string in the URL path. It keeps other local processes from faking updates, without needing environment variables.
- The server binds to 127.0.0.1 only.
- **Install:** read settings.json, save a backup at `settings.json.perch-backup-<timestamp>`, merge Perch's entries in, and leave every existing hook untouched. Each Perch entry is identified by its URL prefix `http://127.0.0.1:<port>/hook/`.
- **Uninstall:** remove exactly the entries with that URL prefix and nothing else.
- When Perch isn't running, the POST fails fast (2 s timeout). Claude Code treats a failed HTTP hook as non-blocking, so sessions are unaffected. Confirm this with a manual test (§9).
- Hooks fire in terminal, IDE extension, and desktop app sessions alike.

Fields used: `session_id`, `cwd` (the project), `hook_event_name`, `tool_name` + `tool_input` (PreToolUse), `notification_type` (Notification), `last_assistant_message` (Stop).

### 4.2 Ask — headless Claude Code

Perch runs:

```
claude -p --output-format stream-json --verbose --include-partial-messages
       --permission-mode <mode> --permission-prompts none
       [--resume <session_id>]
```

- The prompt goes through stdin, which is then closed. This avoids quoting problems and prompts that start with `-`; verified on 2.1.282. `--resume` keeps the same `session_id`, also verified.
- The working directory is the selected project folder.
- `<mode>` is set per project: **Read only** (`dontAsk`), **Edit files** (`acceptEdits`, the default), or **Auto** (`auto`).
- `--permission-prompts none` means runs never hang waiting for approval. Denials come through as `permission_denied` events and appear in the feed as "Blocked: <tool> <summary>".
- The first `system/init` event gives the `session_id`. Perch stores it per project, and follow-ups pass `--resume <id>`. A "New conversation" button clears it.
- Parsed events: `system/init`, `system/api_retry`, `stream_event` text deltas, `assistant` (tool_use blocks give step labels), `user` (tool_result), `result` (final text, `is_error`, `permission_denials`).
- **Stop button:** on macOS/Linux, send SIGINT so the turn ends cleanly. On Windows, kill the process tree (`taskkill /T /F /PID`). The turn is left unfinished, and the next `--resume` continues it, which Claude Code documents.
- **Duplicate suppression:** Ask runs also fire the user's hooks. Perch ignores hook events whose `session_id` belongs to an Ask run it is currently streaming.
- Every Ask run goes through the money guard (§4.4) first.

### 4.4 Money guard

Rule: an Ask run can use subscription tokens but can never cause a charge.

1. **Clean environment.** The child process gets the user's environment minus every variable that switches Claude Code to paid auth or another provider: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX`, `CLAUDE_CODE_USE_FOUNDRY`. The list is a constant in `runner` with a unit test.
2. **Auth check.** Before the first Ask run of each app session, and after any auth failure, Perch runs `claude auth status` (JSON) with the same clean environment. Ask is allowed only if the status shows a claude.ai subscription login (Pro/Max/Team/Enterprise). API key, Console, and cloud-provider logins are refused. The exact JSON field names come from real output captured as a test fixture during implementation. If the output can't be parsed, Ask stays disabled; it fails closed.
3. **When refused:** the pet shows `setup`, and the panel explains: "Ask only works with a Claude subscription login. Run `claude auth login` and choose your Claude account." Watch mode keeps working, since it only listens and sends no prompts.
4. **Run-time checks.** Two checks run while a run streams, based on output captured from Claude Code 2.1.282:
   - The `system/init` event must have `apiKeySource: "none"`. Anything else kills the run immediately.
   - Every `rate_limit_event` carries `rate_limit_info.isUsingOverage`. If it is `true`, the request is drawing on paid usage credits, so Perch kills the run at once and shows "Stopped: your plan limit is used up and this would bill usage credits. Resets at <resetsAt>." If `rate_limit_info.status` is `"rejected"`, Perch shows "Plan limit reached — resets at <resetsAt>" and does not retry.
5. **Usage credits notice.** Perch can't read the account's usage-credits setting ahead of time, and the overage check above reacts on the first overage event. So before the first Ask run, a one-time notice says: "If you turned on usage credits in your Claude account, Perch stops any run the moment it would use them." Settings links to claude.ai/settings/usage.
6. **No API-billing features.** Perch never passes `--console`, an API key, `apiKeyHelper`, or fast mode.

### 4.3 Finding the `claude` binary

In order:
1. The path saved in Settings.
2. `claude` on PATH.
3. The newest Claude Code binary inside VS Code extensions (`~/.vscode/extensions/anthropic.claude-code-*/`). Search the folder for the executable instead of hardcoding a subpath.

Run `claude --version` and parse the version. Below 2.1.259, or not found: the pet shows a "needs setup" state and Settings explains what to do.

## 5. Architecture

### 5.1 Units

**Rust core (`src-tauri/src/`)**

| Module | Job | Depends on |
|---|---|---|
| `locator` | Find the claude binary, read its version | filesystem, process |
| `hooks_installer` | Merge and remove Perch hooks in a settings.json value; backup | serde_json only (pure fn plus a thin IO wrapper) |
| `hook_server` | Localhost HTTP server; checks the token; hook JSON → `PetEvent` | `normalize` |
| `runner` | Spawn and stop Ask runs; read NDJSON line by line → `PetEvent` | `normalize`, `locator` |
| `normalize` | Pure functions: hook payload → `PetEvent`, stream-json line → `PetEvent` | none |
| `sessions` | In-memory session table plus the pet mood reducer | `PetEvent` |
| `store` | Config and history JSON in the app data dir | filesystem |

Every `PetEvent` goes to `sessions`, which emits `sessions-changed` and `pet-mood-changed` Tauri events to the frontend.

**Frontend (`src/`)**
- `pet/` — pet window: sprite plus speech bubble; listens for `pet-mood-changed`.
- `panel/` — panel window: project picker, session feed, prompt box, streamed reply.
- `settings/` — hooks install/remove, binary path, per-project permission mode, launch at login.
- `shared/types.ts` — TS mirrors of the Rust event types.

### 5.2 Event model

```ts
type PetEvent = {
  sessionId: string;
  project: string;          // absolute path (cwd)
  source: "watch" | "ask";
  kind: "started" | "prompt" | "step" | "blocked" | "needs_you" | "reply_delta" | "done" | "failed" | "ended";
  label?: string;           // short human text, e.g. "Editing app.tsx", "Running npm test"
  text?: string;            // reply delta or final message excerpt
  at: number;               // epoch ms
};
```

Mapping:

| Source signal | kind |
|---|---|
| SessionStart / system init | started |
| UserPromptSubmit | prompt |
| PreToolUse / assistant tool_use | step (label from tool name + main arg) |
| Notification `permission_prompt` | needs_you (`idle_prompt` is ignored: it fires on every finished turn and would pin the pet to needs_you) |
| system/permission_denied | blocked (label "Blocked: <tool_name>") |
| system/api_retry | step (label "Retrying (attempt/max_retries)…") |
| stream_event text_delta | reply_delta |
| Stop / result success | done |
| StopFailure / result is_error / exit non-zero | failed |
| SessionEnd / process exit | ended |

Step labels: Read → "Reading <file>", Edit/Write → "Editing <file>", Bash → "Running <first 40 chars>", Grep/Glob → "Searching", Agent → "Delegating to a subagent", anything else → the tool name.

### 5.3 Pet mood (derived, not stored)

Priority, highest first, across all live sessions:
1. **needs_you** — any session waiting on the user.
2. **failed** — a failure in the last 30 s.
3. **working** — any session whose last event is prompt, step, blocked, or reply_delta, and less than 30 min old (so a crashed session can't pin the pet to working).
4. **done** — a completion in the last 8 s.
5. **listening** — panel is open.
6. **sleeping** — no events for 10 min.
7. **idle** — otherwise.
8. **setup** — overrides everything when the binary is missing or too old, or the hooks aren't installed.

## 6. UI

**First run:** a short onboarding with three steps: (1) name your pet (1–24 characters, default "Perch"), (2) consent to installing hooks, (3) Claude Code check (binary, version, auth). The name can be changed later in Settings.

**Pet name:** used in the speech bubble title, panel header ("<name> is working on <project>"), notification titles, tray tooltip, and the Ask conversation's assistant label.

**Pet window:** about 140×140, transparent, frameless, always on top, not in the taskbar, draggable. Position is saved. A speech bubble shows the latest label for about 4 s. Clicking toggles the panel; right-clicking opens a menu (Settings, Hide for 1 hour, Quit).

**Tray icon:** Show/Hide pet, Settings, Quit.

**Panel window** (about 380×540, opens next to the pet and stays on screen):
- **Header:** project picker. Lists recent projects (from hook `cwd`s and Ask history) plus "Choose folder…".
- **Activity:** live sessions for all projects, each with its project name, source badge (VS Code/terminal vs Ask), and current label. Finished sessions stay listed for 1 hour.
- **Conversation** (selected project, Ask): message list with streamed Markdown, code blocks with syntax highlighting, and blocked-action lines.
- **Composer:** text box (Enter sends, Shift+Enter adds a newline), Stop button while running, permission mode dropdown, and "New conversation".

**Notifications:** OS notification on `done` and `needs_you` while the panel is closed, titled with the pet name and naming the project. Settings has a toggle. Clicking a notification does nothing in v1, because the Tauri notification plugin doesn't deliver desktop click events. Clicking the pet opens the panel.

**Global shortcut:** Ctrl+Alt+P (Cmd+Option+P on macOS) toggles the panel.

**Pet art:** one original character drawn as layered SVG and animated with CSS keyframes. Each mood gets its own animation (blink/breathe, ears up, typing/bobbing, waving and bouncing, slump plus "!" mark, snoozing Zs). No Anthropic marks or likeness.

## 7. Persistence

`<app data>/perch/config.json`: pet name, usage-credits notice seen, hook port, token, binary path override, pet position, notification toggle, launch-at-login.

`<app data>/perch/projects.json`: recent projects with last-seen time, permission mode, and the Ask `session_id` for each.

Ask conversation text isn't stored by Perch. On reopen, the panel shows "Continue conversation" (resumes by id) and loads prior turns from the session's transcript `.jsonl`. Ask runs fire the user's hooks too, so Perch saves `transcript_path` from the first hook event carrying that run's `session_id`. If the transcript can't be read, the panel starts empty but resume still works.

## 8. Error handling

| Situation | Behavior |
|---|---|
| claude not found / too old | Mood `setup`; Settings shows how to install or update, plus a path picker |
| Hooks not installed | Mood `setup` until installed or dismissed ("Ask only" mode) |
| settings.json invalid JSON | Refuse to edit it and show the parse error with the file path |
| Port in use | Error with a "move to new port" action that reinstalls the hooks |
| Bad or missing token on POST | 404, ignored |
| Ask run: non-zero exit / auth error in result | `failed` event; conversation shows the error text from the result |
| Auth is API key / Console / cloud provider | Ask disabled with login instructions; Watch unaffected (§4.4) |
| `claude auth status` fails or is unparseable | Ask disabled (fail closed), error shown in Settings |
| Plan limit reached during Ask | Run ends with "Plan limit reached — resets at <time>"; no retry |
| api_retry events | Label "Retrying (n/max)…" |
| Stream line not JSON | Skip it and log it (debug log in app data) |
| Project folder deleted | Remove it from recents and show a notice |

## 9. Testing

**Rust unit tests (most coverage):**
- `normalize`: fixtures of real hook payloads and captured stream-json runs (success, tool use, permission denial, error, api_retry) → expected `PetEvent` lists.
- `hooks_installer`: empty file, file with other hooks, file with Perch hooks already present, reinstall on a new port, uninstall leaves foreign hooks byte-for-byte equal.
- `sessions`: the mood reducer against event sequences, including timing windows (inject the clock).
- `locator`: version parsing and extension-dir search on a temp directory tree.
- Money guard: the env scrub removes every listed variable and keeps the rest; auth-status fixtures (subscription → allowed; API key, Console, Bedrock, garbage output → refused); plan-limit error → no retry.
- Pet name validation: empty, too long, and whitespace-only names are rejected.

**Frontend (Vitest):** mood → animation class mapping; the composer's Enter/Shift+Enter behavior.

**Manual release checklist:**
1. Fresh install → consent → hooks appear in settings.json, backup exists.
2. Prompt in the VS Code extension → pet goes working → done, and a notification shows.
3. Trigger a permission prompt in VS Code → pet shows needs_you.
4. Quit Perch, use Claude Code normally → no delay or errors from the hooks.
5. Ask flow: pick project, send, stream, follow-up resumes, Stop works, a blocked action is shown.
6. Rename the claude binary → setup state with guidance.
6a. Set `ANTHROPIC_API_KEY` in the environment, launch Perch, send an Ask → the run still uses the subscription (the key is scrubbed). Log Claude Code in with `claude auth login --console` → Ask is refused.
6b. Rename the pet → the new name shows in the bubble, panel, and next notification.
7. Uninstall hooks → only Perch entries removed.

## 10. Release

- Public GitHub repo, MIT license. README covers what it does, requirements (Claude Code ≥ 2.1.259, logged in), install, how hooks are used and removed, privacy (Perch sends nothing anywhere itself; its only network traffic is localhost), cost ("uses your subscription, never an API key; only usage credits you turned on yourself can bill you"), and "Not affiliated with Anthropic."
- GitHub Actions with `tauri-action` builds Windows (NSIS), macOS (dmg), and Linux (AppImage) on each version tag and attaches them to a GitHub Release.
- Installers are unsigned in v1. The README explains the Windows SmartScreen and macOS Gatekeeper warnings.
- Before first public release: check Anthropic's current Claude Code terms and usage docs for anything affecting tools that launch the user's local `claude` binary, and update the README usage note.
