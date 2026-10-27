[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    & cargo test --workspace
    if ($LASTEXITCODE -ne 0) { throw "host tests failed" }

    # Type-check the kernel crate without linking; linking needs the WDK.
    & cargo check -p cmflt-driver
    if ($LASTEXITCODE -ne 0) { throw "driver type-check failed" }

    Write-Host "OK: host tests + driver type-check" -ForegroundColor Green
}
finally {
    Pop-Location
}
