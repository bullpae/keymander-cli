#!/bin/sh
# bullpae/homebrew-tap의 Formula/keymander.rb를 생성한다.
# 사용법: gen-homebrew-formula.sh <version> <sha_arm_mac> <sha_x86_mac> <sha_x86_linux>
# 릴리스 워크플로우(update-tap 잡)가 릴리스마다 호출한다.
set -eu

VERSION="$1"
SHA_ARM_MAC="$2"
SHA_X86_MAC="$3"
SHA_X86_LINUX="$4"

# 저장소 이름을 하드코딩하지 않는다 — 저장소를 개명하면 formula가 죽은 URL을
# 가리키게 되고, 그 사실이 사용자의 `brew install` 실패로만 드러난다.
# CI에서는 GITHUB_REPOSITORY, 로컬에서는 origin 리모트에서 유도한다.
REPO="${GITHUB_REPOSITORY:-$(git remote get-url origin 2>/dev/null |
    sed -E 's#(git@github\.com:|https://github\.com/)##; s#\.git$##')}"
: "${REPO:?저장소를 알 수 없습니다 — GITHUB_REPOSITORY를 지정하거나 git 저장소 안에서 실행하세요}"

HOMEPAGE="https://github.com/${REPO}"
BASE="${HOMEPAGE}/releases/download/v${VERSION}"

cat <<EOF
# 자동 생성 파일 — 직접 수정하지 마세요.
# ${REPO}의 scripts/gen-homebrew-formula.sh가 릴리스마다 갱신합니다.
class Keymander < Formula
  desc "Keyboard-driven cross-platform launcher (TUI + desktop + key-remap daemon)"
  homepage "${HOMEPAGE}"
  version "${VERSION}"
  license "MIT"

  on_macos do
    on_arm do
      url "${BASE}/keymander-portable-aarch64-apple-darwin.tar.gz"
      sha256 "${SHA_ARM_MAC}"
    end
    on_intel do
      url "${BASE}/keymander-portable-x86_64-apple-darwin.tar.gz"
      sha256 "${SHA_X86_MAC}"
    end
  end

  on_linux do
    on_intel do
      url "${BASE}/keymander-portable-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${SHA_X86_LINUX}"
    end
  end

  def install
    bin.install "kmd", "kmd-desktop", "kmd-daemon"
    pkgshare.install "kmd-data/config.toml" => "config.example.toml"
  end

  def caveats
    config_dir = OS.mac? ? "~/Library/Application Support/kmd" : "~/.config/kmd"
    <<~TEXT
      기본 설정으로 바로 동작합니다. 번들 예시 설정에서 시작하려면:
        mkdir -p "#{config_dir}"
        cp "#{opt_pkgshare}/config.example.toml" "#{config_dir}/config.toml"

      키 리맵 데몬을 쓰려면: kmd daemon start
      macOS에서는 시스템 설정 → 개인정보 보호 및 보안에서
      손쉬운 사용/입력 모니터링 권한을 kmd-daemon에 허용해야 합니다.
    TEXT
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/kmd --version")
  end
end
EOF
