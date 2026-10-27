# Builds the Windows installer: release binaries + settings app + NSIS bundle.
# Output: target\release\bundle\nsis\Fluss_<version>_x64-setup.exe
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

cargo build --release -p fluss-daemon -p fluss-cli
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$triple = "x86_64-pc-windows-msvc"
$bin = "ui\src-tauri\binaries"
New-Item -ItemType Directory -Force $bin | Out-Null
Copy-Item target\release\fluss-daemon.exe "$bin\fluss-daemon-$triple.exe" -Force
Copy-Item target\release\fluss.exe "$bin\fluss-$triple.exe" -Force

Set-Location ui
npm ci
npm run build
npx tauri build --config src-tauri/tauri.bundle.conf.json
if ($LASTEXITCODE) { exit $LASTEXITCODE }
Get-ChildItem "$root\target\release\bundle\nsis\*.exe" | ForEach-Object { "built $($_.FullName) ($([int]($_.Length / 1MB)) MB)" }
