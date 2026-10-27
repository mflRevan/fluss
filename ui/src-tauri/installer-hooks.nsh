; Fluss installer hooks (Tauri NSIS).

!macro NSIS_HOOK_PREINSTALL
  ; Updating: stop the running background app so its files can be replaced.
  nsExec::Exec 'taskkill /F /IM fluss-daemon.exe'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Put the `fluss` CLI on the user's PATH.
  nsExec::Exec `powershell -NoProfile -ExecutionPolicy Bypass -Command "$$p=[Environment]::GetEnvironmentVariable('Path','User'); if (($$p -split ';') -notcontains '$INSTDIR') { [Environment]::SetEnvironmentVariable('Path', ($$p.TrimEnd(';') + ';$INSTDIR'), 'User') }"`
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'taskkill /F /IM fluss-daemon.exe'
  nsExec::Exec 'taskkill /F /IM fluss-settings.exe'
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Fluss"
  nsExec::Exec `powershell -NoProfile -ExecutionPolicy Bypass -Command "$$p=[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::SetEnvironmentVariable('Path', (($$p -split ';' | Where-Object { $$_ -and $$_ -ne '$INSTDIR' }) -join ';'), 'User')"`
!macroend
