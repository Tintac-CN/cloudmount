[CmdletBinding()]
param(
    [ValidateSet("release", "debug")]
    [string]$Configuration = "release",
    [switch]$SkipVerify
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $cargoArgs = @("build", "-p", "cmflt-driver", "--target", "x86_64-pc-windows-msvc")
    if ($Configuration -eq "release") { $cargoArgs += "--release" }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $dll = Join-Path $root "target\x86_64-pc-windows-msvc\$Configuration\cmflt_driver.dll"
    if (-not (Test-Path $dll)) { throw "driver artifact not found: $dll" }

    $outDir = Join-Path $root "out"
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    $sys = Join-Path $outDir "cloudmount.sys"
    Copy-Item $dll $sys -Force

    $pdb = [IO.Path]::ChangeExtension($dll, ".pdb")
    if (Test-Path $pdb) {
        Copy-Item $pdb (Join-Path $outDir "cloudmount.pdb") -Force
    }

    Write-Host "built $sys" -ForegroundColor Green
    if (-not $SkipVerify) {
        & (Join-Path $PSScriptRoot "verify-driver.ps1") -Path $sys
        if ($LASTEXITCODE -ne 0) { throw "driver verification failed" }
    }
}
finally {
    Pop-Location
}
