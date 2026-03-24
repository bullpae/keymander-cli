#!/usr/bin/env bash
# dist/config.shared.toml + dist/config.keymap.<platform>.toml → stdout 또는 파일
#
# 사용 예:
#   ./scripts/assemble-config.sh windows kmd-data/config.toml
#   ./scripts/assemble-config.sh macos  /tmp/config.toml

set -euo pipefail

PLATFORM="${1:?usage: assemble-config.sh <windows|macos|linux> <outfile>}"
OUT="${2:?usage: assemble-config.sh <windows|macos|linux> <outfile>}"

case "$PLATFORM" in
  windows|macos|linux) ;;
  *) echo "unknown platform: $PLATFORM" >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

mkdir -p "$(dirname "$OUT")"
cat dist/config.shared.toml "dist/config.keymap.${PLATFORM}.toml" > "$OUT"
echo "Wrote $OUT (platform=$PLATFORM)"
