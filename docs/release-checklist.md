# Release checklist

Written for v1.0, when the app was called Perch; v1.1 adds the upgrade from Perch below. Manual QA for the gates CI can't cover: `docs/release/v1.0.0.md` gates **G2** (install lifecycle) and **G3** (features), on Windows — the only platform v1.0 verifies (macOS/Linux ship beta on CI evidence alone; see `docs/release/v1.0.0.md` for G4). Record the result of each gate (pass, or what failed) back into `docs/release/v1.0.0.md`'s Evidence column.

Run on a Windows 11 machine with Claude Code logged in with a subscription, and VS Code with the Claude Code extension installed. Start from a clean state: quit Devlings, delete `%APPDATA%\io.github.perchpet.perch\`, and make sure `~/.claude/settings.json` has no Devlings hooks. Keep the previous release's installer (currently v0.2.0) on hand for the upgrade tests.

## v1.1 — Upgrade from Perch 1.0.0 (Windows)

The installer removes Perch before installing Devlings (design `docs/specs/2026-09-27-devlings-v1.1.md` D22.4). Start with Perch 1.0.0 installed from its own installer (default folder), onboarding done, hooks installed, a custom pet name, **Start at login** on, a desktop shortcut, and a Claude Code session open in VS Code. Note the port and token in the relay commands of `~/.claude/settings.json` (they end `\Perch\perch.exe" --hook-relay <port> <token>`).

- [ ] **Cancel keeps Perch.** With Perch running, run the Devlings installer and click Install. A dialog says "Perch, the earlier version of Devlings, is running. Click OK to close it and continue." Click **Cancel**: the install stops ("Perch is still running, so Devlings wasn't installed."), Perch is still running and still installed, and `%LOCALAPPDATA%\Devlings` has no `devlings.exe`.
- [ ] **Install over Perch.** Run it again, click **OK** in that dialog. The details list says "Found Perch, the earlier name of Devlings, in …\AppData\Local\Perch. Removing it; your settings and hooks stay." Finish with "Run Devlings" unticked.
- [ ] **Perch is gone, its data isn't.** `%LOCALAPPDATA%\Perch` no longer exists; `%LOCALAPPDATA%\Devlings` has `devlings.exe`, `uninstall.exe` and `pets`. Settings → Apps lists Devlings, not Perch. No `perch.exe` in Task Manager. `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Perch`, `reg query HKCU\Software\github\Perch` and `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v Perch` all report nothing found. The Start menu and desktop have Devlings and no Perch; a Perch taskbar pin is gone. `%APPDATA%\io.github.perchpet.perch\config.json` is unchanged, and `settings.json` still has all 11 hook entries, still pointing at `perch.exe` (update-mode uninstall kept them).
- [ ] **First launch.** Start Devlings from the Start menu. No onboarding; same pet, name and position. Tray tooltip, window titles and **Settings → About** say Devlings (version 1.1.0-rc.N). `settings.json`: the Stop, StopFailure and SessionEnd commands now run `…\AppData\Local\Devlings\devlings.exe` with the same port and token, everything else unchanged, and a new `settings.json.devlings-backup-<epoch>` holds the previous content. **Settings → Watching** says hooks are installed. `reg query …\Run /v Devlings` points at `devlings.exe`, **Start at login** is still on, and there's still no `Perch` value. `%LOCALAPPDATA%\io.github.perchpet.perch\logs\devlings.log` has "Devlings 1.1.0-rc.N starting", "Hooks updated to this version's entries" and "Start-at-login was on in Devlings' settings but missing; restored it"; the old `perch.log` is still there, untouched.
- [ ] **Only once.** Quit and start Devlings again: no new backup, and the log says "Hooks are up to date".
- [ ] **It all still works.** The VS Code session's card goes working → done (the relay reaches Devlings). With notifications on, a session finishing while its thread isn't open shows a toast, and clicking it opens the thread. **Copy diagnostics** starts "Devlings diagnostics" / "App: Devlings 1.1.0-rc.N". Quit Devlings, then use and end a Claude Code session: no hook error text.
- [ ] **Start at login.** Sign out and back in: Devlings starts; nothing tries to start Perch.
- [ ] **Uninstall.** Uninstall Devlings from Settings → Apps: no hook entries of ours are left in `settings.json` (a backup is saved), the `Run` value `Devlings` is gone, and so are `%LOCALAPPDATA%\Devlings` and its shortcuts.
- [ ] **In-app update from Perch** (only once the release's `latest.json` can be downloaded without signing in, so a public release): with Perch 1.0.0 installed, point `PERCH_UPDATE_ENDPOINT` at the RC's `latest.json` (or let the default endpoint redirect), wait for "Perch 1.1.0… is ready", and click **Restart**. The installer runs with no questions, Perch closes, and Devlings starts by itself. The results match the checks above, plus a Devlings Start menu shortcut exists (an update install doesn't normally create one) and a desktop shortcut exists only if Perch had one.

## G2.1 — Fresh install, no terminal

- [ ] Install from the built NSIS installer with no terminal open at any point. Onboarding has three steps: Watch (pick and name the pet, install hooks), Ask (subscription, never an API key, Agent SDK credit link, three green setup checks), Permission prompts (the switch, off by default); Finish closes it.
- [ ] `settings.json` now has Devlings' hook entries (§3 of `docs/specs/2026-09-26-perch-v1.0-design.md`: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PostToolUseFailure, Notification, PermissionDenied, PermissionRequest, Stop, StopFailure, SessionEnd) and a `settings.json.devlings-backup-*` exists.
- [ ] A live Claude Code session in VS Code shows up as a card within a few seconds.

## G2.2 — Upgrade from v0.2.0 keeps settings

- [ ] Install v0.2.0 first, set a custom pet name, a non-default port (trigger "Port in use" to force one), toggle notifications off, and drag the pet to a memorable custom spot (not the default bottom-right).
- [ ] Install the new build over it (don't uninstall first). Pet name, port and notification setting all survive, and the pet reappears at the **same visual spot** it was left at (M9/D9: a saved position now means the sprite's anchor, migrated once from v0.2's window-anchor meaning).
- [ ] Hooks reinstall once (v0.2's 7 HTTP entries differ in shape from v1's set) — confirm via a new `settings.json.devlings-backup-*` and the updated hook list, but confirm it does **not** reinstall again on a second launch with no change.

## G2.3 — In-app update, RC to RC; tampered signature refused

- [ ] Publish two RCs (e.g. `v1.0.0-rc.1` and `v1.0.0-rc.2`) and install the first.
- [ ] With `autoUpdate` on, Devlings finds rc.2 (launch check or the daily check via `DEVLINGS_UPDATE_ENDPOINT` pointed at the RC feed) and shows the "ready, restart to install" card. Restart installs rc.2; **Settings → About** shows the new version.
- [ ] Repeat with a `latest.json` whose signature has been altered by one byte: the update is refused (no install), and this is visible somewhere a user would notice (an error state, not a silent no-op).
- [ ] With an Ask run active, "Restart to install" does not appear (or is disabled) until the run ends.

## G2.4 — Uninstaller removes exactly Devlings' hooks and start-at-login

- [ ] With hooks installed and "start at login" on, run the uninstaller. `settings.json` no longer has any Devlings hook entries (diff against the pre-uninstall backup: only Devlings' lines are gone), and the start-at-login entry is gone.
- [ ] Installing an **update** (not an uninstall) over an existing install does not run this cleanup — hooks and start-at-login survive the upgrade.

## G2.5 — Quit with hooks installed, no delay or noise *(waivable)*

- [ ] Quit Devlings, then use Claude Code normally (a prompt, a tool call, ending the session). No noticeable delay and no hook-error text on stderr or in the transcript. If this fails, waive it in `docs/release/v1.0.0.md` with the specific noise observed.

## G3.1 — Every control in the README table works

Work through the README's "Using it" table on a live pet with at least one active session and one finished one: click the pet, the pencil, a card, hover a card, click a card's ×, the bell, the chevron, drag the pet (arrow keys, then Esc), right-click the pet (Settings, change pet, hide for an hour, quit), and Ctrl+Alt+P.

## G3.2 — Watch states; "needs input" within ~1 s

- [ ] Drive a session from VS Code through running → done; the card matches at each step.
- [ ] Drive a session from a terminal the same way.
- [ ] Trigger a permission prompt (Manual mode, ask for a shell command) and time from the prompt appearing to the pet's "?" badge: under ~1 s (D5 in the v1.0 design doc — PermissionRequest fires the state immediately).

## G3.3 — Ask in all modes; plan-limit fixture; money guard; untrusted folders

- [ ] Ask in each permission mode (default, acceptEdits, Manual/plan) and confirm the mode chip and behavior match — e.g. Manual mode surfaces a permission card instead of running silently, Read-only mode shows "Blocked: Write" for a file edit.
- [ ] Send a prompt engineered to hit a plan/output limit (the "plan-limit fixture") and confirm Devlings shows a clear stopped/limited state, not a silent hang.
- [ ] Set `ANTHROPIC_API_KEY=sk-test` in the environment Devlings is launched from, then Ask: the run still succeeds on the subscription (confirms the key is scrubbed, not used). Then `claude auth login --console` (API-key login) and Ask again: refused with the subscription message. Log back in with `claude auth login` afterward.
- [ ] Ask against a folder Claude Code doesn't already trust that has project config (a `.claude/settings.json` with a hook and an `env` entry, a `.mcp.json` server): the run uses `--setting-sources user` (the project hook doesn't fire), and the chat shows the notice listing what was skipped (hook event names, `env` key names but never values, MCP server names, helper keys, skill count) with a "Trust this folder in Devlings" action. Using it switches the row to "Trusted in Devlings. The next Ask uses this folder's settings." and the next Ask in that folder runs normally with no notice (the hook fires). "Stop trusting" (on the row or in Settings → Trusted folders) brings the notice back on the next Ask.
- [ ] Ask in a folder Claude Code already trusts, and in an untrusted folder with no `.claude` folder or `.mcp.json`: no notice, normal flags.
- [ ] With a parent folder trusted in Claude Code (e.g. the home folder), Ask in a git repository under it that has project config and was never trusted itself: the notice still appears. Trusting the repository in Claude Code (not the parent) makes it go away.

## G3.4 — Permission answering in VS Code, the terminal and Ask

- [ ] With watch-approvals **off** (default), a permission prompt in VS Code/terminal behaves exactly as it does without Devlings running (Devlings only shows "needs input", it doesn't answer).
- [ ] After upgrading from a build without approvals (hooks installed, onboarding done), the intro card appears once; **Not now** hides it for good, **Turn on** switches watch-approvals on.
- [ ] Turn watch-approvals on (Settings → Approvals). A permission prompt in VS Code and in the terminal shows an approval card above the pet at the same moment as Claude Code's own prompt, with the exact command, Claude's description, Deny / Allow and the always button when Claude Code offered a rule, plus a countdown bar. Nothing is focused and Enter does nothing.
- [ ] **Allow** in Devlings: Claude Code's own prompt goes away and the tool runs. **Deny**: the tool is blocked with "The user declined this in Devlings." **Always allow / Allow for this session**: the tool runs and the rule applies to the next matching call.
- [ ] Answer in Claude Code instead: the card disappears once the tool has run (or at the next prompt); clicking the stale card before then does nothing harmful.
- [ ] Let the hold run out (set 30 s): the card goes, the thread shows "needs input", and Claude Code's own prompt still works.
- [ ] A fresh card's buttons are faded and ignore clicks for about a second, then fade in; when a card above or below it goes away (placeholder "Answered" / "No longer waiting"), or the cards flip below the pet, they fade out again briefly.
- [ ] An MCP tool card shows the raw `mcp__…` name and opens Details by default; a Write/Edit card's Details show the content / old and new text; a very large Write offers only Deny with "Too long to review here. Answer in Claude Code."
- [ ] A question (AskUserQuestion) or plan (ExitPlanMode) in a watched session shows "Has a question for you" / "Has a plan for you to review" with no card; Claude Code's own dialog handles it. In an Ask, the chat shows a note and Claude carries on.
- [ ] In an Ask run, an approval renders inline in the mini chat and as a card; Allow/Deny work; Stop and New chat both cancel any pending request for that run.

## G3.5 — Recovery from an extension path change and an off-screen position

- [ ] Rename/move the VS Code Claude Code extension's currently-used version folder (simulating an update) and start an Ask: Devlings re-locates the binary **on its own** (M9 — no manual Auto-detect click needed) and the Ask still runs; **Settings → Claude Code** shows the new path/version afterward.
- [ ] Point **Choose file…** at a binary, then delete that file and start an Ask: Devlings falls back to auto-detecting a working binary instead of failing.
- [ ] Manually write an off-screen `x`/`y` into the pet's saved position (outside any connected monitor's bounds — e.g. after a monitor is disconnected) and relaunch: Devlings clamps the pet back into the visible work area instead of leaving it unreachable.
- [ ] Drag the pet flush against the top, left, right and bottom edges and into every corner of the primary monitor: the sprite itself reaches the physical edge (M9/D9 — only the sprite is clamped, not the whole window). Repeat on a secondary monitor with a different DPI scale if one is available.
- [ ] With a card open (composer or a thread bubble), drag the pet to the top edge: the card flips to open **below** the pet instead of being pushed off-screen, without the pet jumping. Drag back down: it flips back above. Small drags right at the flip point don't flicker back and forth.
- [ ] With a card open, drag the pet near the left and right edges: the card shifts sideways to stay fully on screen instead of being clipped.

## G3.6 — 60-minute soak; idle CPU and memory

- [ ] Leave Devlings running idle (hooks installed, no active Ask, no active sessions) for 60 minutes with Task Manager (Details tab, "devlings.exe") sampled every 5–10 minutes.
- [ ] Idle CPU stays near the ~1% target (brief spikes on hook events are fine; a sustained climb is not). Memory doesn't grow unbounded over the hour (a few MB of drift is fine; tens of MB climbing linearly is a leak — file an issue with the numbers rather than shipping over it).

## Before publishing (see `docs/release/v1.0.0.md` for the rest of G7)

- [ ] Every gate above is ✅ or explicitly ⚠️ waived with a reason recorded.
- [ ] Anthropic's current Claude Code terms and usage docs re-checked for anything affecting tools that launch the local `claude` binary; update the README Cost section if needed (G5.5).
- [ ] The v1.0.0 milestone has zero open bugs (G7.4).
