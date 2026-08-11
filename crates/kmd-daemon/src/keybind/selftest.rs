//! 키 주입 셀프테스트 (docs/14 Tier 2, macOS 전용)
//!
//! 접근성이 부여된 실기기에서 `kmd daemon e2e`로 실행하는 진짜 E2E:
//! 마커(MAGIC_USER_DATA) 없는 합성 키 이벤트는 데몬의 CGEventTap이 물리
//! 입력과 똑같이 처리하므로, "가짜 물리 키"를 주입하고 listen-only 탭으로
//! 데몬이 내보낸 리맵 출력을 캡처해 어서션한다.
//!
//! 검증 시나리오:
//! - A. 레이어 홀드 매핑: 트리거 홀드 + 매핑 키 → 기대 출력 키 (마커 있음)
//! - B. 트리거 탭 = 한영: Ctrl+Space 주입이 정확히 1회
//! - C. 연타 디바운스: 300ms 내 재탭은 무시 — Ctrl+Space가 추가로 1회만
//!   (B+C 합계 토글 2회 = 입력 소스 원상 복귀)
//!
//! 주의: 실행 중 잠깐(1~2초) 실제 키 입력 경로에 합성 이벤트가 흐른다 —
//! 사용자가 명시적으로 호출하는 명령이며, 실행 중 타이핑은 피할 것.
//! 타이밍 어서션은 이벤트 개수/순서 기반이고 대기는 폴링+데드라인이다
//! (벽시계 실측 판정 금지 — docs/14).

use super::macos::{capslock_remapped, vkey_to_cg, CG_EVENT_SOURCE_USER_DATA, MAGIC_USER_DATA};
use super::{BindAction, KeybindConfig, VKey};
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ── 최소 FFI (listen-only 탭 + 주입) ─────────────────────────────────────────

type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CGEventTapProxy = *mut c_void;

const CG_EVENT_KEY_DOWN: u32 = 10;
const CG_EVENT_KEY_UP: u32 = 11;
const CG_EVENT_FLAGS_CHANGED: u32 = 12;
/// 주입 위치 — 물리 키와 같은 최상류(HID) 지점. 세션 위치에 주입하면 같은
/// 세션 위치의 head 탭(데몬 활성 탭)이 이벤트를 받지 못해 리맵이 안 걸린다
/// (실측 — 원본 키가 그대로 누출됐다).
const CG_HID_EVENT_TAP: u32 = 0;
const CG_SESSION_EVENT_TAP: u32 = 1;
const CG_TAIL_APPEND_EVENT_TAP: u32 = 1;
const CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;

type TapCallback = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    etype: u32,
    event: CGEventRef,
    info: *mut c_void,
) -> CGEventRef;

// macos.rs와 같은 심볼을 자기완결적으로 재선언한다 — 타입 별칭 표기가 달라
// clashing 경고가 나지만 전부 포인터라 ABI는 동일하다.
#[allow(clashing_extern_declarations)]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventCreateKeyboardEvent(
        source: *mut c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
}

#[allow(clashing_extern_declarations)]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: *const c_void);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CFRelease(cf: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
}

// ── 캡처 버퍼 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
struct Captured {
    etype: u32,
    keycode: u16,
    /// 데몬이 주입한 이벤트인가 (MAGIC_USER_DATA 마커)
    marked: bool,
}

fn capture_buf() -> &'static Mutex<Vec<Captured>> {
    static BUF: OnceLock<Mutex<Vec<Captured>>> = OnceLock::new();
    BUF.get_or_init(|| Mutex::new(Vec::new()))
}

/// listen 탭 러너의 CFRunLoop (stop용). 0 = 미실행.
static LISTEN_RUNLOOP: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn listen_callback(
    _proxy: CGEventTapProxy,
    etype: u32,
    event: CGEventRef,
    _info: *mut c_void,
) -> CGEventRef {
    if matches!(
        etype,
        CG_EVENT_KEY_DOWN | CG_EVENT_KEY_UP | CG_EVENT_FLAGS_CHANGED
    ) {
        let keycode = unsafe {
            CGEventGetIntegerValueField(event, 9 /* kCGKeyboardEventKeycode */)
        } as u16;
        let marked = unsafe { CGEventGetIntegerValueField(event, CG_EVENT_SOURCE_USER_DATA) }
            == MAGIC_USER_DATA;
        if let Ok(mut buf) = capture_buf().lock() {
            buf.push(Captured {
                etype,
                keycode,
                marked,
            });
        }
    }
    event
}

fn clear_captures() {
    if let Ok(mut buf) = capture_buf().lock() {
        buf.clear();
    }
}

fn count_marked_downs(keycode: u16) -> usize {
    capture_buf()
        .lock()
        .map(|buf| {
            buf.iter()
                .filter(|c| c.marked && c.etype == CG_EVENT_KEY_DOWN && c.keycode == keycode)
                .count()
        })
        .unwrap_or(0)
}

fn saw_unmarked_down(keycode: u16) -> bool {
    capture_buf()
        .lock()
        .map(|buf| {
            buf.iter()
                .any(|c| !c.marked && c.etype == CG_EVENT_KEY_DOWN && c.keycode == keycode)
        })
        .unwrap_or(false)
}

/// 조건이 참이 될 때까지 폴링. 데드라인 내 도달하면 true.
fn poll_until(deadline: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// 마커 없는 "가짜 물리" 키 이벤트 주입 — 데몬 탭이 물리 입력으로 처리한다.
fn inject_physical(keycode: u16, down: bool) {
    unsafe {
        let ev = CGEventCreateKeyboardEvent(std::ptr::null_mut(), keycode, down);
        if ev.is_null() {
            return;
        }
        CGEventPost(CG_HID_EVENT_TAP, ev);
        CFRelease(ev);
    }
}

fn tap_key(keycode: u16, hold: Duration) {
    inject_physical(keycode, true);
    std::thread::sleep(hold);
    inject_physical(keycode, false);
}

// ── 시나리오 실행 ────────────────────────────────────────────────────────────

pub fn run(preset: &KeybindConfig) -> Result<String, String> {
    if let Some(err) = super::hook_error() {
        return Err(format!(
            "키 훅이 죽어 있어 셀프테스트를 실행할 수 없습니다: {err}"
        ));
    }

    // 검증 대상: SendKey 매핑이 있는 첫 레이어 (nav 우선)
    let layer = preset
        .layers
        .iter()
        .filter(|l| {
            l.mappings
                .values()
                .any(|a| matches!(a, BindAction::SendKey(_)))
        })
        .max_by_key(|l| (l.name == "nav") as u8)
        .ok_or("SendKey 매핑이 있는 레이어가 없습니다")?;
    let (mapped_key, expect_out) = layer
        .mappings
        .iter()
        .find_map(|(k, a)| match a {
            // H→Left 같은 순수 키 출력을 우선 선택 (Space=launch 등 부작용 배제)
            BindAction::SendKey(out) if !matches!(out, VKey::Hangul | VKey::Hanja) => {
                Some((*k, *out))
            }
            _ => None,
        })
        .ok_or("순수 SendKey 매핑이 없습니다")?;

    let mut trigger = layer.trigger;
    if trigger == VKey::CapsLock && capslock_remapped() {
        trigger = VKey::F19; // 엔진은 hidutil 재맵 후 F19를 트리거로 쓴다
    }
    let trigger_kc = vkey_to_cg(trigger);
    let mapped_kc = vkey_to_cg(mapped_key);
    let expect_kc = vkey_to_cg(expect_out);
    let hangul_tap = layer.tap_action == Some(VKey::Hangul);
    let hold_wait = Duration::from_millis(u64::from(layer.tap_hold_ms) + 80);
    const SPACE_KC: u16 = 0x31;
    const POLL: Duration = Duration::from_millis(1500);

    // ── listen-only 탭 설치 (tail — 데몬 활성 탭 이후의 최종 스트림 관찰) ──
    let mask =
        (1u64 << CG_EVENT_KEY_DOWN) | (1u64 << CG_EVENT_KEY_UP) | (1u64 << CG_EVENT_FLAGS_CHANGED);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<bool>();
    let runner = std::thread::spawn(move || unsafe {
        let tap = CGEventTapCreate(
            CG_SESSION_EVENT_TAP,
            CG_TAIL_APPEND_EVENT_TAP,
            CG_EVENT_TAP_OPTION_LISTEN_ONLY,
            mask,
            listen_callback,
            std::ptr::null_mut(),
        );
        if tap.is_null() {
            let _ = ready_tx.send(false);
            return;
        }
        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        let rl = CFRunLoopGetCurrent();
        CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        LISTEN_RUNLOOP.store(rl as usize, Ordering::Release);
        let _ = ready_tx.send(true);
        CFRunLoopRun();
        CGEventTapEnable(tap, false);
        CFRelease(source);
        CFRelease(tap);
        LISTEN_RUNLOOP.store(0, Ordering::Release);
    });
    if !ready_rx
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or(false)
    {
        let _ = runner.join();
        return Err("listen 탭 설치 실패 — 접근성 권한을 확인하세요".into());
    }

    let stop_listener = || {
        let rl = LISTEN_RUNLOOP.load(Ordering::Acquire);
        if rl != 0 {
            unsafe { CFRunLoopStop(rl as CFRunLoopRef) };
        }
    };

    let mut report = Vec::new();
    let mut failed = false;

    // ── A. 레이어 홀드 매핑 ─────────────────────────────────────────────
    clear_captures();
    inject_physical(trigger_kc, true);
    std::thread::sleep(hold_wait);
    inject_physical(mapped_kc, true);
    std::thread::sleep(Duration::from_millis(30));
    inject_physical(mapped_kc, false);
    std::thread::sleep(Duration::from_millis(30));
    inject_physical(trigger_kc, false);

    let a_ok = poll_until(POLL, || count_marked_downs(expect_kc) >= 1);
    let leaked = saw_unmarked_down(mapped_kc);
    if a_ok && !leaked {
        report.push(format!(
            "A. 레이어 매핑: {trigger:?} 홀드 + {mapped_key:?} → {expect_out:?} ✓"
        ));
    } else {
        failed = true;
        report.push(format!(
            "A. 레이어 매핑 실패: 출력 {}회, 원본 키 누출 {}",
            count_marked_downs(expect_kc),
            leaked
        ));
    }

    if hangul_tap {
        // 직전 시나리오/이전 사용과의 디바운스 간섭 차단
        std::thread::sleep(Duration::from_millis(400));

        // ── B. 트리거 탭 = 한영 (Ctrl+Space 주입 1회) ──────────────────
        clear_captures();
        tap_key(trigger_kc, Duration::from_millis(50));
        let b_ok = poll_until(POLL, || count_marked_downs(SPACE_KC) >= 1);
        // 여분 발화가 없는지 짧게 더 관찰
        std::thread::sleep(Duration::from_millis(200));
        let b_count = count_marked_downs(SPACE_KC);
        if b_ok && b_count == 1 {
            report.push("B. 탭=한영: Ctrl+Space 1회 주입 ✓".into());
        } else {
            failed = true;
            report.push(format!("B. 탭=한영 실패: Space 주입 {b_count}회 (기대 1)"));
        }

        // ── C. 연타 디바운스 (300ms 내 재탭 무시) ──────────────────────
        std::thread::sleep(Duration::from_millis(400));
        clear_captures();
        tap_key(trigger_kc, Duration::from_millis(50));
        std::thread::sleep(Duration::from_millis(60));
        tap_key(trigger_kc, Duration::from_millis(50));
        poll_until(POLL, || count_marked_downs(SPACE_KC) >= 1);
        std::thread::sleep(Duration::from_millis(400));
        let c_count = count_marked_downs(SPACE_KC);
        if c_count == 1 {
            report.push("C. 연타 디바운스: 2연타 → Ctrl+Space 1회만 ✓".into());
        } else {
            failed = true;
            report.push(format!(
                "C. 연타 디바운스 실패: Space 주입 {c_count}회 (기대 1 — 겹치면 피커 먹통 위험!)"
            ));
        }
        // B(1회) + C(1회) = 토글 2회 → 입력 소스 원상 복귀
        report.push("   (한영 토글 합계 2회 — 입력 소스 원상 복귀됨)".into());
    } else {
        report.push(format!(
            "B/C. 탭 시나리오 skip (tap_action이 Hangul이 아님: {:?})",
            layer.tap_action
        ));
    }

    stop_listener();
    let _ = runner.join();
    clear_captures();

    let summary = report.join("\n");
    if failed {
        Err(format!("키 주입 셀프테스트 실패\n{summary}"))
    } else {
        Ok(format!("키 주입 셀프테스트 통과\n{summary}"))
    }
}
