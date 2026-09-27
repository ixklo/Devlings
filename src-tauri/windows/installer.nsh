; Perch's NSIS installer hooks (tauri.conf.json > bundle > windows > nsis > installerHooks).

; A real uninstall removes Perch's entries from Claude Code's settings.json and Perch's start-at-login entry,
; so nothing is left pointing at a program that is gone. Updates keep both: the updater runs the installer with
; /UPDATE, and in that mode Tauri's template skips the uninstaller, or forwards /UPDATE ($UpdateMode = 1) if it runs it.
; perch.exe is a GUI program and does its cleanup without windows. It exits 0 on success and 1 if settings.json
; couldn't be updated; the uninstall carries on either way.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-hooks'
  ${EndIf}
!macroend
