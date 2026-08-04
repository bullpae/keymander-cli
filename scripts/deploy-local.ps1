# UTF-8 BOM 권장: Windows PowerShell 5.x는 BOM 없으면 이 파일을 cp949로 읽어 한글이 깨짐.
# Cursor에서 저장 시 'UTF-8 with BOM'으로 저장하거나, 저장소의 파일은 BOM이 붙어 있어야 함.

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
#   .\scripts\deploy-local.ps1 -Help          # 도움말 (--help 도 인식)

param(
    [switch]$SkipTest,
    [switch]$RestartOnly,
    [string]$DeployDir = "C:\WinUtil\keymander",
    [switch]$Help
)

$ErrorActionPreference = "Stop"

# ── 콘솔 인코딩 (한글/특수문자 깨짐 방지) ───────────────────
# PowerShell 5.x + Cursor·기본 cmd 터미널에서 UTF-8 스크립트를 실행할 때
# [Console]::OutputEncoding 이 cp949 등이면 Write-Host 출력이 MOJIBAKE 로 보임.
try {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    [Console]::InputEncoding = [System.Text.Encoding]::UTF8
    $OutputEncoding = [System.Text.Encoding]::UTF8
    chcp 65001 | Out-Null
} catch { }

# ── 프로젝트 루트 탐색 ──────────────────────────────────────
$ROOT = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path "$ROOT\Cargo.toml")) {
    $ROOT = $PSScriptRoot
}

# ── 컬러 헬퍼 ───────────────────────────────────────────────
function Write-Info  { param([string]$Msg) Write-Host "[.] $Msg" -ForegroundColor Cyan }
function Write-Ok    { param([string]$Msg) Write-Host "[ok] $Msg" -ForegroundColor Green }
function Write-Warn  { param([string]$Msg) Write-Host "[!] $Msg" -ForegroundColor Yellow }
function Write-Fail  { param([string]$Msg) Write-Host "[x] $Msg" -ForegroundColor Red; exit 1 }

function Show-Usage {
    Write-Host ""
    Write-Host "keymander Windows 로컬 배포 스크립트" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "사용법:"
    Write-Host "  .\scripts\deploy-local.ps1                            # 전체 (빌드+테스트+배포+재시작)"
    Write-Host "  .\scripts\deploy-local.ps1 -SkipTest                  # 테스트 생략"
    Write-Host "  .\scripts\deploy-local.ps1 -RestartOnly               # 재시작만"
    Write-Host "  .\scripts\deploy-local.ps1 -DeployDir 'D:\other\path' # 배포 경로 변경"
    Write-Host "  .\scripts\deploy-local.ps1 -Help                      # 이 도움말"
    Write-Host ""
}

# ── 인자 검증 ───────────────────────────────────────────────
# PowerShell은 "--help" 같은 이중 대시 토큰을 옵션이 아니라 위치 인자로
# 해석해 $DeployDir="--help"가 되고, 그대로 진행하면 리포 옆에 '--help'
# 폴더를 만들어 배포해버리는 사고가 난다. 도움말 요청은 사용법을 보여주고,
# 옵션처럼 생긴 값·상대 경로는 배포 경로로 거부한다.
if ($Help -or $DeployDir -match '^(-{1,2}h(elp)?|/\?|help)$') {
    Show-Usage
    exit 0
}
if ($DeployDir.StartsWith("-")) {
    Write-Fail "잘못된 배포 경로: '$DeployDir' — 옵션은 -SkipTest, -RestartOnly, -DeployDir <경로>, -Help 입니다"
}
if (-not [System.IO.Path]::IsPathRooted($DeployDir)) {
    Write-Fail "배포 경로는 절대 경로여야 합니다: '$DeployDir'"
}

# ── 프로세스 종료/시작 ────────────────────────────────────────
function Stop-KmdProcesses {
    Write-Info "데몬/데스크톱 프로세스 종료 중..."
    Get-Process -Name "kmd-daemon"  -ErrorAction SilentlyContinue | Stop-Process -Force
    Get-Process -Name "kmd-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Seconds 1
}

function Start-Daemon {
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
    Stop-KmdProcesses
    Start-Daemon
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
    # release 프로필로 실행해 [1] 산출물을 재사용한다.
    # debug 재빌드는 target 용량·PDB(LNK1318) 부담이 커서 Windows 로컬에서 자주 실패한다.
    if ($SkipTest) {
        Write-Warn "[2/4] 테스트 생략 (-SkipTest)"
    } else {
        Write-Info "[2/4] 테스트 실행 (release)..."
        cargo test --workspace --release
        if ($LASTEXITCODE -ne 0) { Write-Fail "테스트 실패 — 배포 중단" }
        Write-Ok "모든 테스트 통과"
    }

    # [3] 프로세스 종료 → 배포
    Write-Info "[3/4] $DeployDir 에 배포 중..."
    Stop-KmdProcesses

    $dataDir = Join-Path $DeployDir "kmd-data"
    if (-not (Test-Path $DeployDir)) { New-Item -ItemType Directory -Path $DeployDir -Force | Out-Null }
    if (-not (Test-Path $dataDir))   { New-Item -ItemType Directory -Path $dataDir -Force | Out-Null }

    Copy-Item "target\release\kmd.exe"         $DeployDir -Force
    Copy-Item "target\release\kmd-desktop.exe" $DeployDir -Force
    Copy-Item "target\release\kmd-daemon.exe"  $DeployDir -Force

    $configDest = Join-Path $dataDir "config.toml"
    if (-not (Test-Path $configDest)) {
        $assemble = Join-Path $ROOT "scripts\assemble-config.ps1"
        & $assemble -Platform windows -OutFile $configDest
    } else {
        Write-Info "기존 config.toml 유지 (덮어쓰기 안 함)"
    }

    Write-Ok "바이너리 배포 완료"

    # [4] 데몬 시작
    Write-Info "[4/4] 서비스 시작..."
    Start-Daemon

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
