# Changelog

All notable changes to Devlings (called Perch up to 1.0) are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Six new pets:** Pip the fox (its tail swishes while it works), Miso the cat (smug when reviewing, then a big stretch), Nori the axolotl (frills flutter while it waits), Bean the capybara (an orange on its head at idle), Bolt the robot (its antenna blinks while it works) and Wisp the ghost (floats, and fades a little when idle). Nine pets are now built in; Perch stays the default.

### Changed

- **Perch is now Devlings.** The app has a new name; Perch the teal bird stays, as the default pet. Your pet, its name, projects, settings and Claude Code hooks carry over. On Windows, installing Devlings (or Perch's in-app update) removes Perch for you; on macOS and Linux, remove Perch yourself after installing (see "Upgrading from Perch" in the README). The installers are now `Devlings_<version>_…`, updates come from the renamed repository, and the logs and `settings.json` backups are named after Devlings too. For testers: `DEVLINGS_UPDATE_ENDPOINT` replaces `PERCH_UPDATE_ENDPOINT`, which still works.
- The pet pickers in onboarding and Settings scroll, and the right-click **Change pet** menu lists every built-in pet, birds first.
- Switching pets keeps a name you chose; if you never renamed your pet, the new pet brings its own name.

### Fixed

- A Claude Code session stays with the folder it started in. Before, every subfolder a session moved into was added to your project list, and the session's card was renamed after it.
- Cards no longer drop underscores and asterisks from names like `get_user_id`, file names or `2*3*4` when they show Claude's reply as plain text, and notifications now show the reply as plain text too instead of raw Markdown.
- A conversation file that starts with a byte order mark no longer hides its first message.

## [1.0.0] - 2026-09-27

The first stable release: Perch updates itself, answers Claude Code's permission prompts, protects you in folders you haven't trusted, and uninstalls cleanly.

### Added

- **In-app updates.** Perch checks GitHub at launch and once a day, downloads signed updates in the background, verifies them, and shows a quiet "ready, restart to install" card. It never restarts during an Ask. **Settings → About** has "Check for updates" and an auto-check switch.
- **Answer Claude Code permission prompts from the pet.** An Ask that wants to run something shows the request in the mini chat with the exact command, Claude's description, and Deny, Allow and (when Claude Code offers a rule) Always allow. For your other Claude Code sessions it's opt-in (**Settings → Approvals**, or the one-time card after updating): the card appears alongside Claude Code's own prompt, which keeps working, and stays open for 30 seconds to 4 minutes.
- Permission cards show everything being approved under **Details** (open by default when the headline leaves something out), badge risky flags, and only offer Deny for requests too long to show. Their buttons only work once the card has sat still for a moment, so a card that just appeared or moved can't catch a stray click. Claude's questions and plan approvals are never answered with a plain Allow.
- A session shows "needs input" the moment Claude Code asks for permission, and goes back to working once it's answered anywhere.
- **Untrusted folders.** Asks in a folder Claude Code hasn't trusted no longer run that folder's project hooks, environment variables, helper commands, MCP servers or skills. The chat says what was skipped and offers to trust the folder in Perch; undo it from the chat or **Settings → Trusted folders**.
- **Clean uninstall on Windows.** The uninstaller removes exactly Perch's hooks and its start-at-login entry, keeping a backup of Claude Code's `settings.json`; updates and reinstalls leave them alone.
- **Plan usage** in **Settings → Cost**, as Claude Code reported it during your last Ask ("42% of your 5-hour limit, resets 3:10 PM"), always labelled with the time it was seen, plus a short note in the mini chat near a limit. No extra requests.
- On Windows, **clicking a notification** brings up the pet and opens that session, like clicking its card.
- **Keyboard and screen reader support.** Every control is reachable with Tab and shows a clear focus ring; Enter or Space on the pet opens the composer; menus and pickers take arrow keys; focus returns to the pet when a card closes. Cards and buttons are named with their project and status, and a screen reader hears each status change once, never a reply as it streams.
- **Diagnostics.** A rotating log file, **Copy diagnostics** (versions, how Claude Code was found, hook status and the last 200 log lines, with the hook token and your home folder redacted) and **Open log folder** in **Settings → About**.
- `THIRD_PARTY_NOTICES.md` covering every Rust crate and npm package that ships, linked from **Settings → About**.

### Changed

- **Quitting Perch no longer bothers Claude Code.** The Stop, StopFailure and SessionEnd hooks now go through a tiny `perch --hook-relay` command that exits quietly within a second, so Claude Code shows no hook errors and doesn't slow down while Perch is closed. Existing hooks are updated once, with a backup.
- **Much less work while idle** (about 0.2% CPU on the test machine): the idle animation rests between loops, nothing animates or polls while the pet or Settings is hidden or minimized, the pointer check slows down far from the pet, and the "needs input" dot pulses three times, then stays lit.
- **Onboarding** explains Perch in three short steps: watching your sessions (with the hooks), asking from the pet (your subscription, never an API key, and how Asks are counted), and answering permission prompts.
- Asks now answer permission prompts in Perch instead of denying every tool that needs approval.
- Text, muted labels, badges, switches and focus rings meet WCAG AA contrast in light and dark mode. With reduced motion on, nothing pulses, spins or slides.
- The README's Cost section explains how Asks are metered (the plan's Agent SDK credit) and that watching is free.

### Fixed

- The pet can sit flush against every screen edge and corner: cards open below it when there isn't room above, shift sideways near a side edge, and are capped to the room on screen. A saved position from 0.2 lands on the same spot.
- "Open in VS Code" also checks common install locations and the `vscode://` link, not just `code` on PATH.
- Perch finds the Claude Code binary again on its own when the remembered path stops working (for example after the VS Code extension updates).
- An unseen "done" or "blocked" card stays until you've seen it, even when its session ends right after finishing.
- If start-at-login is on in Perch's settings but the entry went missing (for example after uninstalling and reinstalling), Perch puts it back.
- The plan shown in Settings is labelled "as reported by Claude Code", since Claude Code's own cache can lag behind your plan.

### Security

- A strict content security policy: no remote scripts or images, and Markdown images render as their alt text; links open in your browser.
- The hook server compares its token in constant time, caps request bodies and never lets a stalled request hold up others; the token never appears in logs or diagnostics.
- Pet packages can't read outside their folder (no `..`, absolute paths or symlinks), and their size and dimensions are checked, again when read.
- `settings.json` is written atomically, never rewritten when it can't be parsed, and its backups are pruned to the newest five.
- A downloaded update is verified by signature and re-checked before it's installed.
- An unused window capability was removed.

### For contributors

- CI on Windows, macOS and Linux for every pull request and push to `main`: tests, typecheck and build, Rust tests and clippy, `cargo deny` (licenses and advisories) and `npm audit`, plus a macOS and Linux launch smoke test with a screenshot. `vitest` no longer passes with zero tests found.
- A release workflow that builds NSIS, a universal macOS DMG (ad-hoc signed and verified), an AppImage and a `.deb`, signed updater bundles, `latest.json`, `SHA256SUMS.txt` and build-provenance attestations; `-rc` tags publish as pre-releases.
- `scripts/set-version.mjs` / `check-version.mjs` keep the version in sync; `deny.toml` keeps dependencies MIT-compatible; `npm run notices:generate` rebuilds the notices.
- `SECURITY.md`, `CONTRIBUTING.md`, issue and pull request templates, a release-notes template, and a rewritten `docs/release-checklist.md` mapped to the gates in `docs/release/v1.0.0.md`.

## [0.2.0] - 2026-09-26

Perch v0.2 is a full redesign. The pet is now a pixel-art character that floats above your windows, shows each Claude Code session as a speech-bubble card, and opens a small chat right above itself.

### Added

- Pixel-art pets with real animations: typing at a laptop while Claude works, a "?" when it needs you, a magnifying glass when inspecting a result, and a storm cloud on failure. The pet waves on hover and runs while dragged.
- Thread cards above the pet for every Claude Code session (running, needs input, ready or blocked), with the most urgent shown first. Clicking a card opens that chat or jumps to the project in VS Code.
- A compact composer to ask Claude Code from the pet, with project and mode chips; answers stream into a mini chat with formatted text, and follow-ups continue the conversation.
- A control bar under the pet: compose, toggle notifications, collapse the cards (with an unread badge).
- Click-through everywhere except the pet and its cards, so the window never blocks what's behind it.
- Three built-in pets (Perch, Ember, Plum) plus Codex pet format support, so pets already in `~/.codex/pets` show up in Perch too. Pets and sizes are switchable from the right-click menu or Settings.
- A new Settings window with first-run onboarding to name the pet.
- Plain-language status text on cards (e.g. "Run the test suite") instead of raw commands.

### Changed

- Updating from v0.1 keeps the pet name, hooks and projects.

## [0.1.0] - 2026-09-25

First release of Perch: a small bird that sits on your desktop and keeps you posted on Claude Code.

### Added

- Watch any Claude Code session (VS Code, terminal, desktop app): see when it's working, needs your approval, or is done, with a notification when it matters.
- Ask: click the bird, pick a project folder and type. Perch runs your own Claude Code in that folder and streams the answer back; follow-ups continue the same conversation.
- Name your pet whatever you like.
- Windows, macOS and Linux installers. Tested on Windows 11; macOS and Linux compile and package but hadn't yet been tried on real hardware.

[Unreleased]: https://github.com/ixklo/devlings/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/ixklo/devlings/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/ixklo/devlings/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ixklo/devlings/releases/tag/v0.1.0
