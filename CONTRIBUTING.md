# Contributing to Perch

Thanks for taking an interest in Perch. It's a small, free, MIT-licensed project — contributions, bug reports and ideas are all welcome.

## Reporting bugs

Open an [issue](../../issues/new/choose) and pick the bug report template. The single most useful thing you can attach is **Settings → About → Copy diagnostics**, which includes your OS, Perch version, Claude Code version and recent (redacted) logs. Steps to reproduce help too.

Found a security issue instead? See [SECURITY.md](SECURITY.md) — please don't file it as a public issue.

## Suggesting a feature

Open an issue with the feature request template. A concrete scenario ("I do X, and Y happens, but I'd want Z") is more useful than a general idea.

## Developing

Needs Node.js 24 and stable Rust.

```bash
npm install
npm run tauri dev      # run the app
npm test               # frontend tests
npm run build && cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Opening `npm run dev` in a regular browser shows the pet (`/?window=pet`) and settings (`/?window=settings`) with mock data — useful for UI work without a real Claude Code session.

Design docs live in `docs/specs/`; release gates and evidence live in `docs/release/`.

## Before opening a pull request

- `npm test`, `npm run build`, `cargo test --manifest-path src-tauri/Cargo.toml` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` should all pass locally — CI runs the same checks on Windows, macOS and Linux and will block on any of them.
- If you touched a dependency, `cargo deny check` and `npm audit --omit=dev` run in CI too; a new license or advisory needs a documented exception in `deny.toml`, not a silent pass.
- If you added or changed a shipped dependency (a Rust crate in `src-tauri/Cargo.toml`'s `[dependencies]`, or an npm package in `package.json`'s `dependencies`), regenerate the notices: `npm run notices:generate` (needs `cargo install cargo-about --locked --features cli` once), and commit the updated `THIRD_PARTY_NOTICES.md`.
- Keep `src/` and `src-tauri/src/` changes covered by tests where practical; CI's smoke-test job only checks that the app launches and doesn't panic, it isn't a substitute for unit tests.
- Add a line to `CHANGELOG.md` under `[Unreleased]` for anything a user would notice.

## Pull requests

Fill in the pull request template — it's short. Small, focused PRs are easier to review than large ones; if a change is going to be big, opening an issue first to talk through the approach saves rework.

## Code of conduct

Be respectful. Disagreements about code and design are normal and fine; personal attacks aren't. Reports of a violation can go through the same private channel as security issues (see [SECURITY.md](SECURITY.md)) if you'd rather not raise it publicly.
