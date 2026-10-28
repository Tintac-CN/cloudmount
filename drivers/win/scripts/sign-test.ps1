[CmdletBinding()]
param(
    [string]$SysPath = "",
    [string]$Subject = "CloudMount Test Driver"
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($SysPath)) {
    $SysPath = Join-Path (Split-Path -Parent $PSScriptRoot) "out\cloudmount.sys"
}
if (-not (Test-Path $SysPath)) { throw "driver image not found: $SysPath" }

$wdkBin = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin\10.0.19041.0\x64"
$signtool = Join-Path $wdkBin "signtool.exe"
if (-not (Test-Path $signtool)) {
    $found = Get-ChildItem (Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin") -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } | Select-Object -First 1
    if ($found) { $signtool = $found.FullName } else { throw "signtool.exe not found (install the WDK)" }
}

$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq "CN=$Subject" -and $_.HasPrivateKey } |
    Select-Object -First 1
if (-not $cert) {
    Write-Host "creating self-signed code-signing certificate: CN=$Subject"
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=$Subject" `
        -CertStoreLocation "Cert:\CurrentUser\My" -HashAlgorithm SHA256 `
        -NotAfter (Get-Date).AddYears(2)
}

$outDir = Split-Path -Parent $SysPath
$cerPath = Join-Path $outDir "cloudmount-test.cer"
Export-Certificate -Cert $cert -FilePath $cerPath -Force | Out-Null

# Trust the test certificate for kernel test-signing. These stores require an
# elevated shell; the caller is expected to run this from an admin prompt.
Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\LocalMachine\Root" | Out-Null
Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\LocalMachine\TrustedPublisher" | Out-Null

& $signtool sign /fd SHA256 /s My /n $Subject $SysPath
if ($LASTEXITCODE -ne 0) { throw "signtool failed" }

& $signtool verify /pa /v $SysPath
if ($LASTEXITCODE -ne 0) { throw "signtool verify failed" }

Write-Host "signed $SysPath" -ForegroundColor Green
Write-Host "To load on a test machine:"
Write-Host "  bcdedit /set testsigning on   # reboot required"
Write-Host "  InfDefaultInstall.exe <path>\cloudmount.inf"
Write-Host "  fltmc load cloudmount"
Write-Host "  fltmc filters"
Write-Host "  fltmc unload cloudmount"
