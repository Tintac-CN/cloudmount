[CmdletBinding()]
param(
    [switch]$SkipClient
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$inf = Join-Path $root "cloudmount.inf"
$client = Join-Path $root "target\debug\cmflt-client.exe"

& (Join-Path $PSScriptRoot "preflight-load.ps1")
if ($LASTEXITCODE -ne 0) { throw "preflight failed; see BLOCKED lines above" }

if (-not (Get-Service -Name cloudmount -ErrorAction SilentlyContinue)) {
    $installer = Join-Path $env:SystemRoot "System32\InfDefaultInstall.exe"
    & $installer $inf
    if ($LASTEXITCODE -ne 0) { throw "InfDefaultInstall failed with exit code $LASTEXITCODE" }
}

& fltmc.exe load cloudmount
if ($LASTEXITCODE -ne 0) { throw "fltmc load cloudmount failed with exit code $LASTEXITCODE" }
& fltmc.exe filters

if (-not $SkipClient) {
    if (-not (Test-Path $client)) {
        Push-Location $root
        try { & cargo build -p cmflt-client } finally { Pop-Location }
    }
    & $client handshake
    if ($LASTEXITCODE -ne 0) {
        & fltmc.exe unload cloudmount
        throw "cmflt-client handshake failed with exit code $LASTEXITCODE"
    }
    Write-Host "placeholder smoke: $client create-placeholder <new-file> --size 4096"
}

Write-Host "driver loaded; unload with: fltmc unload cloudmount"
