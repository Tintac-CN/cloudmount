[CmdletBinding()]
param(
    [string]$Path = (Join-Path (Split-Path -Parent $PSScriptRoot) "out\cloudmount.sys"),
    [switch]$SkipFpScan
)

$ErrorActionPreference = "Stop"

function Find-Dumpbin {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vsPath) {
            $tools = Get-ChildItem (Join-Path $vsPath "VC\Tools\MSVC") -Directory -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending | Select-Object -First 1
            if ($tools) {
                $candidate = Join-Path $tools.FullName "bin\Hostx64\x64\dumpbin.exe"
                if (Test-Path $candidate) { return $candidate }
            }
        }
    }
    $command = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    throw "dumpbin.exe not found (install Visual Studio C++ build tools)"
}

if (-not (Test-Path $Path)) { throw "driver image not found: $Path" }
$dumpbin = Find-Dumpbin
$failures = New-Object System.Collections.Generic.List[string]

$headerText = (& $dumpbin /nologo /headers $Path | Out-String)
if ($LASTEXITCODE -ne 0) { throw "dumpbin /headers failed" }

if ($headerText -notmatch "8664 machine \(x64\)") { $failures.Add("not an x64 PE image") }
if ($headerText -notmatch "subsystem \(Native\)") { $failures.Add("subsystem is not Native") }
# Resolve the entry point and prove it is the exported DriverEntry. dumpbin
# sometimes omits the entry symbol name for .sys files, so compare RVAs from
# the header and the export table instead of relying on the suffix.
$entryMatch = [regex]::Match($headerText, '(?im)^\s*([0-9A-F]+)\s+entry point\s+\(([0-9A-F]+)\)')
if (-not $entryMatch.Success) {
    $failures.Add("no entry point found")
} else {
    $entryRva = [Convert]::ToInt64($entryMatch.Groups[1].Value, 16)
    $exportText = (& $dumpbin /nologo /exports $Path | Out-String)
    if ($LASTEXITCODE -ne 0) { throw "dumpbin /exports failed" }
    $driverMatch = [regex]::Match($exportText, '(?im)^\s*\d+\s+\d+\s+([0-9A-F]+)\s+DriverEntry\b')
    if (-not $driverMatch.Success) {
        $failures.Add("DriverEntry is not exported")
    } else {
        $driverRva = [Convert]::ToInt64($driverMatch.Groups[1].Value, 16)
        if ($driverRva -ne $entryRva) {
            $failures.Add(("entry point RVA 0x{0:X} is not DriverEntry RVA 0x{1:X}" -f $entryRva, $driverRva))
        }
    }
}
if ($headerText -notmatch "DLL") { $failures.Add("image is not marked as a DLL") }

# Reject a section that is simultaneously executable and writable (HVCI).
$sectionPattern = '(?ms)^\s*([\.\w]+) name\s*$.*?(?=^\s*[\.\w]+ name\s*$|\z)'
foreach ($match in [regex]::Matches($headerText, $sectionPattern)) {
    $name = $match.Groups[1].Value
    $block = $match.Value
    if ($block -match '(?i)\bExecute\b' -and $block -match '(?i)\bWrite\b') {
        $failures.Add("section $name is both executable and writable")
    }
}

$importText = (& $dumpbin /nologo /imports $Path | Out-String)
if ($LASTEXITCODE -ne 0) { throw "dumpbin /imports failed" }
$allowedImports = @("FLTMGR.SYS", "NTOSKRNL.EXE", "HAL.DLL", "WMILIB.SYS")
$imports = @(
    [regex]::Matches($importText, '(?m)^\s{4}([A-Za-z0-9_\-\.]+\.(?:SYS|sys|EXE|exe|DLL|dll))\s*$') |
        ForEach-Object { $_.Groups[1].Value.ToUpperInvariant() } |
        Sort-Object -Unique
)
foreach ($import in $imports) {
    if ($allowedImports -notcontains $import) {
        $failures.Add("unexpected import: $import")
    }
}
if ($imports -notcontains "FLTMGR.SYS") { $failures.Add("missing FLTMGR.SYS import") }

if (-not $SkipFpScan) {
    $disasmText = (& $dumpbin /nologo /disasm $Path | Out-String)
    if ($LASTEXITCODE -ne 0) { throw "dumpbin /disasm failed" }
    if ($disasmText -match '(?i)\b(?:xmm|ymm|zmm)\d+') {
        $failures.Add("FP/SSE/AVX register found in disassembly")
    }
}

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { Write-Host "FAIL: $_" -ForegroundColor Red }
    exit 1
}

$size = (Get-Item $Path).Length
Write-Host "PASS: $Path" -ForegroundColor Green
Write-Host ("  size={0} bytes; imports={1}" -f $size, ($imports -join ", "))
Write-Host "  x64 / Native / DriverEntry / no RWX / no FP-SSE"
