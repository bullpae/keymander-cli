# CapsLock 레이어 트리거

> 상태: **0.13.0부터 기본값** (2026-08-10 실험 → 같은 날 macOS/Windows 실기기
> 검증 후 승격). vim-nav 프리셋 기본 트리거 = CapsLock.
> LAlt로 되돌리기는 config 두 줄 (아래 참고).
>
> **2026-08-12 개정**: 탭 한/영은 폐지 (탭 = 무동작). CapsLock과 LShift가
> 같은 새끼손가락 인접 키라 실수 탭이 조용히 입력 소스를 바꿔 혼란을 줬다.
> 한/영 주 경로는 RAlt 짧은 탭(물리 한영키 자리, 마우스 레이어 tap_action)으로
> 이동·통일. RShift 더블탭 기본도 같은 이유(경로 단일화)로 제거.
> 이 문서 아래의 "탭 = 한/영" 서술은 개정 전 이력이다.
>
> **인접 비용**: CapsLock↔LShift가 같은 새끼손가락이라는 데서 오는 구조적
> 비용과 오발사 등급표는 [16_keymap_ergonomics.md](16_keymap_ergonomics.md) §3에
> 정리돼 있다. 트리거를 바꾸려는 검토를 시작하기 전에 그쪽을 먼저 볼 것.
>
> **LAlt 복귀 재검토(2026-08-26)**: 16번 §5에 결론이 있다 — `Ctrl+Alt+key`는
> `unmapped="passthrough"`로 **이미 보존되지만**, `Alt+key`(리본 니모닉)와
> `Alt+Shift+key`(HWP)는 구조적으로 못 살린다. CapsLock 유지 + 레이어 로컬
> Shift로 간다.

## 배경

레이어 트리거를 modifier(LAlt/Ctrl/Cmd)로 두면 그 modifier의 글자 단축키가
nav 매핑 키에서 가려진다(예: HWP `Alt+Shift+N` 자간좁히기 → nav가 N을 가로챔,
`Ctrl+N` 개체삽입 프리픽스 → 붕괴). vim-nav를 끄지 않고 이 충돌을 없애는
유일한 길은 **트리거를 non-modifier로** 두는 것이고, 그중 CapsLock이 후보다:

- **충돌 0**: CapsLock+글자는 어떤 앱도 단축키로 안 씀 → 모든 Ctrl/Alt/Cmd
  단축키 보존 + vim-nav 동시 사용.
- **tap-hold 안전**: 평소 타이핑에 안 눌리는 키라 Space와 달리 롤오버·지연·
  IME 문제 없음.

## macOS의 함정과 해결 (hidutil → F19)

macOS는 CapsLock을 LED-토글 `flagsChanged`로 전달해(코드 `macos.rs`의
`(flags & 0x10000)`) **홀드 감지가 불가능**하고 디바운스 지연도 있다.

**해결**: 데몬이 시작 시 `hidutil`로 CapsLock(HID `0x700000039`)을
F19(`0x70000006E`)로 재맵한다. F19는 기본 기능 없는 깨끗한 일반 키라
down/up이 정확히 오고 토글·지연이 없다. 그다음 엔진의 트리거를 F19로 바꿔 쓴다.

- 구현: `macos.rs`의 `remap_capslock_trigger_to_f19()` — config 레이어 트리거에
  CapsLock이 있으면 hidutil 적용 + 트리거를 `VKey::F19`로 재작성.
- 원복: `stop()`에서 `clear_hidutil_remap()` (UserKeyMapping 비움). 재부팅도 초기화.
- **주의**: 데몬을 SIGKILL 등으로 급하게 죽이면 stop()이 안 돌아 재맵이 남는다.
  재부팅하거나 `hidutil property --set '{"UserKeyMapping":[]}'`로 수동 원복.
  `kmd daemon stop`(정상 종료)은 자동 원복한다.

Windows/Linux는 CapsLock이 평범한 키라 재맵 불필요 — trigger="CapsLock"만으로 동작.

## 기본값 (0.13.0~)

vim-nav 프리셋 기본이 이 구성이다 — config 없이도 동작한다:

```toml
[launcher.keymap.layers.nav]
trigger = "CapsLock"      # 홀드 = nav 레이어. 짧게 탭 = 무동작 (2026-08-12~)
tap_hold_ms = 200
# tap_action = "Hangul"   # 탭 한영으로 되돌리려면 주석 해제
```

탭 액션의 변천: 처음엔 `Escape`(물리 Esc 중복 + 오탭 시 조합 취소·다이얼로그
닫힘 부작용) → `Hangul`(한영·레이어를 CapsLock 하나로 통합) → **무동작**
(2026-08-12, 실수 탭이 조용히 입력 소스를 바꾸는 문제로 한영을 RAlt 탭으로 이동).

### 한영 토글 직렬화·디바운스 (macOS, 사고 재발 방지)

macOS의 `Hangul` 액션은 Ctrl+Space(입력 소스 전환 단축키)를 합성 주입한다
(`macos.rs toggle_input_source()` — TIS 직선택은 반쪽 전환 버그로 배제, 상단
독스트링 참고). 이 단축키는 macOS가 **"홀드하면 입력 소스 피커 표시"**로도
해석하기 때문에, 토글이 겹치면(연타 → 주입 스레드 2개의 Ctrl down/up 인터리브)
피커(TextInputMenuAgent)가 열리고, 피커가 key focus를 훔친 채 멈추면 시스템
전역 키보드가 먹통이 된다 — 마우스 포인터만 살고 재부팅 전까지 복구 불가
(2026-08-10 실사고).

방어 2중 (`toggle_input_source()`):
- **in-flight 가드** — 이전 주입(4이벤트)이 끝나기 전의 재요청은 버린다.
- **디바운스 300ms** — 주입 완료 후 300ms 내 재요청도 버린다. **CapsLock
  연타 시 두 번째 탭이 무시되는 것은 의도된 동작**이다(연타는 대개 "전환이
  안 된 것 같아 다시 누름"이라 한 번만 수행하는 게 의도에 부합).

만에 하나 유사 증상(키보드 전멸·마우스 생존)이 다시 나타나면, 재부팅 대신:
시스템 설정 → 손쉬운 사용 → 키보드 → **보조 키보드**를 마우스로 켜고
터미널에 `killall TextInputMenuAgent` 입력.

데몬이 시작 시 자동으로 macOS 재맵을 적용한다. `kmd daemon status`에
`nav: CapsLock 홀드`로 표시된다(엔진 내부는 F19).

주의(Windows): 트리거가 CapsLock이면 예전 기본이던 HHKB식 CapsLock 모드탭
(탭=Caps/홀드=Ctrl)은 주입되지 않는다 — 트리거를 LAlt로 되돌리면 모드탭이
다시 살아난다(둘은 같은 키를 쓰므로 동시 불가).

## HHKB

HHKB는 CapsLock 자리가 Control이라 이 방식이 그대로 안 맞는다. HHKB에서는
그 키를 `tap=Ctrl / hold=layer` tap-hold로 두는 별도 구성이 필요(향후 과제).

## 이전 방식(LAlt)으로 되돌리기

config.toml(macOS `~/Library/Application Support/kmd/`, Windows `%APPDATA%\kmd\`,
포터블 설치는 `kmd-data/`)의 nav 레이어에 두 줄만 덮어쓰고 `kmd daemon restart`:

```toml
trigger = "LAlt"
tap_action = "Escape"
```

macOS는 데몬 재시작 시 hidutil 재맵이 자동 해제되어 CapsLock 대문자 잠금이
복구된다. (급할 땐 `kmd daemon stop`만으로도 재맵은 원복된다.)
