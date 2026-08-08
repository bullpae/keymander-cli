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
    pkill -f kmd-desktop 2>/dev/null || true

    # 로그는 런타임 디렉터리(OS 표준)로 — kmd daemon status가 보여주는 경로와 일치
    case "$(uname -s)" in
        Darwin) RUNTIME_DIR="$HOME/Library/Application Support/kmd" ;;
        *)      RUNTIME_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/kmd" ;;
    esac
    mkdir -p "$RUNTIME_DIR" "$DEPLOY_DIR/kmd-data"

    # macOS: LaunchAgent가 설치돼 있으면 반드시 launchd로 재시작한다.
    # nohup으로 이 스크립트(터미널) 밑에서 띄우면 TCC가 접근성 권한을 데몬이
    # 아니라 부모(터미널/셸) 기준으로 귀속시켜, kmd-daemon을 허용해도
    # AXIsProcessTrusted=false가 되고 키 훅이 죽는다. launchd(PID 1)가 띄우면
    # 귀속이 깨끗하다. (kmd daemon install 로 LaunchAgent 등록)
    local agent="$HOME/Library/LaunchAgents/com.keymander.daemon.plist"
    if [ "$(uname -s)" = "Darwin" ] && [ -f "$agent" ]; then
        info "데몬 재시작 중 (launchd)..."
        launchctl kickstart -k "gui/$(id -u)/com.keymander.daemon" 2>/dev/null \
            || { launchctl bootstrap "gui/$(id -u)" "$agent" 2>/dev/null || true; }
    else
        info "데몬 시작 중..."
        if [ "$(uname -s)" = "Darwin" ]; then
            warn "LaunchAgent 미설치 — nohup으로 시작합니다."
            warn "이 경우 TCC 접근성 귀속이 터미널 기준이 되어 키 훅이 안 붙을 수 있습니다."
            echo "    권장: kmd daemon install (로그인 자동시작 + 깨끗한 권한 귀속)"
        fi
        pkill -f kmd-daemon 2>/dev/null || true
        sleep 1
        nohup "$DEPLOY_DIR/kmd-daemon" > "$RUNTIME_DIR/daemon.log" 2>&1 &
    fi
    sleep 2

    if pgrep -f kmd-daemon > /dev/null; then
        ok "kmd-daemon 실행 중 (PID: $(pgrep -f kmd-daemon | head -1))"
    else
        fail "kmd-daemon 시작 실패"
    fi

    # macOS: 접근성 권한이 없으면 데몬은 멀쩡히 뜨지만 CGEventTap 설치가 실패해
    # Alt+Space 와 모든 레이어가 통째로 죽는다. 데몬 자신이 보고하는 현재 상태로
    # 판정한다 — 로그 grep은 누적된 과거 실패 줄에 걸려 오탐이 난다.
    # (kmd status 출력에 "키 훅" 라인이 있으면 = hook_error 존재 = 미동작)
    if [ "$(uname -s)" = "Darwin" ]; then
        sleep 1
        if "$DEPLOY_DIR/kmd" daemon status 2>/dev/null | grep -q "키 훅"; then
            echo ""
            warn "접근성(손쉬운 사용) 권한이 없어 키보드 훅이 설치되지 않았습니다."
            warn "Alt+Space 와 nav/mouse 레이어가 동작하지 않습니다."
            echo "    시스템 설정 > 개인 정보 보호 및 보안 > 손쉬운 사용 에서"
            echo "    기존 kmd-daemon 항목을 제거(-)한 뒤 아래 경로를 다시 추가하세요:"
            echo "      $DEPLOY_DIR/kmd-daemon"
            echo "    (Finder 열기 후 ⌘⇧G 로 경로 붙여넣기)"
            echo "    추가한 뒤: ./scripts/deploy-local.sh --restart"
            echo ""
        fi
    fi
}

# macOS 코드 서명 — 재배포 시 접근성 권한 유지용.
#
# 링커의 ad-hoc 서명은 identifier 와 cdhash 가 바이너리 내용에서 파생되므로
# 빌드할 때마다 바뀐다. TCC 는 ad-hoc 바이너리를 identifier+cdhash 로 매칭하기
# 때문에, 재배포하면 손쉬운 사용 허용이 조용히 무효화되고 키 훅이 죽는다.
#
# 서명 키는 로그인 키체인이 아니라 **전용 키체인**(~/.keymander-release/)에 둔다.
# 로그인 키체인의 키는 codesign 이 쓸 때마다 암호 프롬프트를 띄우는데, 비대화
# 세션에서는 응답할 수 없어 실패하고 사용자 화면에는 뜬금없는 서명 요청 창만
# 뜬다 (2026-08-08 사고 원인 중 하나). 전용 키체인은 암호 파일(0600)로
# 스크립트가 직접 잠금해제하므로 프롬프트가 없다. 구성 절차: docs/07 §7
KMD_KEYCHAIN="$HOME/.keymander-release/kmd-codesign.keychain-db"
KMD_KEYCHAIN_PASS_FILE="$HOME/.keymander-release/keychain-pass"

codesign_binaries() {
    [ "$(uname -s)" = "Darwin" ] || return 0

    local sign_id="${KMD_CODESIGN_ID:-}"
    if [ -z "$sign_id" ] && [ -f "$KMD_KEYCHAIN" ] && [ -f "$KMD_KEYCHAIN_PASS_FILE" ]; then
        sign_id="keymander-local-codesign"
    fi
    if [ "$sign_id" != "" ] && [ "$sign_id" != "-" ] && [ -f "$KMD_KEYCHAIN_PASS_FILE" ]; then
        # 잠금해제 실패 시 서명이 프롬프트를 띄울 수 있으므로 ad-hoc 으로 강등.
        # (프롬프트를 띄우지 않는 것이 서명 유지보다 우선이다)
        if ! security unlock-keychain \
                -p "$(cat "$KMD_KEYCHAIN_PASS_FILE")" "$KMD_KEYCHAIN" 2>/dev/null; then
            warn "전용 키체인 잠금해제 실패 — ad-hoc 서명으로 진행"
            sign_id="-"
        fi
    fi
    : "${sign_id:=-}"

    # .new(새 inode)를 서명한다 — rename 전에. 제자리 재서명 금지 (위 주석 참조).
    local bin
    for bin in kmd kmd-daemon kmd-desktop; do
        if ! codesign --force --sign "$sign_id" \
                --identifier "com.keymander.$bin" \
                "$DEPLOY_DIR/$bin.new" 2>/dev/null; then
            warn "codesign 실패: $bin"
        fi
    done

    if [ "$sign_id" = "-" ]; then
        warn "ad-hoc 서명 — 이번 배포로 접근성 권한이 초기화될 수 있습니다."
        echo "    안정 서명 구성: docs/07_distribution.md §7 (전용 키체인)"
    else
        ok "코드 서명 완료 ($sign_id, 전용 키체인)"
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

# 새 inode 에 배치한 뒤 서명하고 rename 으로 교체한다. 이미 실행된 적 있는
# 파일을 제자리에서 덮어쓰거나 재서명하면 커널의 vnode 서명 캐시가 낡아,
# 다음 실행이 OS_REASON_CODESIGNING 으로 즉사한다 (2026-08-08 실사고 —
# launchd가 데몬을 기동 직후 kill). rename 은 원자적이고 실행 중 프로세스의
# 매핑(기존 inode)도 건드리지 않는다.
for bin in kmd kmd-desktop kmd-daemon; do
    cp "$TARGET_DIR/$bin" "$DEPLOY_DIR/$bin.new"
    chmod +x "$DEPLOY_DIR/$bin.new"
done
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
codesign_binaries
for bin in kmd kmd-desktop kmd-daemon; do
    mv -f "$DEPLOY_DIR/$bin.new" "$DEPLOY_DIR/$bin"
done

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
