; Fluss installer hooks (Tauri NSIS, per-user install, no admin rights).
;
; Files outside $INSTDIR that Fluss owns:
;   %APPDATA%\Fluss       settings (config.toml) and the dictionary (rules.json)
;   %LOCALAPPDATA%\Fluss  models, engine runtimes, logs, update downloads, backups
;   HKCU\...\Run "Fluss"  start at sign-in (written by the app itself)
;   user PATH             $INSTDIR, so `fluss` works in a terminal
;
; Modes: a normal install, an in-app update (`/P /UPDATE`: passive, keeps shortcuts),
; and the uninstaller, which Tauri also runs with /UPDATE when replacing a version.

Var FlussWasRunning

; Stop Fluss so its files can be replaced. Ask nicely first (the daemon finishes
; writing settings and stops its engine servers), then make sure nothing is left.
!macro FLUSS_STOP
  StrCpy $FlussWasRunning 0
  nsExec::Exec 'cmd /c tasklist /FI "IMAGENAME eq fluss-daemon.exe" /NH | find /I "fluss-daemon.exe"'
  Pop $0
  ${If} $0 = 0
    StrCpy $FlussWasRunning 1
    ${If} ${FileExists} "$INSTDIR\fluss.exe"
      ; Bounded: a daemon that doesn't answer is killed below instead of hanging setup.
      nsExec::Exec /TIMEOUT=10000 '"$INSTDIR\fluss.exe" stop'
      Pop $0
    ${EndIf}
  ${EndIf}
  nsExec::Exec 'taskkill /F /IM fluss-daemon.exe'
  Pop $0
  nsExec::Exec 'taskkill /F /IM fluss-settings.exe'
  Pop $0
  ; A `fluss logs -f` left open in a terminal would lock fluss.exe.
  nsExec::Exec 'taskkill /F /IM fluss.exe'
  Pop $0
  ; Give Windows a moment to release the file handles of killed processes.
  Sleep 400
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro FLUSS_STOP
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Shortcuts and the taskbar keep showing a cached icon after an update; refresh it.
  nsExec::Exec 'ie4uinit.exe -show'
  Pop $0
  ; Put the `fluss` CLI on the user's PATH (handles quotes and %VARS% in PATH safely).
  nsExec::Exec '"$INSTDIR\fluss.exe" setup path-add'
  Pop $0
  ; Updating or reinstalling a Fluss that was set up: bring it back in the tray
  ; without opening a window. It also restores the sign-in entry, which the old
  ; uninstaller removes when the user picks "uninstall before installing".
  ; A first install starts from the finish page instead, which opens onboarding.
  ${If} $FlussWasRunning = 1
  ${OrIf} $UpdateMode = 1
  ${OrIf} ${FileExists} "$APPDATA\Fluss\config.toml"
    Exec '"$INSTDIR\fluss-daemon.exe" --background'
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro FLUSS_STOP
  ; Replacing this version with another one keeps PATH (the new install needs it anyway).
  ${If} $UpdateMode <> 1
    nsExec::Exec '"$INSTDIR\fluss.exe" setup path-remove'
    Pop $0
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    ; Downloaded update installers are never worth keeping.
    RMDir /r "$LOCALAPPDATA\Fluss\updates"
    ; "Delete the application data" (unchecked by default). Tauri only clears its own
    ; WebView data; Fluss keeps its settings and models under its own name.
    ${If} $DeleteAppDataCheckboxState = 1
      RMDir /r "$APPDATA\Fluss"
      RMDir /r "$LOCALAPPDATA\Fluss"
    ${EndIf}
  ${EndIf}
!macroend
