<p align="center"><img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="Perch, a small teal bird on a branch"></p>

# Perch

A small pet that sits on your desktop and keeps you posted on Claude Code, so you don't have to keep VS Code open to know what's happening.

- **Watch:** see when any Claude Code session (VS Code, terminal, desktop app) is working, needs your approval, or is done. You get a notification when it matters.
- **Ask:** click the pet, pick a project folder, and type. Perch runs your own Claude Code in that folder and streams the answer back. Follow-ups continue the same conversation.
- **Name it:** your pet, your name for it.

Free and open source (MIT). Perch is not affiliated with Anthropic.

## Requirements

- [Claude Code](https://code.claude.com) 2.1.259 or newer, logged in with a Claude subscription (Pro, Max, Team or Enterprise). The binary bundled with the VS Code extension works too.
- Windows 10/11 (primary), macOS or Linux (best effort).

## Install

Download the installer for your system from [Releases](../../releases).

The installers aren't code-signed yet:
- **Windows:** SmartScreen may say "Windows protected your PC". Click **More info → Run anyway**.
- **macOS:** right-click the app, choose **Open**, then **Open** again.

## Cost

Perch never uses an API key and has no login of its own. Asks run on your Claude subscription and use your plan's usage like any other prompt. Before every Ask, Perch checks that Claude Code is logged in with a subscription. It strips API-key environment variables from the process, and stops a run the moment Claude Code reports it would start drawing on paid usage credits.

## How watching works

On first run, Perch asks to add a few `http` hooks to your Claude Code `settings.json`. Each hook sends session events to `http://127.0.0.1:<port>/hook/<random token>` with a 2-second timeout. If Perch isn't running, the hook fails fast and Claude Code carries on. A backup of `settings.json` is saved before every change. **Settings → Remove hooks** removes exactly Perch's entries and nothing else.

## Privacy

Perch sends nothing anywhere itself. Its only network traffic is on localhost. Your prompts go to Anthropic through your own Claude Code, exactly as they would from a terminal.

## Develop

```bash
npm install
npm run tauri dev      # run the app
npm test               # frontend tests
npm run build && cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests
```

Design: [docs/specs/2026-09-25-perch-design.md](docs/specs/2026-09-25-perch-design.md)
