# Codex 리팩토링 리뷰 결과

코드는 수정하지 않고 `2863c48..HEAD`와 관련 구현을 검토했습니다. 최근 커밋에서 즉시 막아야 할 Critical/High 결함은 찾지 못했습니다. 다만 아래 두 건은 실제 사용자 동작에 영향을 주는 Medium 문제입니다.

## 발견 사항

### Medium — 잘못된 설정값이 성공으로 처리된다

`set_value`의 숫자·불리언 파싱은 실패해도 기존 값을 그대로 반환하고 전체 함수는 `Ok(())`를 반환합니다. TUI는 그 결과마저 버리고 무조건 `dirty = true`로 표시합니다.

- `crates/kmd-core/src/config.rs:754`
- `src/tui/settings/mod.rs:209`
- `src/tui/settings/mod.rs:299`

예를 들어 숫자 필드에 파싱 불가능한 값을 입력하면 사용자는 저장된 것으로 보지만 실제 값은 바뀌지 않습니다. 새 테스트도 “현재 값을 다시 쓰기” 때문에 이 문제를 검출하지 못합니다.

권장 접근:

- `parse_or` 대신 파싱 오류를 `ConfigError`로 반환
- TUI에서 `set_value` 결과에 따라 `dirty`를 설정하고 오류 메시지 표시
- 각 위젯별로 기본값과 다른 유효 값 및 잘못된 값을 넣는 테스트 추가

### Medium — 설정 저장이 주석뿐 아니라 장애 시 파일 전체도 잃을 수 있다

`Config::save`는 구조체를 새 TOML 문자열로 직렬화해 원본 파일을 직접 덮어씁니다.

- `crates/kmd-core/src/config.rs:631`
- `crates/kmd-core/src/config.rs:638`
- `crates/kmd-core/src/config.rs:640`

따라서 F8의 주석 손실은 확정입니다. 더 중요한 점은 직접 `write`라서 디스크 부족이나 프로세스 종료가 겹치면 부분 파일이 남을 수 있다는 것입니다.

우선순위는 P1으로 봅니다. 이미 있는 `toml_edit`로 해당 키만 변경하되, 임시 파일 작성 → flush/sync → rename 방식의 원자적 저장도 같이 적용하는 것이 좋습니다. 주석 보존만 고치고 직접 덮어쓰기는 남기지 않는 편이 낫습니다.

### Low — 설정 연결 테스트는 “키 존재”만 보장한다

테스트는 `get_value` 결과를 그대로 `set_value`에 넘깁니다.

- `src/tui/settings/items.rs:301`
- `src/tui/settings/items.rs:315`
- `src/tui/settings/items.rs:328`

이번 `everything_path` 누락은 잘 잡으며 유지할 가치가 있습니다. 하지만 다음은 빠져나갑니다.

- setter가 다른 필드를 변경하는 경우
- 파싱 실패를 성공으로 처리하는 경우
- setter가 입력을 무시하는 경우
- 위젯 종류와 실제 타입이 맞지 않는 경우
- Config 필드를 추가했지만 설정 UI 등록을 빼먹은 반대 방향 누락

단기적으로는 위젯별 대표 변경값을 쓰고 다시 읽어 동일한지 검사하면 됩니다. “모든 Config 필드가 UI에 있어야 한다”는 요구는 아니므로 반대 방향 검사는 별도 명시적 레지스트리가 생기기 전에는 강제하지 않는 편이 맞습니다.

### Low — `status_lines`가 프로토콜 크레이트에 표시 정책을 넣었다

- `crates/kmd-core/src/ipc.rs:169`
- `crates/kmd-core/src/ipc.rs:179`
- `crates/kmd-core/src/ipc.rs:211`

중복 제거 자체는 타당하고 현재 결과도 일관됩니다. 다만 `extra_line: Option<String>`은 호출자가 들여쓰기와 한글 레이블까지 만들어 특정 위치에 삽입하는 stringly typed 확장점입니다. 새 호출자가 생기거나 JSON/영문 출력이 필요해지면 확장하기 어렵습니다.

추천 구조는 `Response::Status`에서 `StatusView` 같은 구조화된 모델을 만들고, CLI 쪽 공용 presenter가 다음처럼 렌더링하는 것입니다.

```rust
StatusContext {
    log_path: Option<PathBuf>,
    autostart: Option<AutostartStatus>,
}
```

현 단계에서는 동작 결함이 아니므로 즉시 재작업할 필요는 없습니다. 다음 출력 형식 또는 소비자가 추가될 때 바꾸면 충분합니다.

### Low — 설계 정본 문서의 현재 상태가 모순된다

같은 결정 기록에 8월 27일 “트리거 결정 보류”와 8월 26일 “CapsLock 유지 채택”이 함께 있고, 날짜도 역순입니다.

- `docs/16_keymap_ergonomics.md:420`
- `docs/16_keymap_ergonomics.md:421`

또한 kanata 드리프트가 이번 커밋에서 해결됐는데 열려 있는 과제에는 여전히 “미착수”입니다.

- `docs/16_keymap_ergonomics.md:398`

문서가 “정본”을 자처하므로 일반적인 낡은 수치보다 우선해서 정리할 가치가 있습니다.

## 최근 keymap 변경 판단

`VIM_NAV_KBD`의 검토 대상 문법은 정적으로 올바릅니다.

- `defsrc`, `default`, `navigation`, `mouse`가 모두 22열입니다.
- `(layer-toggle navigation)`은 tap-hold 없이 직접 바인딩할 수 있습니다. kanata에서 `layer-toggle`은 실제 토글이 아니라 키를 누르는 동안 활성화되는 `layer-while-held`의 별칭입니다.
- `XX`는 해당 위치를 완전히 차단하는 공식 no-op 액션입니다.
- `_`는 하위 레이어 동작을 사용하는 transparent 액션이라 nav의 Shift 통과 의도와 맞습니다.
- `tap-dance`, mouse 이동 및 wheel 액션의 인자 형태도 문서와 일치합니다.

근거는 kanata의 [공식 Configuration Guide](https://github.com/jtroo/kanata/wiki/Configuration-guide)와 [공식 샘플 설정](https://github.com/jtroo/kanata/blob/main/cfg_samples/kanata.kbd)을 대조했습니다.

다만 `launch-kmd (cmd kmd-desktop)`은 kanata가 `cmd` feature로 빌드되어야 한다는 기존 배포 조건이 있습니다. 이번 변경이 도입한 문제는 아니지만 설치 문서나 진단에서 확인할 만합니다.

추가된 드리프트 테스트 3개는 모두 통과했고, 열 정렬·금지 문자열·트리거의 큰 드리프트를 막습니다. 그러나 실제 parser 테스트는 아니므로 별도 CI에서 kanata의 config 검사 모드를 실행하는 것이 최종 방어선입니다.

## F4–F8 우선순위

| 항목 | 판단 | 우선순위·접근 |
|---|---|---|
| F4 라우팅 중복 | 가치 높음. 두 UI가 같은 prefix enum을 쓰면서 dispatch와 결과 생성이 갈라져 장기 드리프트 가능성이 큼 | P1–P2. 모든 핸들러를 억지로 공용화하지 말고, core에 `QueryOutcome`을 반환하는 순수 라우팅/결과 생성 계층을 둔 뒤 GUI/TUI가 상태 적용만 담당. `emoji_keyword_started`와 clipboard 같은 UI별 정책은 어댑터에 유지 |
| F5 CLI/TUI 테스트 공백 | 가치 높음. 줄 수 자체보다 상태 전이·부작용 코드가 무검증인 것이 문제 | P1. `update_search`에 prefix별 table test, 설정 편집 성공/실패, 선택/실행 상태 전이를 먼저 추가. 터미널 렌더링 snapshot보다 상태 테스트 우선 |
| F6 `process_key` 451줄 | 가치 매우 높지만 위험도도 가장 높음 | P1. 의미 변경 없는 단계적 추출만 수행. `handle_toggle`, `handle_tap_hold`, `handle_layer`, `handle_combo`, `handle_double_tap`처럼 기존 우선순서를 명시적으로 유지하고 현재 시나리오 테스트를 characterization test로 사용 |
| F7 낡은 문서 수치 | 할 가치는 있으나 수동 숫자는 다시 낡음 | P3. 현재 수치만 고치기보다 정확한 줄 수 대신 대략적 규모나 생성 스크립트 사용 |
| F8 주석 손실 | 사용자 데이터 품질 문제라 가치 높음 | P1. `toml_edit` 기반 부분 수정과 원자적 파일 교체를 함께 적용 |

## 추가 설계 판단

- 크레이트 경계는 대체로 합리적입니다. 검색·설정·IPC 모델이 `kmd-core`, 훅과 상태 머신이 `kmd-daemon`, GUI 상태가 `kmd-desktop`에 있습니다. 가장 눈에 띄는 경계 누수는 `ipc::Response::status_lines`의 표시 문구입니다.
- Windows/macOS에는 `send_key_press`, `send_chord_engage`, mouse/combo 실행 등 공통 이름의 중복이 있지만 OS API와 이벤트 의미가 크게 다릅니다. 거대한 “통합 플랫폼 모듈”보다는 공통 `Backend` trait과 공유 작업 디스패처 정도만 추출하는 것이 안전합니다.
- 전역 `Mutex`/`OnceLock`은 훅 콜백과 OS 수명 제약 때문에 대부분 정당합니다. 다만 `CONFIG_LOAD_ERROR`와 `KEYMAP_SUMMARY`는 서버 런타임 상태 구조체로 묶으면 테스트 격리와 재시작 의미가 선명해집니다.
- `let _ =` 중 테스트 cleanup, 채널 종료, 캐시 재생성 실패는 대체로 무해합니다. 반면 설정 setter 결과 무시가 실제 위험 지점입니다. 훅 재설치 결과를 버리는 `crates/kmd-daemon/src/keybind/windows.rs:1543`도 함수 내부에서 충분히 상태를 기록하는지 계속 감사 대상으로 두는 것이 좋습니다.

검증으로 kanata 드리프트 테스트 3개와 설정 연결 테스트 1개를 실행했고 모두 통과했습니다. 리뷰 과정에서는 작업 트리의 코드를 변경하지 않았습니다.
