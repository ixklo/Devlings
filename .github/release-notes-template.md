<!--
Template for a GitHub release's notes. Copy this into the release body (the
release workflow creates a draft with a placeholder; replace it with a
filled-in copy of this file) and delete anything in [brackets], including
this comment.
-->

# Perch [vX.Y.Z]

[One or two sentences: what this release is for.]

## What's new

- [Feature or fix, described in plain language, not commit messages.]
- [...]

## Upgrading

[If nothing special is needed: "Updating from the previous version keeps your pet name, hooks and projects — Perch installs this in place." If a manual step is required, e.g. upgrading from a 0.x release: say exactly what to do.]

Perch checks GitHub for updates at launch and once a day, and installs signed updates in-app; this can be turned off in Settings → About.

## Downloads

| System | File |
|---|---|
| Windows 10/11 | `Perch_[X.Y.Z]_x64-setup.exe` |
| macOS (Apple Silicon and Intel) | `Perch_[X.Y.Z]_universal.dmg` |
| Linux (AppImage) | `Perch_[X.Y.Z]_amd64.AppImage` |
| Linux (.deb) | `perch_[X.Y.Z]_amd64.deb` |

## Checksums and provenance

Each file's SHA-256 is in `SHA256SUMS.txt`, attached to this release. Every asset also has a GitHub build-provenance attestation, generated in CI from this exact source at this tag:

```
gh attestation verify <downloaded-file> --repo yeetstick/perch
```

## Known limits

- **macOS and Linux are beta:** builds and launches in CI, not yet tested on real hardware. [Update this line once real-hardware QA has happened.]
- **Windows 10** is untested (Windows 11 is verified).
- Installers aren't notarized (macOS) or backed by an EV certificate (Windows); see the README's Install section for the SmartScreen/Gatekeeper steps.

[Delete this line and list any other limits specific to this release, or remove the bullet entirely once notarization/signing changes.]
