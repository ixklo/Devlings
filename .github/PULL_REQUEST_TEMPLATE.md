## What does this change?

<!-- One or two sentences: what, and why. Link an issue if there is one. -->

## How was this tested?

<!-- CI runs tests/typecheck/build/clippy on Windows, macOS and Linux automatically, plus
     cargo-deny (Linux only, whole dependency graph) and npm-audit (all three).
     Note anything CI can't cover: manual QA steps, screenshots, platforms you checked by hand. -->

## Checklist

- [ ] `CHANGELOG.md` has an entry under `[Unreleased]` (or this change isn't user-visible)
- [ ] If a shipped dependency changed, `THIRD_PARTY_NOTICES.md` was regenerated (`npm run notices:generate`)
- [ ] Tests were added or updated for the behavior this changes
