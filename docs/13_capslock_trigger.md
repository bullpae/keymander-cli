# CapsLock 레이어 트리거 (실험)

> 상태: 실험 기능 (2026-08-10). 기본 프리셋은 여전히 LAlt — config로 opt-in.

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

## 사용 (opt-in)

`kmd-data/config.toml`:

```toml
[launcher.keymap.layers.nav]
trigger = "CapsLock"      # LAlt 대신
tap_action = "Escape"     # 짧게 탭 = Esc (원하면 변경)
tap_hold_ms = 200
```

배포 후 데몬이 자동으로 macOS 재맵을 적용한다. `kmd daemon status`에
`nav: CapsLock 홀드`로 표시된다(엔진 내부는 F19).

## HHKB

HHKB는 CapsLock 자리가 Control이라 이 방식이 그대로 안 맞는다. HHKB에서는
그 키를 `tap=Ctrl / hold=layer` tap-hold로 두는 별도 구성이 필요(향후 과제).

## 되돌리기

- 빠른 원복: `kmd daemon stop` (hidutil 자동 원복) 후 config에서 trigger를
  `LAlt`로 되돌리고 재배포.
- config 백업: 실험 적용 시 `config.toml.bak-lalt`로 보관됨.
