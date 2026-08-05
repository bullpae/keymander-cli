#!/usr/bin/env bash
# ============================================================
# keymander 로컬 배포 스크립트
# ============================================================
#
# 빌드 → 테스트 → ~/keymander/ 배포 → 데몬 재시작
#
# 사용법:
#   ./scripts/deploy-local.sh          # 전체 (빌드+테스트+배포+재시작)
#   ./scripts/deploy-local.sh --skip-test   # 테스트 생략
#   ./scripts/deploy-local.sh --restart     # 재시작만

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEPLOY_DIR="$HOME/keymander"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}▸${NC} $1"; }
ok()    { echo -e "${GREEN}✓${NC} $1"; }
warn()  { echo -e "${YELLOW}!${NC} $1"; }
fail()  { echo -e "${RED}✗${NC} $1"; exit 1; }

SKIP_TEST=false
RESTART_ONLY=false

FAST=false

usage() {
    echo "사용법:"
    echo "  ./scripts/deploy-local.sh              # 전체 (빌드+테스트+배포+재시작)"
    echo "  ./scripts/deploy-local.sh --skip-test  # 테스트 생략"
    echo "  ./scripts/deploy-local.sh --restart    # 재시작만"
    echo "  ./scripts/deploy-local.sh --fast       # 빠른 빌드 (LTO off — 로컬 검증용)"
    echo ""
    echo "  --fast 는 실행 동작이 release 와 같지만 LTO 를 꺼서 빌드가 크게 빨라진다."
    echo "  바이너리가 조금 커지므로 배포 자산은 CI 가 release 로 만든다."
}

for arg in "$@"; do
    case "$arg" in
        --skip-test) SKIP_TEST=true ;;
        --restart)   RESTART_ONLY=true ;;
        --fast)      FAST=true ;;
        -h|--help|help) usage; exit 0 ;;
        *) usage; fail "알 수 없는 인자: $arg" ;;
    esac
done

# 빌드 프로파일 — --fast 는 LTO 를 끈 fast 프로파일 (Cargo.toml [profile.fast])
# `--profile <이름>` 형태로 통일한다 (`--profile release` == `--release`).
if [ "$FAST" = true ]; then
    PROFILE="fast"
else
    PROFILE="release"
fi
PROFILE_FLAG="--profile $PROFILE"
TARGET_DIR="target/$PROFILE"

restart_daemon() {
    info "데몬/데스크톱 프로세스 종료 중..."
    pkill -f kmd-daemon 2>/dev/null || true
    pkill -f kmd-desktop 2>/dev/null || true
    sleep 1

    info "데몬 시작 중..."
    mkdir -p "$DEPLOY_DIR/kmd-data"
    # 로그는 런타임 디렉터리(OS 표준)로 — kmd daemon status가 보여주는 경로와 일치
    case "$(uname -s)" in
        Darwin) RUNTIME_DIR="$HOME/Library/Application Support/kmd" ;;
        *)      RUNTIME_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/kmd" ;;
    esac
    mkdir -p "$RUNTIME_DIR"
    nohup "$DEPLOY_DIR/kmd-daemon" > "$RUNTIME_DIR/daemon.log" 2>&1 &
    sleep 1

    if pgrep -f kmd-daemon > /dev/null; then
        ok "kmd-daemon 실행 중 (PID: $(pgrep -f kmd-daemon | head -1))"
    else
        fail "kmd-daemon 시작 실패"
    fi
}

if $RESTART_ONLY; then
    echo ""
    echo -e "${CYAN}=== keymander 재시작 ===${NC}"
    echo ""
    restart_daemon
    echo ""
    ok "완료"
    exit 0
fi

cd "$ROOT"

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo ""
echo -e "${CYAN}=== keymander v${VERSION} 로컬 배포 ===${NC}"
echo ""

# [1] 빌드
if [ "$FAST" = true ]; then
    info "[1/4] 빠른 빌드 (fast 프로파일 — LTO off)..."
else
    info "[1/4] 릴리스 빌드..."
fi
# shellcheck disable=SC2086  # PROFILE_FLAG 는 의도적으로 분리되어야 한다
cargo build $PROFILE_FLAG --workspace 2>&1 | tail -3
ok "빌드 완료"

# [2] 테스트
if $SKIP_TEST; then
    warn "[2/4] 테스트 생략 (--skip-test)"
else
    info "[2/4] 테스트 실행..."
    if cargo test --workspace 2>&1 | tail -3 | grep -q "test result: ok"; then
        ok "모든 테스트 통과"
    else
        fail "테스트 실패 — 배포 중단"
    fi
fi

# [3] 배포
info "[3/4] $DEPLOY_DIR 에 배포 중..."
mkdir -p "$DEPLOY_DIR" "$DEPLOY_DIR/kmd-data"

cp "$TARGET_DIR/kmd" "$DEPLOY_DIR/"
cp "$TARGET_DIR/kmd-desktop" "$DEPLOY_DIR/"
cp "$TARGET_DIR/kmd-daemon" "$DEPLOY_DIR/"
if [ ! -f "$DEPLOY_DIR/kmd-data/config.toml" ]; then
    case "$(uname -s)" in
        Darwin) CONFIG_PLATFORM=macos ;;
        Linux)  CONFIG_PLATFORM=linux ;;
        *)      CONFIG_PLATFORM=linux ;;
    esac
    "$SCRIPT_DIR/assemble-config.sh" "$CONFIG_PLATFORM" "$DEPLOY_DIR/kmd-data/config.toml"
else
    info "기존 config.toml 유지 (덮어쓰기 안 함)"
fi
chmod +x "$DEPLOY_DIR/kmd" "$DEPLOY_DIR/kmd-desktop" "$DEPLOY_DIR/kmd-daemon"

ok "바이너리 배포 완료"

# [4] 재시작
info "[4/4] 서비스 재시작..."
restart_daemon

echo ""
echo -e "${GREEN}=== 배포 완료 ===${NC}"
echo "  버전:  v${VERSION}"
echo "  프로파일: ${PROFILE}$([ "$FAST" = true ] && echo '  (LTO off — 로컬 검증용)')"
echo "  경로:  $DEPLOY_DIR"
echo "  데몬:  PID $(pgrep -f kmd-daemon | head -1)"
echo "  실행:  Alt+Space 로 kmd-desktop 실행"
echo ""
