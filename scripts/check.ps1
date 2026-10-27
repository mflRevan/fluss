# Local pre-push check: the same tests CI runs, plus the UI build.
#   powershell -File scripts\check.ps1           # tests + checks + UI build
#   powershell -File scripts\check.ps1 -Quick    # Rust tests only
#   powershell -File scripts\install-hooks.ps1   # run -Quick automatically before every push
param([switch]$Quick)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

function Step($name, [scriptblock]$cmd) {
    $t = Get-Date
    Write-Host "-> $name" -ForegroundColor DarkYellow
    & $cmd
    if ($LASTEXITCODE) { Write-Host "FAILED: $name" -ForegroundColor Red; exit $LASTEXITCODE }
    Write-Host ("   ok ({0:n1}s)" -f ((Get-Date) - $t).TotalSeconds) -ForegroundColor DarkGray
}

Step "core tests" { cargo test -q -p fluss-core }
Step "daemon tests" { cargo test -q -p fluss-daemon }
if ($Quick) { exit 0 }
Step "cli + settings check" { cargo check -q -p fluss-cli -p fluss-settings }
Step "ui build" { Push-Location ui; npm run build --silent; $c = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $c }
Write-Host "all checks passed" -ForegroundColor DarkYellow
