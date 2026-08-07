#!/usr/bin/env bash
# ============================================================
# 저장소 비밀정보 스캔
# ============================================================
#
# 공개 저장소라 한 번 커밋되면 지워도 이미 노출이다 — 폐기·교체 외엔 되돌릴 수
# 없다. GitHub push protection이 공급자 토큰(ghp_ 등)은 막아 주지만, **개인키
# 블록 같은 비공급자 패턴은 저장소 설정에서 따로 켜야** 잡힌다. 그 사각지대를
# 이 스크립트가 CI에서 결정적으로 막는다.
#
# 사용법:
#   scripts/scan-secrets.sh            # 추적 중인 파일만 (빠름, CI 기본)
#   scripts/scan-secrets.sh --history  # 전체 git 이력의 모든 blob (느림)
#
# 종료 코드: 0 = 깨끗, 1 = 의심 항목 발견

set -euo pipefail

MODE="${1:-tracked}"

# 이 파일 자신과 검증용 공개키는 패턴을 포함할 수밖에 없으므로 제외한다.
# (공개키는 배포하라고 있는 것이라 노출이 정상이다)
is_allowed() {
    case "$1" in
        scripts/scan-secrets.sh) return 0 ;;
        dist/keymander-archive-keyring.asc) return 0 ;;
        *) return 1 ;;
    esac
}

# PGP "PRIVATE KEY BLOCK"과 PEM 개인키를 모두 잡는다.
PATTERNS='-----BEGIN (RSA |OPENSSH |EC |DSA |PGP |ENCRYPTED )?PRIVATE KEY|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}'

found=0

scan_tracked() {
    local f
    while IFS= read -r f; do
        is_allowed "$f" && continue
        [ -f "$f" ] || continue
        # 패턴이 '-----'로 시작하므로 반드시 -e 로 넘긴다. 그냥 넘기면 grep이
        # 옵션으로 파싱해 아무것도 못 잡고 조용히 통과한다(실제로 겪음).
        if LC_ALL=C grep -qIE -e "$PATTERNS" "$f" 2>/dev/null; then
            echo "✗ 비밀정보 의심: $f"
            LC_ALL=C grep -nIE -e "$PATTERNS" "$f" 2>/dev/null | head -3 | sed 's/^/    /'
            found=1
        fi
    done < <(git ls-files)
}

scan_history() {
    # 삭제된 파일도 이력에 남아 있으면 여전히 노출이다 — blob 전수 검사.
    python3 - "$PATTERNS" <<'PY'
import re, subprocess, sys
pat = re.compile(sys.argv[1].encode())
allowed = {b'scripts/scan-secrets.sh', b'dist/keymander-archive-keyring.asc'}
out = subprocess.run(['git','rev-list','--objects','--all'],
                     capture_output=True, text=True).stdout
entries = [(p[0], p[1]) for p in (l.split(' ', 1) for l in out.splitlines()) if len(p) == 2]
hits, BATCH = {}, 400
for i in range(0, len(entries), BATCH):
    chunk = entries[i:i+BATCH]
    payload = ('\n'.join(s for s, _ in chunk) + '\n').encode()
    data = subprocess.run(['git','cat-file','--batch'],
                          input=payload, capture_output=True).stdout
    pos = 0
    for sha, path in chunk:
        nl = data.find(b'\n', pos)
        if nl < 0:
            break
        h = data[pos:nl].split()
        if len(h) < 3:
            pos = nl + 1
            continue
        size = int(h[2]); body = data[nl+1:nl+1+size]; pos = nl+1+size+1
        if path.encode() in allowed:
            continue
        if pat.search(body):
            hits.setdefault(path, set()).add(sha[:8])
for path, shas in sorted(hits.items()):
    print(f"✗ 이력에 비밀정보 의심: {path}  blobs={sorted(shas)}")
print(f"(오브젝트 {len(entries)}개 검사)", file=sys.stderr)
sys.exit(1 if hits else 0)
PY
}

if [ "$MODE" = "--history" ]; then
    scan_history || found=1
else
    scan_tracked
fi

if [ "$found" -ne 0 ]; then
    echo ""
    echo "공개 저장소이므로 커밋된 시점에 이미 노출된 것으로 간주해야 합니다."
    echo "파일만 지우지 말고 해당 자격증명을 **폐기·교체**하세요."
    exit 1
fi

echo "✓ 비밀정보 패턴 없음"
