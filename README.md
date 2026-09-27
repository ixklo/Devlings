<p align="center"><img src="docs/screenshots/bubbles.png" width="420" alt="Perch, a pixel-art bird sitting on a branch, with two thread cards above it: one running, one done"></p>

<h1 align="center">Perch</h1>

<p align="center">A pixel-art pet that floats on your desktop and keeps you posted on Claude Code.</p>

---

Perch sits above your other windows. It shows what every Claude Code session is doing, pops a card up when one finishes or needs you, and lets you ask Claude Code something without switching to VS Code or a terminal.

- **Watch.** Any Claude Code session (VS Code, terminal, desktop app) shows up as a card above the pet: running, needs input, ready or blocked. Click a card to jump back to that project.
- **Ask.** Click the pet or the pencil, pick a project folder and type. Perch runs your own Claude Code in that folder and streams the answer into a small chat above the pet. Follow-ups continue the same conversation.
- **Answer permission prompts.** When Claude Code asks to run a command or edit a file, Allow or Deny it from a card above the pet (see [Permission prompts](#permission-prompts)).
- **It reacts.** The pet types while Claude works, holds up a "?" when it needs you, inspects the result when it's ready, and gets a little storm cloud when something fails. Drag it and it runs; hover and it waves.
- **Pick a pet.** Three pets are built in, and you can name yours. Perch also reads pets in the Codex pet format (see below).
- **Stays out of the way.** Everything around the pet is click-through, so the window never blocks what's behind it.

<p align="center"><img src="docs/screenshots/states.png" width="640" alt="The pet's moods, left to right: idle, working at a laptop, asking a question, inspecting with a magnifying glass, sad under a storm cloud"></p>

<table>
  <tr>
    <td align="center"><img src="docs/screenshots/composer.png" width="380" alt="The composer card above the pet, with a project chip, a mode chip and a send button"><br><sub>Ask from the pet</sub></td>
    <td align="center"><img src="docs/screenshots/chat.png" width="300" alt="The mini chat above the pet, showing a finished answer with formatted text"><br><sub>Answers stream into a mini chat</sub></td>
  </tr>
</table>

Free and open source (MIT). Perch is not affiliated with Anthropic.

## Requirements

- [Claude Code](https://code.claude.com) 2.1.259 or newer, logged in with a Claude subscription (Pro, Max, Team or Enterprise). The binary bundled with the VS Code extension works too.
- **Windows 11** (verified). Windows 10 isn't tested yet, but there's no known reason it wouldn't work.
- **macOS and Linux** (beta): the installers build and launch in CI on every change, but haven't been tested on real hardware yet. If you try one, an [issue](../../issues/new/choose) reporting how it went (or what broke) is genuinely useful.

## Install

Download the installer for your system from [Releases](../../releases). Perch walks you through setup on first run, in three steps: pick and name your pet and let it watch your sessions, see how asking from the pet works (with a check that Claude Code is ready), and choose whether it answers permission prompts for your other sessions.

The installers aren't code-signed by a paid certificate, so each OS shows an unfamiliar-software warning the first time:

- **Windows:** SmartScreen says "Windows protected your PC". Click **More info**, then **Run anyway**.
- **macOS:** Gatekeeper blocks the first launch. Since macOS 15, Control-click → Open no longer bypasses this, so instead: open **System Settings → Privacy & Security**, scroll to the bottom, and click **Open Anyway** next to Perch, then confirm in the dialog that follows. macOS builds are ad-hoc signed (not notarized), which is why this step is needed.
- **Linux (AppImage):** make it executable first — `chmod +x Perch_*.AppImage` — then run it.
- **Linux (.deb):** `sudo apt install ./Perch_*.deb` (or double-click it in a graphical package installer).

## Updates

Perch checks GitHub for a new release at launch and once a day, and installs signed updates in-app — no need to redownload an installer. Turn this off in **Settings → About**.

Upgrading from a 0.x release needs one manual install of the new version; after that, in-app updates take over.

## Using it

| Do this | To |
|---|---|
| Click the pet, or the pencil | Ask Claude Code about a project |
| Click a card | Open that chat, or jump to the project (VS Code if found, otherwise the folder) |
| Click a notification (Windows) | The same as clicking that thread's card |
| Hover a card, click × | Mark it seen |
| Deny, Allow or Always allow on a permission card | Answer that Claude Code permission prompt (click only; no key answers it) |
| Bell | Turn notifications on or off |
| Chevron | Collapse the cards (a badge shows the count) |
| Drag the pet | Move it (arrow keys nudge it, Esc sends it home) |
| Right-click the pet | Settings, change pet, hide for an hour, quit |
| Ctrl+Alt+P | Show or hide the pet |
| Tab, Enter or Space, Esc | Use the pet, its cards and Settings from the keyboard (the menu key opens the pet's menu) |

Screen readers hear each status change once, for example "api-server: needs input", and Perch follows your system's reduced-motion setting.

Clicking a notification to open its thread works on Windows only for now. On macOS and Linux, notifications still appear, but clicking one does nothing.

## Permission prompts

When Claude Code wants to run a command, edit a file or fetch a page and needs your OK, Perch shows the request as a card above the pet: the project, the tool, the exact command, file path or URL, and Claude's own description of what it's doing. **Details** shows everything the tool will receive (a file's new content, an edit's old and new text); it opens by itself when the headline doesn't tell the whole story, for example for MCP tools, and a warning badge marks risky flags such as running outside the sandbox. Answer with **Deny** or **Allow**.

- **No accidental answers.** Nothing is pre-selected and no key answers a card, so a stray Enter never approves anything. The buttons also stay inactive until a card has sat still for a moment, and go inactive again whenever it moves, so a card that just popped up or slid into place can't catch a click meant for something else.
- **Too long to review:** if a request is too big to show in full, Perch only offers **Deny**; answer it in Claude Code instead.
- **Questions and plans** (Claude asking you something, or asking you to approve a plan) are left to Claude Code's own dialog for your other sessions. In an Ask, Perch tells Claude to make a reasonable assumption and say so, or to present the plan in the chat for you to review.

- **Asks from Perch:** always on. The request also shows inline in the mini chat. **Stop** or **New chat** turns down anything still waiting, and a request nobody answers is turned down after 10 minutes.
- **Your other Claude Code sessions** (VS Code, the terminal): opt-in, under **Settings → Approvals** (or the one-time card after updating). Claude Code's own prompt still appears at the same time and keeps working, so you can answer in either place; whichever answers first wins, and the card goes away once the request is answered in Claude Code. Perch keeps a card open for 30 seconds, 1, 2 or 4 minutes (1 minute by default); after that the card goes and the request simply waits in Claude Code as usual. One side effect: a headless `claude -p` run started by some other tool has no prompt of its own, so while this is on it waits up to that long for your answer in Perch, and without one Claude Code turns the tool down, just as it would without Perch.
- **Always allow** appears only when Claude Code itself suggests a rule for the request. It applies exactly that suggestion, and the line under it says what it does, for example "Adds the rule Bash(npm test:*) to this project's local settings". When every suggestion only lasts for the session, the button reads **Allow for this session**. A mode switch is only ever offered for the session, and never to the mode that skips all permission checks.
- Perch's answers go to Claude Code over the same `127.0.0.1` hook connection (or, for Asks, Claude Code's own input), never anywhere else.

## Pets

<p align="center"><img src="docs/screenshots/pets.png" width="420" alt="The three built-in pets: teal Perch, orange Ember and purple Plum"></p>

Perch uses the same sprite format as Codex pets: a 1536×1872 PNG or WebP atlas with 8 columns and 9 rows of 192×208 cells, plus a `pet.json`. Perch loads pets from:

- its own pets folder (**Settings → About → Open folder**, next to "More pets")
- `~/.codex/pets` (or `$CODEX_HOME/pets`), so pets you already have in Codex show up in Perch too

Each pet is a folder with `pet.json` (`id`, `displayName`, `description`, `spritesheetPath`) and the sprite sheet. The built-in pets are generated from pixel maps in [`scripts/pets`](scripts/pets).

### Making your own pet

A pet is a folder containing:

- **`pet.json`** — a small manifest:
  - `id`: a short, unique, lowercase identifier (used internally; not shown).
  - `displayName`: the name shown in the picker before you rename it.
  - `description`: a line shown in the pet picker.
  - `spritesheetPath` (optional): the atlas file's name, relative to the folder. Defaults to `spritesheet.png` or `spritesheet.webp` if omitted.
- **The atlas image** — a single PNG or WebP, exactly **1536×1872**, laid out as an 8-column × 9-row grid of **192×208** cells (20 MiB max). Each row is a fixed animation, read left to right; unused cells at the end of a row are simply not drawn, but the row itself must exist. In row order:

  | Row | Animation | Frames |
  |---|---|---|
  | 0 | Idle | 6 |
  | 1 | Running right | 8 |
  | 2 | Running left | 8 |
  | 3 | Waving (on hover) | 4 |
  | 4 | Jumping | 5 |
  | 5 | Failed (storm cloud) | 8 |
  | 6 | Waiting (needs input) | 6 |
  | 7 | Working | 6 |
  | 8 | Reviewing a result | 6 |

  Frame timing is fixed by Perch (it follows the Codex pet contract), so you only need to draw the art — not configure durations.

Drop the folder into Perch's pets folder or `~/.codex/pets`, then pick it from the right-click menu or Settings. Perch validates the manifest and image dimensions and will tell you what's wrong if a pet doesn't load.

## Uninstall

- **Windows:** run the uninstaller (from the Start menu or Settings → Apps). It removes exactly the hooks Perch added to Claude Code's `settings.json`, along with the start-at-login entry, before removing the app itself.
- **macOS / Linux:** there's no uninstaller, so do this first: open **Settings → Watching → Remove**, which removes exactly Perch's hook entries from `settings.json` (a backup is kept alongside it). Then delete the app (drag it out of Applications on macOS, or remove the AppImage/`.deb` on Linux).

## Troubleshooting

- **Hooks aren't firing** (cards never update). Confirm **Settings → Watching** shows "Hooks installed" — if not, click **Install**. If it is installed and sessions still don't show up, restart the Claude Code session (hooks are read when a session starts), and check that nothing else has since edited `~/.claude/settings.json` and removed Perch's entries.
- **"Port in use" under Settings → Watching.** Something else on your machine is bound to Perch's hook port. Click **Move port** — Perch picks a new one and reinstalls its hooks pointing at it.
- **"Claude Code not found"** in setup. Click **Choose file…** and point it at your `claude` (or `claude.exe`) binary, or **Auto-detect** to have Perch look again. The VS Code extension's bundled binary works too.
- **Windows SmartScreen or macOS Gatekeeper** blocking install or launch: see [Install](#install) above.

Still stuck? Open an [issue](../../issues/new/choose) with **Settings → About → Copy diagnostics** attached.

## Cost

Perch never uses an API key and has no login of its own.

- **Watching is free.** It only listens to Claude Code's hooks, so no prompts are sent.
- **Asks use your Claude subscription.** Perch runs your own Claude Code in non-interactive mode (`claude -p`). Anthropic meters that kind of use separately from interactive Claude Code: it draws on a monthly Agent SDK credit that comes with paid plans and that you claim once in your Claude account. If the credit runs out and paid usage credits are off, Asks stop working until it refreshes; interactive Claude Code isn't affected. See [Anthropic's help article](https://support.claude.com/en/articles/15036540) for the current amounts.
- **Plan usage.** **Settings → Cost** shows how much of your plan's limit is used, as Claude Code reported it during your last Ask, with the time it was seen. It isn't live and Perch makes no extra requests for it; after a restart it's empty until your next Ask.
- **Guards.** Before every Ask, Perch checks that Claude Code is logged in with a subscription and strips API-key environment variables from the process. It stops a run the moment Claude Code reports it would start drawing on paid usage credits.

## How watching works

On first run, Perch asks to add a few hooks to your Claude Code `settings.json`:

- Most events are `http` hooks that send the session's event to `http://127.0.0.1:<port>/hook/<random token>` with a short timeout. If Perch isn't running, they fail fast and silently, and Claude Code carries on.
- `Stop`, `StopFailure` and `SessionEnd` run Perch itself as a tiny background relay (`perch --hook-relay …`) instead, because Claude Code shows an error for those events when an `http` hook can't connect. The relay always exits within about a second, so quitting Perch never adds noise or delay to Claude Code.
- The `PermissionRequest` hook allows up to 5 minutes, so Perch can keep a card open while you decide. It only waits while **Settings → Approvals** is on; otherwise Perch replies "no decision" at once. Either way, Claude Code's own prompt never waits for it (see [Permission prompts](#permission-prompts)).

A backup of `settings.json` is saved before every change, and Perch never rewrites the file on start or quit. **Settings → Watching → Remove** removes exactly Perch's entries and nothing else.

## Privacy

Perch sends nothing anywhere itself except two things, both opt-outable: hook events and permission answers on `127.0.0.1` (never leaves your machine), and a check for a newer version at launch and once a day, which downloads a small `latest.json` file from this repository's latest GitHub release (turn it off in Settings → About). Your prompts go to Anthropic through your own Claude Code, exactly as they would from a terminal. See [SECURITY.md](SECURITY.md) for the full security scope and how to report an issue.

### Untrusted folders

Claude Code asks before it trusts a new folder, but the non-interactive runs Perch uses for Asks can't ask. So when you Ask in a folder you haven't trusted in Claude Code (inside a git repository, that means the repository itself or a folder in it; trusting a parent folder such as your home folder doesn't count, just as in Claude Code), and that folder has its own Claude Code setup (a `.claude` folder or a `.mcp.json`, in the folder or its git repository's root), Perch runs Claude Code without the folder's project settings: its hooks, environment variables, helper commands such as `apiKeyHelper`, MCP servers and project skills are all skipped. The chat says exactly what was skipped (environment variables by name only) and offers to trust the folder in Perch; after that, Asks there use the folder's settings. To undo it, use **Stop trusting** in the chat or under **Settings → Trusted folders**. Perch only reads Claude Code's own trust list (`~/.claude.json`) and never changes it.

## Third-party notices

Perch bundles open-source Rust crates and npm packages; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for the full list and license texts (also linked from **Settings → About**).

## Develop

Needs Node.js 24 and stable Rust. See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide, including what CI checks and when to regenerate third-party notices.

```bash
npm install
npm run tauri dev      # run the app
npm test               # frontend tests
npm run build && cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests
```

Opening `npm run dev` in a normal browser shows the pet (`/?window=pet`) and settings (`/?window=settings`) with mock data.

To rebuild the built-in pets: `cd scripts/pets && npm install && node build.mjs && node validate.mjs`.

Design docs: [v0.1](docs/specs/2026-09-25-perch-design.md), [v0.2 redesign](docs/specs/2026-09-26-perch-v0.2-design.md), [v1.0](docs/specs/2026-09-26-perch-v1.0-design.md). Release gates: [docs/release/v1.0.0.md](docs/release/v1.0.0.md).
