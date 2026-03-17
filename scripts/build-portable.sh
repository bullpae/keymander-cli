#!/usr/bin/env bash
# ============================================================
# keymander 포터블 번들 빌드 스크립트 (macOS / Linux)
# ============================================================
#
# 사용법:
#   ./scripts/build-portable.sh
#   ./scripts/build-portable.sh /output/dir
#
# 결과:
#   keymander-portable-v{version}-{os}-{arch}.tar.gz

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="${1:-.}"

cd "$ROOT"

# 버전 읽기
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

echo "=== keymander portable bundle v${VERSION} ==="

# 릴리스 빌드
echo ""
echo "[1/4] 바이너리 빌드 중..."
cargo build --release -p keymander
cargo build --release -p kmd-desktop
cargo build --release -p kmd-daemon

# 스테이징 디렉토리 구성
echo ""
echo "[2/4] 포터블 번들 구성 중..."
STAGE=$(mktemp -d)/keymander
mkdir -p "$STAGE/kmd-data"

cp target/release/kmd "$STAGE/"
cp target/release/kmd-desktop "$STAGE/"
cp target/release/kmd-daemon "$STAGE/"
cp dist/config.toml "$STAGE/kmd-data/"
cp dist/README.txt "$STAGE/"
chmod +x "$STAGE/kmd" "$STAGE/kmd-desktop" "$STAGE/kmd-daemon"

# tar.gz 생성
echo ""
echo "[3/4] tar.gz 패키징 중..."
TARNAME="keymander-portable-v${VERSION}-${OS}-${ARCH}.tar.gz"
TARPATH="$(cd "$OUTPUT_DIR" && pwd)/$TARNAME"
STAGE_PARENT="$(dirname "$STAGE")"
tar czf "$TARPATH" -C "$STAGE_PARENT" keymander

# 정리
rm -rf "$STAGE_PARENT"

# 결과 출력
SIZE=$(du -h "$TARPATH" | cut -f1)
echo ""
echo "[4/4] 완료!"
echo "  파일: $TARPATH"
echo "  크기: $SIZE"
echo ""
echo "사용 방법:"
echo "  1. tar.gz를 원하는 위치에 풀기: tar xzf $TARNAME"
echo "  2. keymander/kmd-desktop 실행"
echo ""
