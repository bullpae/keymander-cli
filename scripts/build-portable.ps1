# ============================================================
# keymander portable bundle build script (Windows)
# ============================================================
#
# Usage:
#   .\scripts\build-portable.ps1
#   .\scripts\build-portable.ps1 -OutputDir "C:\output"

param(
    [string]$OutputDir = "."
)

$ErrorActionPreference = "Stop"

$ROOT = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = $PSScriptRoot
}

Push-Location $ROOT
try {
    $version = (Select-String -Path "Cargo.toml" -Pattern 'version = "([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
    Write-Host "=== keymander portable bundle v$version ===" -ForegroundColor Cyan

    Write-Host "`n[1/4] Building binaries..." -ForegroundColor Yellow
    cargo build --release -p keymander
    if ($LASTEXITCODE -ne 0) { throw "kmd build failed" }
    cargo build --release -p kmd-desktop
    if ($LASTEXITCODE -ne 0) { throw "kmd-desktop build failed" }
    cargo build --release -p kmd-daemon
    if ($LASTEXITCODE -ne 0) { throw "kmd-daemon build failed" }

    Write-Host "`n[2/4] Staging portable bundle..." -ForegroundColor Yellow
    $stage = Join-Path $env:TEMP "keymander-portable-stage\keymander"
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Path "$stage\kmd-data" -Force | Out-Null

    Copy-Item "target\release\kmd.exe" $stage
    Copy-Item "target\release\kmd-desktop.exe" $stage
    Copy-Item "target\release\kmd-daemon.exe" $stage
    $assemble = Join-Path $ROOT "scripts\assemble-config.ps1"
    & $assemble -Platform windows -OutFile (Join-Path $stage "kmd-data\config.toml")
    Copy-Item "dist\README.txt" $stage

    Write-Host "`n[3/4] Creating ZIP..." -ForegroundColor Yellow
    $zipName = "keymander-portable-v$version-windows-x64.zip"
    $zipPath = Join-Path (Resolve-Path $OutputDir) $zipName
    if (Test-Path $zipPath) { Remove-Item $zipPath }

    $stageParent = Split-Path $stage -Parent
    Compress-Archive -Path "$stageParent\keymander" -DestinationPath $zipPath -CompressionLevel Optimal

    Remove-Item -Recurse -Force $stageParent

    $size = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
    Write-Host "`n[4/4] Done!" -ForegroundColor Green
    Write-Host "  File: $zipPath" -ForegroundColor White
    Write-Host "  Size: ${size}MB" -ForegroundColor White
    Write-Host ""
    Write-Host "Usage:" -ForegroundColor Cyan
    Write-Host "  1. Extract the ZIP to any location"
    Write-Host "  2. Run kmd-desktop.exe from the keymander folder"
    Write-Host ""
}
finally {
    Pop-Location
}
