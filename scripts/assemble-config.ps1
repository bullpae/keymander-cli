# ============================================================
# dist/config.shared.toml + dist/config.keymap.<platform>.toml
# → 단일 config.toml (UTF-8, BOM 없음)
# ============================================================
#
# 사용 예:
#   .\scripts\assemble-config.ps1 -Platform windows -OutFile "C:\path\kmd-data\config.toml"

param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("windows", "macos", "linux")]
    [string]$Platform,

    [Parameter(Mandatory = $true)]
    [string]$OutFile
)

$ErrorActionPreference = "Stop"

$ROOT = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = $PSScriptRoot
}

$shared = Join-Path $ROOT "dist\config.shared.toml"
$keymap = Join-Path $ROOT "dist\config.keymap.$Platform.toml"

if (-not (Test-Path $shared)) { throw "Missing: $shared" }
if (-not (Test-Path $keymap)) { throw "Missing: $keymap" }

$utf8 = New-Object System.Text.UTF8Encoding $false
$body = [System.IO.File]::ReadAllText($shared, [System.Text.Encoding]::UTF8).TrimEnd() + "`r`n`r`n" +
        [System.IO.File]::ReadAllText($keymap, [System.Text.Encoding]::UTF8).TrimEnd() + "`r`n"

$fullOut = if ([System.IO.Path]::IsPathRooted($OutFile)) {
    $OutFile
} else {
    Join-Path (Get-Location).Path $OutFile
}

$parent = Split-Path -Parent $fullOut
if ($parent -and -not (Test-Path $parent)) {
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
}

[System.IO.File]::WriteAllText($fullOut, $body, $utf8)
Write-Host "Wrote $fullOut (platform=$Platform)"
