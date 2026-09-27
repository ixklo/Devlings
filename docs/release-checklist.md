# v1.0 release checklist

Manual QA for the gates CI can't cover: `docs/release/v1.0.0.md` gates **G2** (install lifecycle) and **G3** (features), on Windows — the only platform v1.0 verifies (macOS/Linux ship beta on CI evidence alone; see `docs/release/v1.0.0.md` for G4). Record the result of each gate (pass, or what failed) back into `docs/release/v1.0.0.md`'s Evidence column.

Run on a Windows 11 machine with Claude Code logged in with a subscription, and VS Code with the Claude Code extension installed. Start from a clean state: quit Perch, delete `%APPDATA%\io.github.perchpet.perch\`, and make sure `~/.claude/settings.json` has no Perch hooks. Keep the previous release's installer (currently v0.2.0) on hand for the upgrade tests.

## G2.1 — Fresh install, no terminal

- [ ] Install from the built NSIS installer with no terminal open at any point. Onboarding walks through: name the pet, install hooks, three green setup checks.
- [ ] `settings.json` now has Perch's hook entries (§3 of `docs/specs/2026-09-26-perch-v1.0-design.md`: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PostToolUseFailure, Notification, PermissionDenied, PermissionRequest, Stop, StopFailure, SessionEnd) and a `settings.json.perch-backup-*` exists.
- [ ] A live Claude Code session in VS Code shows up as a card within a few seconds.

## G2.2 — Upgrade from v0.2.0 keeps settings

- [ ] Install v0.2.0 first, set a custom pet name, a non-default port (trigger "Port in use" to force one), toggle notifications off, and drag the pet to a memorable custom spot (not the default bottom-right).
- [ ] Install the new build over it (don't uninstall first). Pet name, port and notification setting all survive, and the pet reappears at the **same visual spot** it was left at (M9/D9: a saved position now means the sprite's anchor, migrated once from v0.2's window-anchor meaning).
- [ ] Hooks reinstall once (v0.2's 7 HTTP entries differ in shape from v1's set) — confirm via a new `settings.json.perch-backup-*` and the updated hook list, but confirm it does **not** reinstall again on a second launch with no change.

## G2.3 — In-app update, RC to RC; tampered signature refused

- [ ] Publish two RCs (e.g. `v1.0.0-rc.1` and `v1.0.0-rc.2`) and install the first.
- [ ] With `autoUpdate` on, Perch finds rc.2 (launch check or the daily check via `PERCH_UPDATE_ENDPOINT` pointed at the RC feed) and shows the "ready, restart to install" card. Restart installs rc.2; **Settings → About** shows the new version.
- [ ] Repeat with a `latest.json` whose signature has been altered by one byte: the update is refused (no install), and this is visible somewhere a user would notice (an error state, not a silent no-op).
- [ ] With an Ask run active, "Restart to install" does not appear (or is disabled) until the run ends.

## G2.4 — Uninstaller removes exactly Perch's hooks and start-at-login

- [ ] With hooks installed and "start at login" on, run the uninstaller. `settings.json` no longer has any Perch hook entries (diff against the pre-uninstall backup: only Perch's lines are gone), and the start-at-login entry is gone.
- [ ] Installing an **update** (not an uninstall) over an existing install does not run this cleanup — hooks and start-at-login survive the upgrade.

## G2.5 — Quit with hooks installed, no delay or noise *(waivable)*

- [ ] Quit Perch, then use Claude Code normally (a prompt, a tool call, ending the session). No noticeable delay and no hook-error text on stderr or in the transcript. If this fails, waive it in `docs/release/v1.0.0.md` with the specific noise observed.

## G3.1 — Every control in the README table works

Work through the README's "Using it" table on a live pet with at least one active session and one finished one: click the pet, the pencil, a card, hover a card, click a card's ×, the bell, the chevron, drag the pet (arrow keys, then Esc), right-click the pet (Settings, change pet, hide for an hour, quit), and Ctrl+Alt+P.

## G3.2 — Watch states; "needs input" within ~1 s

- [ ] Drive a session from VS Code through running → done; the card matches at each step.
- [ ] Drive a session from a terminal the same way.
- [ ] Trigger a permission prompt (Manual mode, ask for a shell command) and time from the prompt appearing to the pet's "?" badge: under ~1 s (D5 in the v1.0 design doc — PermissionRequest fires the state immediately).

## G3.3 — Ask in all modes; plan-limit fixture; money guard; untrusted folders

- [ ] Ask in each permission mode (default, acceptEdits, Manual/plan) and confirm the mode chip and behavior match — e.g. Manual mode surfaces a permission card instead of running silently, Read-only mode shows "Blocked: Write" for a file edit.
- [ ] Send a prompt engineered to hit a plan/output limit (the "plan-limit fixture") and confirm Perch shows a clear stopped/limited state, not a silent hang.
- [ ] Set `ANTHROPIC_API_KEY=sk-test` in the environment Perch is launched from, then Ask: the run still succeeds on the subscription (confirms the key is scrubbed, not used). Then `claude auth login --console` (API-key login) and Ask again: refused with the subscription message. Log back in with `claude auth login` afterward.
- [ ] Ask against a folder Claude Code doesn't already trust that has project config (a `.claude/settings.json` with a hook and an `env` entry, a `.mcp.json` server): the run uses `--setting-sources user` (the project hook doesn't fire), and the chat shows the notice listing what was skipped (hook event names, `env` key names but never values, MCP server names, helper keys, skill count) with a "Trust this folder in Perch" action. Using it switches the row to "Trusted in Perch. The next Ask uses this folder's settings." and the next Ask in that folder runs normally with no notice (the hook fires). "Stop trusting" (on the row or in Settings → Trusted folders) brings the notice back on the next Ask.
- [ ] Ask in a folder Claude Code already trusts, and in an untrusted folder with no `.claude` folder or `.mcp.json`: no notice, normal flags.
- [ ] With a parent folder trusted in Claude Code (e.g. the home folder), Ask in a git repository under it that has project config and was never trusted itself: the notice still appears. Trusting the repository in Claude Code (not the parent) makes it go away.

## G3.4 — Permission answering in VS Code, the terminal and Ask

- [ ] With watch-approvals **off** (default), a permission prompt in VS Code/terminal behaves exactly as it does without Perch running (Perch only shows "needs input", it doesn't answer).
- [ ] Turn watch-approvals on. With a Claude Code host (VS Code, the terminal, `claude` itself) in the foreground, a permission request is answered immediately (`{}`, i.e. handled natively) per D3. With no such host focused, Perch holds the request for `approvalHoldSecs` and shows the approval card with Deny / Allow / (Always allow, when Claude Code sent a suggestion).
- [ ] In an Ask run, an approval renders inline in the mini chat; Stop and New chat both cancel any pending request for that run.

## G3.5 — Recovery from an extension path change and an off-screen position

- [ ] Rename/move the VS Code Claude Code extension's currently-used version folder (simulating an update) and start an Ask: Perch re-locates the binary **on its own** (M9 — no manual Auto-detect click needed) and the Ask still runs; **Settings → Claude Code** shows the new path/version afterward.
- [ ] Point **Choose file…** at a binary, then delete that file and start an Ask: Perch falls back to auto-detecting a working binary instead of failing.
- [ ] Manually write an off-screen `x`/`y` into the pet's saved position (outside any connected monitor's bounds — e.g. after a monitor is disconnected) and relaunch: Perch clamps the pet back into the visible work area instead of leaving it unreachable.
- [ ] Drag the pet flush against the top, left, right and bottom edges and into every corner of the primary monitor: the sprite itself reaches the physical edge (M9/D9 — only the sprite is clamped, not the whole window). Repeat on a secondary monitor with a different DPI scale if one is available.
- [ ] With a card open (composer or a thread bubble), drag the pet to the top edge: the card flips to open **below** the pet instead of being pushed off-screen, without the pet jumping. Drag back down: it flips back above. Small drags right at the flip point don't flicker back and forth.
- [ ] With a card open, drag the pet near the left and right edges: the card shifts sideways to stay fully on screen instead of being clipped.

## G3.6 — 60-minute soak; idle CPU and memory

- [ ] Leave Perch running idle (hooks installed, no active Ask, no active sessions) for 60 minutes with Task Manager (Details tab, "perch.exe") sampled every 5–10 minutes.
- [ ] Idle CPU stays near the ~1% target (brief spikes on hook events are fine; a sustained climb is not). Memory doesn't grow unbounded over the hour (a few MB of drift is fine; tens of MB climbing linearly is a leak — file an issue with the numbers rather than shipping over it).

## Before publishing (see `docs/release/v1.0.0.md` for the rest of G7)

- [ ] Every gate above is ✅ or explicitly ⚠️ waived with a reason recorded.
- [ ] Anthropic's current Claude Code terms and usage docs re-checked for anything affecting tools that launch the local `claude` binary; update the README Cost section if needed (G5.5).
- [ ] The v1.0.0 milestone has zero open bugs (G7.4).
