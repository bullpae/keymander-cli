#!/bin/bash
# brew로 설치된 keymander 바이너리를 고정 경로(~/.keymander/bin)로 동기화한다.
#
# 목적: macOS TCC 접근성 권한은 "실행 파일 경로+서명" 기준이므로, brew 업그레이드로
# Cellar 경로가 바뀔 때마다 손쉬운 사용 재부여가 필요해진다. 데몬을 항상 이 고정
# 경로 + 안정 서명(전용 키체인, docs/07 §7)으로 실행하면 권한이 영구 유지된다.
#
# 사용: ~/.keymander/sync.sh 로 복사해 두고, com.keymander.sync LaunchAgent
# (WatchPaths=/opt/homebrew/opt/keymander)가 brew 업그레이드 직후 자동 실행한다.
# 수동 실행도 안전하다 (동기화된 버전과 같으면 no-op).
set -euo pipefail

SRC="/opt/homebrew/opt/keymander/bin"
DST="$HOME/.keymander/bin"
KEYCHAIN="$HOME/.keymander-release/kmd-codesign.keychain-db"
PASS_FILE="$HOME/.keymander-release/keychain-pass"
PLIST="$HOME/Library/LaunchAgents/com.keymander.daemon.plist"
LOG="$HOME/.keymander/sync.log"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >>"$LOG"; }

mkdir -p "$DST"

# brew 미설치/제거 시 no-op
[ -x "$SRC/kmd-daemon" ] || { log "source 없음 — skip"; exit 0; }

# 버전 마커: Cellar 실경로가 같으면 이미 동기화된 것
resolved="$(readlink -f "$SRC/kmd-daemon")"
marker="$DST/.synced-from"
if [ -f "$marker" ] && [ "$(cat "$marker")" = "$resolved" ]; then
    log "이미 동기화됨 ($resolved) — skip"
    exit 0
fi

log "동기화 시작: $resolved"

# 안정 서명 준비 (잠금해제 실패 시 ad-hoc 강등 — 프롬프트 방지가 서명 유지보다 우선)
SIGN_ID="keymander-local-codesign"
if [ ! -f "$PASS_FILE" ] || ! security unlock-keychain \
        -p "$(cat "$PASS_FILE")" "$KEYCHAIN" 2>/dev/null; then
    log "경고: 전용 키체인 잠금해제 실패 — ad-hoc 서명 (접근성 풀릴 수 있음)"
    SIGN_ID="-"
fi

# 새 inode에 서명 후 rename 교체 (제자리 재서명 금지 — vnode 서명 캐시 함정, docs/07)
for bin in kmd kmd-daemon kmd-desktop; do
    cp "$SRC/$bin" "$DST/$bin.new"
    if [ "$SIGN_ID" = "-" ]; then
        codesign --force --sign - --identifier "com.keymander.$bin" "$DST/$bin.new"
    else
        codesign --force --sign "$SIGN_ID" --identifier "com.keymander.$bin" \
            --keychain "$KEYCHAIN" "$DST/$bin.new"
    fi
    mv -f "$DST/$bin.new" "$DST/$bin"
done
echo "$resolved" >"$marker"

# 바이너리 교체 후 재시작은 반드시 bootout→bootstrap
# (launchd가 서명 신원을 pin하므로 kickstart는 OS_REASON_CODESIGNING으로 즉사)
if [ -f "$PLIST" ]; then
    launchctl bootout "gui/$(id -u)/com.keymander.daemon" 2>/dev/null || true
    sleep 1
    launchctl bootstrap "gui/$(id -u)" "$PLIST"
    log "데몬 재시작 완료"
fi

log "동기화 완료: $resolved"
