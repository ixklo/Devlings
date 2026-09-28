<!--
Template for a GitHub release's notes. Copy this into the release body (the
release workflow creates a draft with a placeholder; replace it with a
filled-in copy of this file) and delete anything in [brackets], including
this comment.
-->

# Devlings [vX.Y.Z]

[One or two sentences: what this release is for.]

## What's new

- [Feature or fix, described in plain language, not commit messages.]
- [...]

## Upgrading

[If nothing special is needed: "Updating from the previous version keeps your pet name, hooks and projects — Devlings installs this in place." If a manual step is required, e.g. upgrading from a 0.x release: say exactly what to do.]

[For 1.1, the first release named Devlings: "Perch is now Devlings. Your pet, its name, projects, settings and hooks carry over. On Windows, installing Devlings (or Perch's in-app update) closes and removes Perch for you. On macOS, delete Perch from Applications after installing; on Linux, run `sudo apt remove perch` or delete the Perch AppImage. If Perch started at login there, also delete `~/Library/LaunchAgents/Perch.plist` (macOS) or `~/.config/autostart/Perch.desktop` (Linux)."]

Devlings checks GitHub for updates at launch and once a day, and installs signed updates in-app; this can be turned off in Settings → About.

## Downloads

| System | File |
|---|---|
| Windows 10/11 | `Devlings_[X.Y.Z]_x64-setup.exe` |
| macOS (Apple Silicon and Intel) | `Devlings_[X.Y.Z]_universal.dmg` |
| Linux (AppImage) | `Devlings_[X.Y.Z]_amd64.AppImage` |
| Linux (.deb) | `Devlings_[X.Y.Z]_amd64.deb` |

Each installer is also attached under a name without the version (`Devlings-windows-x64-setup.exe`, `Devlings-macos-universal.dmg`, `Devlings-linux-x86_64.AppImage`, `Devlings-linux-amd64.deb`), which is what the README's download links point at. They're byte-for-byte copies, with the same checksum.

[While the repository is private there's no macOS build: delete its row above and its name from that list.]

## Checksums and provenance

Each file's SHA-256 is in `SHA256SUMS.txt`, attached to this release. Every asset also has a GitHub build-provenance attestation, generated in CI from this exact source at this tag. Attestations are tied to a file's digest, not its name, so a version-free copy verifies exactly like its original:

```
gh attestation verify <downloaded-file> --repo ixklo/devlings
```

Files from v1.0.0 and earlier verify with `--repo yeetstick/perch`, the project's earlier address.

[Attestations exist only for releases built while the repository is public (release.yml skips them for a private repository). For a release built while it was private, delete this whole section except the `SHA256SUMS.txt` sentence.]

## Known limits

- **macOS and Linux are beta:** builds and launches in CI, not yet tested on real hardware. [Update this line once real-hardware QA has happened.]
- **Windows 10** is untested (Windows 11 is verified).
- Installers aren't notarized (macOS) or backed by an EV certificate (Windows); see the README's Install section for the SmartScreen/Gatekeeper steps.

[Delete this line and list any other limits specific to this release, or remove the bullet entirely once notarization/signing changes.]
