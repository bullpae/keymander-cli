#!/usr/bin/env bash
# 코드 규모 스냅샷 — 리팩토링 백로그(docs/11)의 "대형 파일" 항목용.
#
# 백로그에 줄 수를 손으로 적으면 며칠 만에 낡는다(실제로 app.rs가 2449줄로
# 적혀 있는 동안 2644줄이 됐다). 판단이 필요할 때 이 스크립트를 돌린다.
#
#   ./scripts/code-metrics.sh              # 상위 15개 파일 + 테스트 수
#   ./scripts/code-metrics.sh 30           # 상위 30개
set -euo pipefail
cd "$(dirname "$0")/.."

TOP="${1:-15}"

echo "== 소스 파일 규모 (상위 $TOP) =="
find crates src -name '*.rs' -not -path '*/target/*' -print0 \
  | xargs -0 wc -l \
  | sort -rn \
  | grep -v ' total$' \
  | head -n "$TOP" \
  | awk '{ printf "  %6d  %s\n", $1, $2 }'

echo
echo "== 가장 긴 함수 (상위 10, 근사치 — 다음 fn까지의 거리라 사이 항목이 섞일 수 있다) =="
find crates src -name '*.rs' -not -path '*/target/*' -print0 \
  | xargs -0 awk '
      /^[[:space:]]*(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?(unsafe[[:space:]]+)?fn / {
          if (name != "") print NR - start, name, FILENAME
          name = $0; start = NR
      }
      END { if (name != "") print NR - start, name, FILENAME }' \
  | sort -rn | head -10 \
  | sed 's/^\([0-9]*\) */  \1줄  /'

echo
echo "== 테스트 수 =="
for scope in crates/kmd-core crates/kmd-daemon crates/kmd-desktop src; do
  n=$(grep -rn '#\[test\]\|#\[tokio::test\]' --include='*.rs' "$scope" 2>/dev/null | wc -l | tr -d ' ')
  printf "  %-22s %s\n" "$scope" "$n"
done
