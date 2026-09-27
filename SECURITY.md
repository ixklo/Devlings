# Security policy

## Reporting a vulnerability

Please report security issues privately, not in a public issue.

Use GitHub's private vulnerability reporting: go to the [Security tab](../../security/advisories/new) of this repository and click **Report a vulnerability**. That opens a private draft advisory visible only to you and the maintainer, and is the fastest way to reach us.

Please include:

- What the issue is and why it's a security problem, not just a bug.
- Steps to reproduce, or a proof of concept.
- The affected version (Settings → About → Copy diagnostics gives this, along with OS and Claude Code version).

We'll acknowledge reports as quickly as we can and keep you updated as we work on a fix. Once a fix ships, we'll credit you in the release notes unless you'd rather stay anonymous.

## Scope

This covers Perch itself: the Tauri/Rust backend, the frontend UI, the installers, and the auto-updater. In particular:

- **The localhost hook server.** Perch runs a small HTTP server on `127.0.0.1` that receives Claude Code hook events and answers permission requests. It's bound to loopback only, and every request must carry Perch's per-install token. Ways to reach it from outside localhost, forge or guess the token, or use it to run something Perch didn't intend, are all in scope.
- **The Claude Code hooks Perch installs** into `settings.json`, and the uninstaller/relay that removes them.
- **The auto-updater**, including signature verification of update artifacts.
- **The Ask flow's subscription-only guard** (the "money guard" that's meant to stop a run before it would use paid API credits).

Out of scope: vulnerabilities in Claude Code itself (report those to Anthropic), and vulnerabilities in a third-party dependency that don't have a working exploit path through Perch (report those upstream; see `THIRD_PARTY_NOTICES.md` for what's bundled, and feel free to also flag it here so we can track an upgrade).

## Supported versions

Perch is pre-1.0 and ships one supported line: the latest published release. Security fixes land in the next release rather than being backported. Once 1.0 ships, this section will be updated if that changes.

| Version | Supported |
|---|---|
| Latest release | Yes |
| Older releases | No |

## Keeping up to date

Perch checks GitHub for updates at launch and once a day, and installs signed updates in-app (Settings → About shows the current version, and the check can be turned off there). Running the latest release is the best way to get security fixes.
