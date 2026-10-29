# Sets the version everywhere it lives, then shows how to release it.
#   powershell -File scripts\bump-version.ps1 0.2.0
# The workspace Cargo.toml is the source of truth: the daemon, CLI and settings app
# read it at build time (tauri.conf.json has no version, so Tauri uses Cargo's).
param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$') { throw "not a semver version: $Version" }

# Only the first `version = ` (the [workspace.package] one) changes.
$toml = Get-Content Cargo.toml -Raw
$re = [regex]'(?m)^version = "[^"]+"'
if (-not $re.IsMatch($toml)) { throw "no workspace version in Cargo.toml" }
[IO.File]::WriteAllText("$PWD\Cargo.toml", $re.Replace($toml, "version = `"$Version`"", 1))

Push-Location ui
npm version $Version --no-git-tag-version --allow-same-version | Out-Null
Pop-Location

cargo update -w -q
if ($LASTEXITCODE) { exit $LASTEXITCODE }

Write-Host "version is now $Version (Cargo.toml, Cargo.lock, ui/package.json)" -ForegroundColor DarkYellow
Write-Host "release:  git commit -am `"Release $Version`"; git tag v$Version; git push origin main v$Version"
