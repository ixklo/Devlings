# Release checklist

Run on a Windows machine with Claude Code logged in with a subscription. Start from a clean state:
quit Perch, delete `%APPDATA%\io.github.perchpet.perch\`, and make sure `~/.claude/settings.json` has no Perch hooks.

- [ ] 1. Fresh install → onboarding: name the pet, install hooks, three green checks. `settings.json` has 7 Perch http hooks and a `settings.json.perch-backup-*` exists.
- [ ] 2. Prompt in the VS Code extension → the pet goes working → done, the bubble shows steps, and a notification appears with the pet name.
- [ ] 3. Trigger a permission prompt in VS Code (Manual mode, ask for a shell command) → the pet shows the "!" badge and bounces (needs you).
- [ ] 4. Quit Perch, then use Claude Code normally → no noticeable delay and no hook errors in the session.
- [ ] 5. Ask flow: pick a project, send, reply streams; a follow-up remembers context; Stop shows "Stopped"; in Read only mode, asking for a file edit shows "Blocked: Write".
- [ ] 6. Point Settings at a missing binary (Choose Claude Code file… → any non-claude file) → setup state with guidance; Auto-detect recovers.
- [ ] 6a. Set `ANTHROPIC_API_KEY=sk-test` in the environment, start Perch from that shell, and Ask → the run still succeeds on the subscription (the key is scrubbed). Then `claude auth login --console` (API login) → Ask is refused with the subscription message; log back in with `claude auth login`.
- [ ] 6b. Rename the pet in Settings → the new name shows in the bubble, the panel, the tray tooltip, and the next notification.
- [ ] 7. Settings → Remove hooks → only Perch entries removed; diff `settings.json` against the backup.
- [ ] 8. Occupy the hook port (`python -m http.server <port> --bind 127.0.0.1`), restart Perch → "Move to a new port" appears and fixes it.
- [ ] 9. Check Anthropic's current Claude Code terms and usage docs for changes affecting tools that launch the local `claude` binary; update the README Cost section if needed.
