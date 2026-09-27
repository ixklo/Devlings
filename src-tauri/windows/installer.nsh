; Devlings' NSIS installer hooks (tauri.conf.json > bundle > windows > nsis > installerHooks).
; Tauri's installer template (tauri-bundler's installer.nsi) expands these macros inside its own sections, so they can
; use its variables ($UpdateMode, $PassiveMode), defines (${MANUFACTURER}, ${PRODUCTNAME}), macros and functions.

; ---------------------------------------------------------------------------------------------------------------------
; Upgrading from Perch, the app's name before 1.1 (design: docs/specs/2026-09-27-devlings-v1.1.md)
;
; The product name changed, so the installer treats Devlings as a new app, with a new folder (%LOCALAPPDATA%\Devlings)
; and a new uninstall entry. The app identifier didn't change, so settings, projects, logs and the WebView2 profile
; (the app-data folders named after it) carry over untouched. Before installing, an existing Perch install is removed:
;   1. Found through its uninstall entry (HKCU: Tauri's default per-user install), else the install folder Tauri saved
;      under HKCU\Software\<manufacturer>\Perch, else the default %LOCALAPPDATA%\Perch. It counts only if its
;      uninstall.exe is there.
;   2. A running perch.exe is closed: the installer's own check only looks for devlings.exe. A normal install asks
;      first; a passive or silent one (an update) doesn't.
;   3. Perch's uninstaller runs silently in update mode (/S /UPDATE), so Perch 1.0's own hook skips removing Claude
;      Code's hooks and the app data is kept. `_?=` runs it in place, so ExecWait waits for it; its uninstall.exe and
;      the then-empty folder are removed afterwards.
;   4. What update mode leaves behind goes too: Perch's start-at-login values (HKCU Run and StartupApproved\Run, both
;      named "Perch"), its Start menu and desktop shortcuts (only ones that point at the removed perch.exe) and the
;      install folder Tauri saved.
; The app then finishes the move on its first launch: the hooks' relay command still runs Perch's perch.exe, which
; counts as outdated, so it's rewritten once (with a backup) to run devlings.exe; and start-at-login, if it was on,
; is restored under the new name.
; ---------------------------------------------------------------------------------------------------------------------

!define PERCH_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Perch"
!define PERCH_RUNKEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define PERCH_STARTUPAPPROVEDKEY "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"

Var PerchDir
Var PerchFound
Var PerchRemoved
Var PerchHadDesktopShortcut

!macro NSIS_HOOK_PREINSTALL
  StrCpy $PerchFound 0
  StrCpy $PerchRemoved 0
  StrCpy $PerchHadDesktopShortcut 0

  ; 1. Find it. Tauri saves InstallLocation in quotes.
  ReadRegStr $PerchDir HKCU "${PERCH_UNINSTKEY}" "InstallLocation"
  StrCpy $R9 $PerchDir 1
  ${If} $R9 == '"'
    StrCpy $PerchDir $PerchDir -1 1
  ${EndIf}
  ${If} $PerchDir == ""
  ${OrIfNot} ${FileExists} "$PerchDir\uninstall.exe"
    ReadRegStr $PerchDir HKCU "Software\${MANUFACTURER}\Perch" ""
  ${EndIf}
  ${If} $PerchDir == ""
  ${OrIfNot} ${FileExists} "$PerchDir\uninstall.exe"
    StrCpy $PerchDir "$LOCALAPPDATA\Perch"
  ${EndIf}

  ${If} ${FileExists} "$PerchDir\uninstall.exe"
    StrCpy $PerchFound 1
    DetailPrint "Found Perch, the earlier name of Devlings, in $PerchDir. Removing it; your settings and hooks stay."

    ; 2. Close it.
    nsis_tauri_utils::FindProcessCurrentUser "perch.exe"
    Pop $R9
    ${If} $R9 = 0
      ${IfNot} ${Silent}
      ${AndIf} $PassiveMode <> 1
        ${If} ${Cmd} `MessageBox MB_OKCANCEL|MB_ICONINFORMATION "Perch, the earlier version of Devlings, is running. Click OK to close it and continue." IDCANCEL`
          Abort "Perch is still running, so Devlings wasn't installed."
        ${EndIf}
      ${EndIf}
      nsis_tauri_utils::KillProcessCurrentUser "perch.exe"
      Pop $R9
      Sleep 500
    ${EndIf}

    ; 3. Uninstall it in update mode, and wait.
    ClearErrors
    ExecWait '"$PerchDir\uninstall.exe" /S /UPDATE _?=$PerchDir' $R9
    ${If} ${Errors}
      StrCpy $R9 -1
    ${EndIf}
    ${If} $R9 = 0
    ${AndIfNot} ${FileExists} "$PerchDir\perch.exe"
      StrCpy $PerchRemoved 1
      Delete "$PerchDir\uninstall.exe"
      RMDir "$PerchDir"
      DeleteRegKey HKCU "Software\${MANUFACTURER}\Perch"
    ${Else}
      DetailPrint "Couldn't remove Perch (uninstaller result: $R9)."
      ${IfNot} ${Silent}
      ${AndIf} $PassiveMode <> 1
        MessageBox MB_OK|MB_ICONEXCLAMATION "Devlings couldn't remove Perch, its earlier version. When this install finishes, uninstall Perch in Windows Settings > Apps, then open Devlings and check Settings > Watching."
      ${EndIf}
    ${EndIf}

    ; 4. Clean up what update mode keeps. Start-at-login goes either way, so Perch and Devlings never both start.
    DeleteRegValue HKCU "${PERCH_RUNKEY}" "Perch"
    DeleteRegValue HKCU "${PERCH_STARTUPAPPROVEDKEY}" "Perch"
    ${If} $PerchRemoved = 1
      !insertmacro IsShortcutTarget "$SMPROGRAMS\Perch.lnk" "$PerchDir\perch.exe"
      Pop $R9
      ${If} $R9 = 1
        !insertmacro UnpinShortcut "$SMPROGRAMS\Perch.lnk"
        Delete "$SMPROGRAMS\Perch.lnk"
      ${EndIf}
      !insertmacro IsShortcutTarget "$DESKTOP\Perch.lnk" "$PerchDir\perch.exe"
      Pop $R9
      ${If} $R9 = 1
        StrCpy $PerchHadDesktopShortcut 1
        !insertmacro UnpinShortcut "$DESKTOP\Perch.lnk"
        Delete "$DESKTOP\Perch.lnk"
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

; An update-mode install (Perch 1.0's updater installing Devlings) doesn't create shortcuts, because an update normally
; keeps the old ones. Perch's are gone, so Devlings makes its own with the template's functions: the Start menu one,
; which also registers the app ID Windows notifications use, and the desktop one if Perch had one. /NS still means none.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $PerchRemoved = 1
  ${AndIf} $UpdateMode = 1
    StrCpy $UpdateMode 0
    Call CreateOrUpdateStartMenuShortcut
    ${If} $PerchHadDesktopShortcut = 1
      Call CreateOrUpdateDesktopShortcut
    ${EndIf}
    StrCpy $UpdateMode 1
  ${EndIf}
!macroend

; A real uninstall removes Devlings' entries from Claude Code's settings.json and its start-at-login entry, so nothing
; is left pointing at a program that is gone. Updates keep both: the updater runs the installer with /UPDATE, and in
; that mode Tauri's template skips the uninstaller, or forwards /UPDATE ($UpdateMode = 1) if it runs it.
; devlings.exe is a GUI program and does its cleanup without windows. It exits 0 on success and 1 if settings.json
; couldn't be updated; the uninstall carries on either way.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-hooks'
  ${EndIf}
!macroend
