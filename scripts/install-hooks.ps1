# Installs a git pre-push hook that runs the quick local check (Rust tests).
Set-Location (Split-Path -Parent $PSScriptRoot)
$hook = ".git/hooks/pre-push"
@'
#!/bin/sh
# Fluss: quick local check before pushing (skip with `git push --no-verify`).
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1 -Quick
'@ | Set-Content -NoNewline -Encoding ascii $hook
Write-Host "installed $hook"
