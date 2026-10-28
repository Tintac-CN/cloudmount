[CmdletBinding()]
param(
    [int]$Count = 100
)

$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "preflight-load.ps1")
if ($LASTEXITCODE -ne 0) { throw "preflight failed; see BLOCKED lines above" }

$timer = [Diagnostics.Stopwatch]::StartNew()
for ($i = 1; $i -le $Count; $i++) {
    & fltmc.exe load cloudmount
    if ($LASTEXITCODE -ne 0) { throw "load failed at iteration $i (exit $LASTEXITCODE)" }
    & fltmc.exe unload cloudmount
    if ($LASTEXITCODE -ne 0) { throw "unload failed at iteration $i (exit $LASTEXITCODE)" }
    if (($i % 10) -eq 0) { Write-Host "G1: $i/$Count load-unload cycles" }
}
$timer.Stop()
Write-Host ("G1 PASS: {0} cycles in {1:N1}s" -f $Count, $timer.Elapsed.TotalSeconds) -ForegroundColor Green
