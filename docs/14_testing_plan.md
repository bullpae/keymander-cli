# 테스트 전략 — E2E 자동화 계획 + 테스트 정리 계획

> 상태: 2026-08-11 수립. Tier 1은 이 문서와 함께 구현됨.
> Tier 2는 진행 중인 Windows 안정화 브랜치들(macos.rs·engine.rs·server.rs를
> 공유)이 main에 합류한 뒤 착수한다 — 충돌 회피 시퀀싱.

## 현황 (2026-08-11)

| 층 | 수단 | 규모 | CI |
|---|---|---|---|
| 단위 (로직) | `#[test]` 인라인 | 339개 / 전체 ~4초 | ✅ 3-OS 매트릭스 |
| 엔진 시뮬레이션 | engine.rs 키 시퀀스 테스트 | 53개 (탭홀드·더블탭·chord·리피트) | ✅ |
| 통합 | kmd-core `tests/audit_no_window.rs` | 1개 | ✅ |
| 데몬 프로세스 E2E | **Tier 1 (이 계획으로 추가)** | `tests/e2e_daemon.rs` | ✅ unix |
| 키 주입 진짜 E2E | 수동 — `paste-test`/`clip-test` CLI + docs/08·12·13 체크리스트 | — | ❌ |
| GUI (iced) | 앱 모델 단위 테스트만 | — | ❌ |

교훈(2026-08-10 키보드 먹통 사고): 엔진 단위 테스트는 **OS와의 상호작용**
(Ctrl+Space 홀드가 입력 소스 피커를 여는 것 같은)을 잡지 못한다. 그 빈틈을
메우는 것이 E2E 계획의 목적이다.

## Part A — E2E 자동화

### Tier 1: 데몬 프로세스/IPC E2E (CI, unix) — ✅ 구현됨

`crates/kmd-daemon/tests/e2e_daemon.rs`. 빌드된 실제 `kmd-daemon` 바이너리를
`CARGO_BIN_EXE_kmd-daemon`으로 spawn해 검증한다:

- 기동 → 포트 파일 생성 → Status 응답(레이어 로드, config 오류 없음)
- **토큰 인증 거부** (잘못된 토큰 → 요청 거부) — 보안 회귀 방지
- 중복 기동 거부 (kmd.lock 단일 인스턴스)
- Shutdown → 프로세스 정상 종료

격리: 자식 프로세스의 `HOME`/`XDG_*`를 tempdir로 돌려 config·data·런타임
파일(포트/락)이 실제 사용자 환경과 완전히 분리된다. 이 때문에 **unix 전용**
— Windows의 `dirs`는 KnownFolder API를 쓰므로 env로 격리가 안 된다
(Tier 2에서 `KMD_DATA_DIR` 오버라이드를 추가해 Windows도 편입).

안전장치: 테스트 config는 트리거를 `LAlt`로 명시한다 — 기본값(CapsLock)이면
데몬이 hidutil 재맵(시스템 전역 상태)을 건드린다. `config_error == None`
어서션이 이 안전장치가 무효화되는 것(파싱 실패 → 기본 config 폴백)을 잡는다.

게이트: `KMD_E2E=1`일 때만 실행 (미설정 시 조용히 skip). CI test 잡이 설정하고,
로컬은 `KMD_E2E=1 cargo test -p kmd-daemon --test e2e_daemon`.

**첫 실행 성과**: 중복 기동 시나리오가 실제 버그를 잡았다 — kmd-daemon
바이너리 자체에 단일 인스턴스 가드가 없어(CLI만 체크) launchd·직접 실행
경로로 데몬 2개가 동시에 뜰 수 있었다(이벤트 탭 이중 설치 + 포트 파일
덮어쓰기). server::run() 진입부에 Ping 생존 판정 가드를 추가해 해결.

### Tier 2: 키 주입 진짜 E2E — 로컬 게이트 (Windows 브랜치 합류 후)

핵심 성질: `CGEventPost`로 세션에 쏜 이벤트는 데몬의 CGEventTap을 **그대로
통과**한다(오늘 데몬의 합성 이벤트가 자기 탭에 MAGIC_USER_DATA 마커로 자기
구분을 하는 이유). 마커 없는 합성 이벤트 = 가짜 물리 키.

`kmd daemon e2e` 셀프테스트 명령(신규):

1. listen-only 탭(출력 캡처) 설치
2. 마커 없는 키 이벤트 주입: CapsLock(F19) 탭 / 홀드+HJKL / 연타
3. 캡처된 출력이 기대 리맵(한영 토글 1회만·화살표·디바운스)과 일치하는지 어서션

- 접근성이 부여된 머신 전용 — **릴리스 전 로컬 1커맨드 게이트**로 운용.
  GitHub macOS 러너는 SIP 때문에 접근성 부여 불가 → CI 편입 불가.
- 구현 위치가 macos.rs·cmd/daemon.rs라 **진행 중인 4개 브랜치와 충돌** —
  그들 합류 후 착수한다.
- 타이밍 어서션은 벽시계 의존 금지(mouse.rs CI 플레이크 전례) — 이벤트
  카운트/순서로 판정하고, 대기는 폴링+타임아웃으로.

추가 실험(후순위): Windows 러너는 대화형 데스크톱이 있어 SendInput+LL 훅이
동작하는 편 — Tier 2를 windows CI에 올릴 수 있는지 1회 검증해 볼 가치.

### Tier 3: GUI (kmd-desktop) — 보류

iced는 접근성 트리/드라이버가 없어 표준 GUI 자동화가 불가. 현실적 노선:

- update 로직을 모델 단위 테스트로 계속 확대 (지금 방식 유지)
- 픽셀/스크린샷 자동화는 도입하지 않는다 — 유지비 > 효용 (테마·폰트·DPI churn)
- 실기기 확인이 필요한 부분(런처 GUI 상호작용)은 docs 체크리스트 유지

## Part B — 불필요 테스트 정리

전체 실행 시간이 ~4초라 **런타임 비용은 없다. 비용은 유지보수(churn)다** —
정리 기준도 거기에 맞춘다.

### 삭제 기준 (하나라도 해당하면 후보)

1. **구현 미러링**: 코드의 매핑 테이블/상수를 그대로 복사해 비교하는 테스트 —
   데이터 추가마다 같은 내용을 두 번 쓰게 만들고, 실수를 잡지 못한다
   (테이블 오타가 테스트에도 복사되므로).
2. **중복 커버리지**: 같은 경로를 다른 이름으로 다시 도는 테스트 — 대표 1개 +
   경계 사례만 남긴다.
3. **죽은 기능**: 제거·비활성화된 기능의 테스트.
4. **벽시계 의존**: sleep/Instant 실측에 기대는 판정 — 삭제가 아니라 순수 함수
   재작성 대상 (CI 플레이크 원천).

### 유지 기준 (삭제 금지)

- 엔진 시퀀스 테스트 전체 (engine.rs 53개) — 이 프로젝트의 핵심 자산
- 사고·버그의 회귀 테스트 (오토리피트, chord 누출, stuck key, 한영 스냅백류)
- 파서/조합 로직 (hangul 조합, query prefix, transform, web URL 빌드)
- 보안 어서션 (토큰 인증, 파일 권한, audit_no_window)

### 감사 결과 (2026-08-11, 1차)

| 영역 | 개수 | 판정 |
|---|---|---|
| engine.rs 53 · server.rs 5 · mouse/윈도우 훅 | 유지 | 핵심 + 진행 중 브랜치 영역(동결) |
| kmd-core hangul 19 · web 22 · transform 9 · search 17 등 | 유지 | 행동 테스트, 값어치 있음 |
| brand_icons.rs 20 · system_icons.rs 5 | **검토 후보** | 매핑 데이터 테이블 성격 — 구현 미러링(기준 1) 여부를 개별 확인해 대표+경계만 남기는 축소 검토 |
| builtin_emoji.rs 10 | 부분 검토 | db 로드 1 + 검색 대표 2~3개면 충분한지 확인 |
| kmd-core 3.67초 (최다 소요) | 프로파일 | 느린 테스트 상위를 확인해 fixture 공유 검토 (io 반복 로드 의심) |

**동결 영역**: 진행 중 4개 브랜치가 건드리는 파일(macos.rs, windows.rs,
clipboard.rs, engine.rs, server.rs, ipc.rs, config.rs, app.rs, tui/app.rs)의
테스트는 브랜치 합류 전까지 정리하지 않는다 — 충돌 비용 > 정리 이득.

### 프로세스

- 삭제는 커밋 메시지에 기준 번호(위 1~4)와 근거를 남긴다.
- 새 버그 수정에는 회귀 테스트를 함께 넣는 관행 유지 (이번 한영 직렬화처럼
  E2E가 필요한 것은 Tier 2 시나리오 목록에 추가).
