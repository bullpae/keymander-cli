# CapsLock 레이어 트리거

> 상태: **0.13.0부터 기본값** (2026-08-10 실험 → 같은 날 macOS/Windows 실기기
> 검증 후 승격). vim-nav 프리셋 기본 트리거 = CapsLock, 탭 = 한/영.
> LAlt로 되돌리기는 config 두 줄 (아래 참고).

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
trigger = "CapsLock"      # 홀드 = nav 레이어
tap_action = "Hangul"     # 짧게 탭 = 한/영 전환 (macOS 순정 caps 한영과 동일 UX)
tap_hold_ms = 200
```

탭 액션은 처음엔 `Escape`였으나 물리 Esc와 중복이고 오탭 시 조합 취소·다이얼로그
닫힘 부작용이 있어 `Hangul`로 교체 — 한영도 레이어도 CapsLock 하나로 통합된다.
(Shift/CapsLock 인접 오타 문제도 해소. Shift+Space·RShift 더블탭 한영은 그대로 유지.)

데몬이 시작 시 자동으로 macOS 재맵을 적용한다. `kmd daemon status`에
`nav: CapsLock 홀드`로 표시된다(엔진 내부는 F19).

주의(Windows): 트리거가 CapsLock이면 예전 기본이던 HHKB식 CapsLock 모드탭
(탭=Caps/홀드=Ctrl)은 주입되지 않는다 — 트리거를 LAlt로 되돌리면 모드탭이
다시 살아난다(둘은 같은 키를 쓰므로 동시 불가).

## HHKB

HHKB는 CapsLock 자리가 Control이라 이 방식이 그대로 안 맞는다. HHKB에서는
그 키를 `tap=Ctrl / hold=layer` tap-hold로 두는 별도 구성이 필요(향후 과제).

## 이전 방식(LAlt)으로 되돌리기

`kmd-data/config.toml`의 nav 레이어에 두 줄만 덮어쓰고 데몬 재시작:

```toml
trigger = "LAlt"
tap_action = "Escape"
```

macOS는 데몬 재시작 시 hidutil 재맵이 자동 해제되어 CapsLock 대문자 잠금이
복구된다. (급할 땐 `kmd daemon stop`만으로도 재맵은 원복된다.)
