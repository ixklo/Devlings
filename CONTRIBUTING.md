# Contributing to Devlings

Thanks for taking an interest in Devlings. It's a small, free, MIT-licensed project — contributions, bug reports and ideas are all welcome.

## Reporting bugs

Open an [issue](../../issues/new/choose) and pick the bug report template. The single most useful thing you can attach is **Settings → About → Copy diagnostics**, which includes your OS, Devlings version, Claude Code version and recent (redacted) logs. Steps to reproduce help too.

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

Opening `npm run dev` in a regular browser shows the pet (`/?window=pet`) and settings (`/?window=settings`) with mock data — useful for UI work without a real Claude Code session. The query switches are listed at the top of `src/shared/mock.ts`; `?scene=cards|approval|chat|demo` shows the fixed, made-up states the README images are captured from.

Design docs live in `docs/specs/`; release gates and evidence live in `docs/release/`.

## Pets

- **Your own pet** doesn't need a pull request: [docs/making-pets.md](docs/making-pets.md) explains the folder, `pet.json` and the spritesheet, and where Devlings looks for pets.
- **The built-in pets** are generated from pixel maps in `scripts/pets`: `cd scripts/pets && npm install && node build.mjs && node validate.mjs`. `npm test` there (CI runs it on Linux) checks that the committed sheets are what the generator builds and that the three birds haven't changed. A new built-in pet also needs its id in `PET_IDS` (`scripts/pets/lib/contract.mjs`), `BUNDLED_ORDER` (`src-tauri/src/pets.rs`) and the browser preview's list (`src/shared/mock.ts`), so open an issue first.

## README images and the website

Everything in `docs/screenshots/`, plus `docs/social-preview.png` and `docs/pets/template.png`, is generated; re-run the scripts instead of editing the images, and check the results before committing.

- `cd scripts/pets && npm install && node render-docs.mjs` draws `pets.gif` (the nine pets idling) and the pet template straight from the atlases. Re-run it after changing a pet's art (`node build.mjs` first).
- `node render-site.mjs` (same folder) draws the landing page's own images into `site/img/`: the pet lineup at the top, the four moods and each pet's gallery tile, each with a still `.png` for visitors who ask for reduced motion. Re-run it after changing a pet's art. The site in `site/` is published to GitHub Pages by `.github/workflows/pages.yml`.
- `node capture-ui.mjs` (same folder; needs Microsoft Edge or Google Chrome, and `npm install` at the repository root) opens the real UI in a headless browser against the browser preview's made-up scenes and writes the screenshots, the hero `demo.gif` (each in a light and a `-dark` version) and the 1280×640 social preview. `--only cards,demo` and `--theme light` re-make just those. Re-run it after a UI change that shows in them. The scenes only use made-up projects under a placeholder home folder, so nothing personal ends up in an image.
- GitHub doesn't take the social preview from the repository: a maintainer uploads `docs/social-preview.png` under **Settings → General → Social preview**.
- `site/index.html` is a one-page website with the same story. It isn't published yet. It uses no JavaScript, web fonts or trackers, and should stay that way; when it's published, `img/` next to it holds the files from `docs/screenshots/`, `docs/pets/lineup.png` and `docs/social-preview.png`, and `favicon.png` is the app icon (`src-tauri/icons/128x128.png`).
- The README's and the site's download links point at the version-free installer names every release carries (`https://github.com/ixklo/devlings/releases/latest/download/Devlings-windows-x64-setup.exe` and friends, made by `scripts/stable-assets.mjs`); `npm test` checks both use exactly those names.

## Before opening a pull request

- `npm test`, `npm run build`, `cargo test --manifest-path src-tauri/Cargo.toml` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` should all pass locally — CI runs the same checks on Windows, macOS and Linux and will block on any of them.
- If you touched a dependency, CI also runs `cargo deny check` (Linux only, but it checks the whole dependency graph for every target, including Windows- and macOS-only crates) and `npm audit --omit=dev` (all three OSes); a new license or advisory needs a documented exception in `deny.toml`, not a silent pass.
- If you added or changed a shipped dependency (a Rust crate in `src-tauri/Cargo.toml`'s `[dependencies]`, or an npm package in `package.json`'s `dependencies`), regenerate the notices: `npm run notices:generate` (needs `cargo install cargo-about --locked --features cli` once), and commit the updated `THIRD_PARTY_NOTICES.md`.
- Keep `src/` and `src-tauri/src/` changes covered by tests where practical; CI's smoke-test job only checks that the app launches and doesn't panic, it isn't a substitute for unit tests.
- Add a line to `CHANGELOG.md` under `[Unreleased]` for anything a user would notice.
- If your change shows in the README's images, re-make them (see above).

## Pull requests

Fill in the pull request template — it's short. Small, focused PRs are easier to review than large ones; if a change is going to be big, opening an issue first to talk through the approach saves rework.

## Code of conduct

Be respectful. Disagreements about code and design are normal and fine; personal attacks aren't. Reports of a violation can go through the same private channel as security issues (see [SECURITY.md](SECURITY.md)) if you'd rather not raise it publicly.
