[CmdletBinding()]
param(
    [string]$SysPath = "",
    [switch]$SkipSignature
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($SysPath)) {
    $SysPath = Join-Path $root "out\cloudmount.sys"
}
$issues = New-Object System.Collections.Generic.List[string]

$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $issues.Add("current shell is not elevated")
}

try {
    if (Confirm-SecureBootUEFI) {
        $issues.Add("Secure Boot is enabled; testsigning is blocked by firmware policy. Disable Secure Boot in UEFI/BIOS (or use a VM with Secure Boot off) and reboot.")
    }
} catch {
    Write-Host "Secure Boot query unavailable (legacy BIOS or unsupported platform); continuing."
}

$bcd = (& bcdedit.exe /enum "{current}" 2>&1 | Out-String)
if ($bcd -notmatch "testsigning\s+Yes") {
    $issues.Add("testsigning is not active for this boot; run 'bcdedit /set testsigning on' and reboot after Secure Boot is disabled.")
}

if (-not (Test-Path $SysPath)) {
    $issues.Add("driver image not found: $SysPath")
} elseif (-not $SkipSignature) {
    $signtool = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin\10.0.19041.0\x64\signtool.exe"
    if (-not (Test-Path $signtool)) {
        $found = Get-ChildItem (Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin") -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\' } | Select-Object -First 1
        if ($found) { $signtool = $found.FullName }
    }
    if (-not $signtool) {
        $issues.Add("signtool.exe not found")
    } else {
        & $signtool verify /pa $SysPath *> $null
        if ($LASTEXITCODE -ne 0) {
            $issues.Add("driver is not signed with a trusted test certificate (run scripts/sign-test.ps1)")
        }
    }
}

if ($issues.Count -gt 0) {
    $issues | ForEach-Object { Write-Host "BLOCKED: $_" -ForegroundColor Red }
    exit 1
}

Write-Host "preflight OK: elevated, Secure Boot off, testsigning active, driver signed" -ForegroundColor Green
Write-Host "next: InfDefaultInstall.exe $root\cloudmount.inf"
Write-Host "      fltmc load cloudmount"
Write-Host "      cmflt-client handshake"
