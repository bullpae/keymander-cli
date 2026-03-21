# ============================================================
# keymander Windows 로컬 배포 스크립트
# ============================================================
#
# 빌드 → 테스트 → C:\WinUtil\keymander 배포 → 프로세스 재시작
#
# 사용법:
#   .\scripts\deploy-local.ps1                # 전체 (빌드+테스트+배포+재시작)
#   .\scripts\deploy-local.ps1 -SkipTest      # 테스트 생략
#   .\scripts\deploy-local.ps1 -RestartOnly   # 재시작만
#   .\scripts\deploy-local.ps1 -DeployDir "D:\other\path"  # 배포 경로 변경

param(
    [switch]$SkipTest,
    [switch]$RestartOnly,
    [string]$DeployDir = "C:\WinUtil\keymander"
)

$ErrorActionPreference = "Stop"

# ── 프로젝트 루트 탐색 ──────────────────────────────────────
$ROOT = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = $PSScriptRoot
}

# ── 컬러 헬퍼 ───────────────────────────────────────────────
function Write-Info  { param([string]$Msg) Write-Host "▸ $Msg" -ForegroundColor Cyan }
function Write-Ok    { param([string]$Msg) Write-Host "✓ $Msg" -ForegroundColor Green }
function Write-Warn  { param([string]$Msg) Write-Host "! $Msg" -ForegroundColor Yellow }
function Write-Fail  { param([string]$Msg) Write-Host "✗ $Msg" -ForegroundColor Red; exit 1 }

# ── 프로세스 재시작 ─────────────────────────────────────────
function Restart-Daemon {
    Write-Info "데몬/데스크톱 프로세스 종료 중..."
    Get-Process -Name "kmd-daemon"  -ErrorAction SilentlyContinue | Stop-Process -Force
    Get-Process -Name "kmd-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 1

    Write-Info "데몬 시작 중..."
    $daemonExe = Join-Path $DeployDir "kmd-daemon.exe"
    if (-not (Test-Path $daemonExe)) {
        Write-Fail "kmd-daemon.exe 를 찾을 수 없습니다: $daemonExe"
    }
    Start-Process -FilePath $daemonExe -WindowStyle Hidden
    Start-Sleep -Seconds 1

    $proc = Get-Process -Name "kmd-daemon" -ErrorAction SilentlyContinue
    if ($proc) {
        Write-Ok "kmd-daemon 실행 중 (PID: $($proc.Id))"
    } else {
        Write-Fail "kmd-daemon 시작 실패"
    }
}

# ── --RestartOnly 모드 ──────────────────────────────────────
if ($RestartOnly) {
    Write-Host ""
    Write-Host "=== keymander 재시작 ===" -ForegroundColor Cyan
    Write-Host ""
    Restart-Daemon
    Write-Host ""
    Write-Ok "완료"
    exit 0
}

# ── 전체 배포 플로우 ────────────────────────────────────────
Push-Location $ROOT
try {
    $version = (Select-String -Path "Cargo.toml" -Pattern 'version = "([^"]+)"' |
                Select-Object -First 1).Matches.Groups[1].Value

    Write-Host ""
    Write-Host "=== keymander v$version Windows 로컬 배포 ===" -ForegroundColor Cyan
    Write-Host ""

    # [1] 빌드
    Write-Info "[1/4] 릴리스 빌드..."
    cargo build --release --workspace
    if ($LASTEXITCODE -ne 0) { Write-Fail "빌드 실패" }
    Write-Ok "빌드 완료"

    # [2] 테스트
    if ($SkipTest) {
        Write-Warn "[2/4] 테스트 생략 (-SkipTest)"
    } else {
        Write-Info "[2/4] 테스트 실행..."
        cargo test --workspace
        if ($LASTEXITCODE -ne 0) { Write-Fail "테스트 실패 — 배포 중단" }
        Write-Ok "모든 테스트 통과"
    }

    # [3] 배포
    Write-Info "[3/4] $DeployDir 에 배포 중..."
    $dataDir = Join-Path $DeployDir "kmd-data"
    if (-not (Test-Path $DeployDir)) { New-Item -ItemType Directory -Path $DeployDir -Force | Out-Null }
    if (-not (Test-Path $dataDir))   { New-Item -ItemType Directory -Path $dataDir -Force | Out-Null }

    Copy-Item "target\release\kmd.exe"         $DeployDir -Force
    Copy-Item "target\release\kmd-desktop.exe" $DeployDir -Force
    Copy-Item "target\release\kmd-daemon.exe"  $DeployDir -Force

    $configDest = Join-Path $dataDir "config.toml"
    if (-not (Test-Path $configDest)) {
        Copy-Item "dist\config.toml" $dataDir -Force
    } else {
        Write-Info "기존 config.toml 유지 (덮어쓰기 안 함)"
    }

    Write-Ok "바이너리 배포 완료"

    # [4] 재시작
    Write-Info "[4/4] 서비스 재시작..."
    Restart-Daemon

    Write-Host ""
    Write-Host "=== 배포 완료 ===" -ForegroundColor Green
    Write-Host "  버전:  v$version"
    Write-Host "  경로:  $DeployDir"
    $daemon = Get-Process -Name "kmd-daemon" -ErrorAction SilentlyContinue
    if ($daemon) {
        Write-Host "  데몬:  PID $($daemon.Id)"
    }
    Write-Host "  실행:  Alt+Space 로 kmd-desktop 실행"
    Write-Host ""
}
finally {
    Pop-Location
}
