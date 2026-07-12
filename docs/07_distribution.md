# 배포 채널 셋업 가이드

keymander를 winget / Homebrew / .deb / .rpm으로 배포하기 위해 구축한 파이프라인의
**남은 수동 작업**과, **다른 프로젝트에 동일하게 적용하기 위한 체크리스트**.

작성: 2026-07-12 (v0.9.2 기준)

---

## 1. 현재 상태 요약

| 채널 | 상태 | 남은 일 |
|---|---|---|
| GitHub Releases + SHA256SUMS | ✅ 자동화 완료 | 없음 |
| Homebrew (`brew install bullpae/tap/keymander`) | ✅ 설치 가능 | 자동 갱신용 시크릿 등록 (§2.2) |
| winget | 🔶 최초 등록 PR 심사 중 ([winget-pkgs#401304](https://github.com/microsoft/winget-pkgs/pull/401304)) | CLA 서명 (§2.1) + 자동 갱신용 시크릿 (§2.2) |
| .deb / .rpm | ✅ CI 구성 완료 | 다음 릴리스(v0.9.3)부터 자산으로 첨부됨 |

릴리스 자동화 흐름 (태그 push 시):

```
git tag vX.Y.Z && git push origin vX.Y.Z
  → build-cli / build-desktop / build-bundle / build-packages (deb·rpm)
  → release (SHA256SUMS.txt 생성 + GitHub Release 발행)
  → update-tap    (homebrew-tap의 formula 자동 갱신)   ← TAP_GITHUB_TOKEN 필요
  → update-winget (winget-pkgs에 갱신 PR 자동 제출)     ← WINGET_GITHUB_TOKEN 필요
```

`-`가 포함된 태그(`v1.0.0-rc1` 등)는 tap/winget 갱신을 건너뛴다.

---

## 2. 직접 해야 하는 작업 (1회성)

### 2.1 Microsoft CLA 서명 — winget 최초 등록

winget-pkgs PR에는 Microsoft CLA 동의가 필요하다. **계정당 1회**만 하면 되고,
이후 모든 PR(자동 갱신 포함)에 적용된다.

1. [PR #401304](https://github.com/microsoft/winget-pkgs/pull/401304)에 `microsoft-github-policy-service` 봇이 남긴 코멘트를 확인한다.
2. 봇 안내에 따라 PR에 코멘트를 단다 (개인 자격 기여):
   ```
   @microsoft-github-policy-service agree
   ```
3. 서명 후 자동 검증 파이프라인(Azure)이 돌고, `Validation-Completed` 라벨이 붙으면
   모더레이터 승인을 기다린다. 신규 패키지는 보통 며칠 걸린다.
4. 머지 확인: Windows에서 `winget search keymander` → `winget install keymander`.

문제가 생기면 PR에 봇이 라벨/코멘트로 원인을 남긴다
(예: `Validation-Installation-Error`, `Manifest-Validation-Error`).

### 2.2 PAT 발급 + 시크릿 등록 — 자동 갱신 활성화

릴리스 CI가 다른 저장소(homebrew-tap, winget-pkgs fork)에 쓰기 위해 PAT가 필요하다.

**가장 간단한 방법 — classic PAT 1개로 둘 다 처리:**

1. GitHub → Settings → Developer settings → Personal access tokens →
   **Tokens (classic)** → Generate new token (classic)
2. Note: `keymander-release-automation`, Expiration: 1년 권장
3. Scopes: **`public_repo`** 하나만 체크
   (homebrew-tap 쓰기 + winget-pkgs fork/PR 모두 공개 저장소라 이걸로 충분)
4. 생성된 토큰을 복사한 뒤:
   ```bash
   gh secret set TAP_GITHUB_TOKEN    --repo bullpae/keymander-cli   # 붙여넣기
   gh secret set WINGET_GITHUB_TOKEN --repo bullpae/keymander-cli   # 같은 토큰 붙여넣기
   gh secret list --repo bullpae/keymander-cli                      # 확인
   ```

**보안을 더 조이려면** TAP 쪽만 fine-grained PAT로 분리:
Fine-grained tokens → Repository access: `homebrew-tap`만 선택 →
Permissions → Contents: **Read and write**. (winget 쪽은 임의 공개 저장소
fork/PR이 필요해서 fine-grained로는 제약이 있다 — classic `public_repo` 사용.)

토큰 만료 시 같은 명령으로 재등록하면 된다. 만료가 다가오면 GitHub가 메일로 알려준다.

### 2.3 (권장) git 커미터 정보 설정

현재 로컬 커밋이 `ATOM <atom@ATOM-MacBook-Pro.local>`로 기록되고 있다:

```bash
git config --global user.name  "bullpae"
git config --global user.email "bullpae@gmail.com"
```

---

## 3. 신규 프로젝트에 동일 적용하는 체크리스트

전제: GitHub 저장소 + 태그 push로 도는 릴리스 CI + 오픈소스 라이선스.

### 3.1 이 저장소에서 복사할 것

| 원본 | 내용 | 수정할 부분 |
|---|---|---|
| `.github/workflows/release.yml`의 `Generate checksums` 스텝 | SHA256SUMS.txt 생성 | 없음 (그대로) |
| `.github/workflows/release.yml`의 `update-tap` 잡 | tap formula 자동 갱신 | 저장소/formula 이름, 아티팩트 이름 |
| `.github/workflows/release.yml`의 `update-winget` 잡 | winget 갱신 PR 자동화 | `identifier`, `installers-regex` |
| `.github/workflows/release.yml`의 `build-packages` 잡 | .deb/.rpm 빌드 | 패키지/바이너리 이름 |
| `scripts/gen-homebrew-formula.sh` | formula 생성기 | 클래스명, URL, 바이너리 목록, caveats |
| `Cargo.toml`의 `[package.metadata.deb]`/`[package.metadata.generate-rpm]` | 패키지 메타데이터 | 이름·설명·assets 경로 |

### 3.2 채널별 절차

**① 체크섬** — release 잡에 스텝 복사. 끝.

**② Homebrew** — tap 저장소는 **계정당 하나**(`bullpae/homebrew-tap`)를 공유한다.
새 프로젝트는 `Formula/<이름>.rb`만 추가하면 된다:

1. 프로젝트에 `scripts/gen-homebrew-formula.sh` 복사 후 수정
2. 릴리스 산출물의 SHA를 넣어 초기 formula 생성 → tap 저장소에 커밋
3. `brew install bullpae/tap/<이름>`으로 설치 검증 (`brew test <이름>`까지)
4. `update-tap` 잡 복사, `TAP_GITHUB_TOKEN` 시크릿 등록 (기존 토큰 재사용 가능)

**③ winget** — 최초 1회 수동 등록 + 이후 자동:

1. 안정 URL의 릴리스 zip(또는 msi/exe)과 SHA256 준비
2. manifest 3종 작성: `manifests/<첫글자>/<계정>/<이름>/<버전>/` 아래
   `*.installer.yaml`, `*.locale.en-US.yaml`, `*.yaml` (버전 파일).
   keymander의 실제 예: [winget-pkgs#401304](https://github.com/microsoft/winget-pkgs/pull/401304)
   - 단독 exe zip이면 `InstallerType: zip` + `NestedInstallerType: portable`.
     **portable일 때만 NestedInstallerFiles에 여러 exe를 넣을 수 있다.**
   - `InstallerSha256`은 대문자 관례
3. 제출은 둘 중 하나:
   - `komac new <계정>.<이름>` (TTY 필요 — CI가 아닌 로컬 터미널에서 직접 실행)
   - 수동: winget-pkgs를 fork → 브랜치에 파일 3개 추가 → PR
     (거대 저장소라 clone 없이 GitHub contents API로 올리는 게 빠르다)
4. PR 제목 관례: `New package: <계정>.<이름> version X.Y.Z`
5. CLA는 계정당 1회 (§2.1)
6. 머지 후 `update-winget` 잡(winget-releaser)이 이후 버전을 자동 제출

**④ .deb/.rpm** — Rust 프로젝트면 cargo-deb/cargo-generate-rpm 메타데이터 복사·수정
후 `build-packages` 잡 복사. Rust가 아니면 [nfpm](https://nfpm.goreleaser.com/)이 같은
역할(설정 파일 하나로 deb/rpm/apk 생성)을 한다.

**릴리스 전 로컬 검증법** (CI가 막히는 걸 예방):

```bash
# deb/rpm 메타데이터 — 바이너리를 빌드해 두고 실제 패키징까지
cargo build --release && mkdir -p target/package && <config 준비>
cargo deb -p <pkg> --no-build -o /tmp/pkg && cargo generate-rpm -o /tmp/pkg

# 워크플로우 YAML
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"

# formula
brew install --formula ./Formula/<이름>.rb && brew test <이름> && brew uninstall <이름>
```

### 3.3 다음 단계 채널 (필요해지면)

- **AUR**(Arch): PKGBUILD 하나, 요건 최소 — 가장 쉬움
- **COPR**(Fedora/RHEL): spec 파일로 dnf 저장소 제공
- **homebrew-core / 공식 배포판**: 인지도 요건(별 75+ 등) 충족 후

---

## 4. 트러블슈팅 메모 (keymander에서 겪은 것)

- **komac이 TTY를 요구** — 파이프/스크립트 환경에서는 대화형 프롬프트(중첩 파일
  선택)가 막힌다. 로컬 터미널에서 직접 실행하거나 manifest를 수동 작성할 것.
- **winget portable + zip**: zip 안 최상위 디렉터리까지 포함한 상대 경로를
  `RelativeFilePath`에 적는다 (`keymander\kmd.exe`).
- **cargo-deb `$auto` 의존성**은 `dpkg-shlibdeps`가 필요 — CI apt 목록에
  `dpkg-dev` 포함 (macOS 로컬 테스트에서는 경고만 뜨고 의존성 없이 패키징됨).
- **Homebrew 바이너리 배포**는 코드사인/노터라이즈 없이도 Gatekeeper에 걸리지
  않는다 (brew 설치 경로는 quarantine 미부착). 직접 다운로드와 다른 점.
- **MSVC 배포 전제**: Windows 바이너리는 CRT 정적 링크(`crt-static`) 상태여야
  깨끗한 시스템에서 VCRUNTIME140.dll 없이 동작한다 (v0.9.1에서 적용).
