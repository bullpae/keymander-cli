//! macOS 키 바인딩 백엔드 — CGEventTap 글로벌 키보드 훅
//!
//! Core Graphics CGEventTap으로 키 이벤트를 가로채고,
//! 바인딩 테이블에 따라 키를 리매핑하거나 억제한다.
//!
//! 필수: 시스템 설정 > 개인 정보 보호 및 보안 > 손쉬운 사용 권한

use super::engine::{EngineState, KeyDecision};
use super::{resolve_launch_cmd, BindAction, KeybindConfig, KeyboardBackend, MacroStep, VKey};
use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

// ── Core Graphics / Core Foundation FFI ──────────────────────────────────────

type CGEventRef = *mut c_void;
type CGEventSourceRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFStringRef = *const c_void;
type CGEventTapProxy = *mut c_void;

const CG_EVENT_KEY_DOWN: u32 = 10;
const CG_EVENT_KEY_UP: u32 = 11;
const CG_EVENT_FLAGS_CHANGED: u32 = 12;
const CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;

const CG_SESSION_EVENT_TAP: u32 = 1;
const CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

const CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x0002_0000;
const CG_EVENT_FLAG_MASK_CONTROL: u64 = 0x0004_0000;
const CG_EVENT_FLAG_MASK_ALTERNATE: u64 = 0x0008_0000;
const CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;

const CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
const CG_EVENT_SOURCE_USER_DATA: u32 = 42;
const CG_EVENT_SOURCE_STATE_PRIVATE: i32 = -1;

/// 자체 생성 이벤트 식별용 매직 넘버
const MAGIC_USER_DATA: i64 = 0x6B6D6400; // "kmd\0"

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    type_: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
#[allow(dead_code)]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventSourceCreate(state_id: i32) -> CGEventSourceRef;
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventSetType(event: CGEventRef, type_: u32);
    /// 실제 HID 수정자 플래그 조회 (state_id=1: kCGEventSourceStateHIDSystemState)
    fn CGEventSourceFlagsState(state_id: i32) -> u64;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: i64,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *mut c_void;
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        c_str: *const u8,
        encoding: u32,
    ) -> *const c_void;
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: i64,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CFRelease(cf: *mut c_void);
    static kCFRunLoopCommonModes: CFStringRef;
    fn CFArrayGetCount(arr: *const c_void) -> i64;
    fn CFArrayGetValueAtIndex(arr: *const c_void, idx: i64) -> *const c_void;
    fn CFStringCompare(s1: *const c_void, s2: *const c_void, opts: u64) -> i32;
}

// ── Carbon TIS (Text Input Source) API — 한/영 입력 소스 전환 ────────────────

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> *mut c_void;
    fn TISGetInputSourceProperty(source: *mut c_void, key: *const c_void) -> *const c_void;
    fn TISCreateInputSourceList(properties: *const c_void, all_installed: bool) -> *mut c_void;
    fn TISSelectInputSource(source: *mut c_void) -> i32;
    static kTISPropertyInputSourceLanguages: *const c_void;
    static kTISPropertyInputSourceType: *const c_void;
    static kTISTypeKeyboardInputMode: *const c_void;
}

// ── VKey → macOS CGKeyCode 변환 ──────────────────────────────────────────────

fn vkey_to_cg(key: VKey) -> u16 {
    match key {
        VKey::A => 0x00,
        VKey::S => 0x01,
        VKey::D => 0x02,
        VKey::F => 0x03,
        VKey::H => 0x04,
        VKey::G => 0x05,
        VKey::Z => 0x06,
        VKey::X => 0x07,
        VKey::C => 0x08,
        VKey::V => 0x09,
        VKey::B => 0x0B,
        VKey::Q => 0x0C,
        VKey::W => 0x0D,
        VKey::E => 0x0E,
        VKey::R => 0x0F,
        VKey::Y => 0x10,
        VKey::T => 0x11,
        VKey::O => 0x1F,
        VKey::U => 0x20,
        VKey::I => 0x22,
        VKey::P => 0x23,
        VKey::L => 0x25,
        VKey::J => 0x26,
        VKey::K => 0x28,
        VKey::N => 0x2D,
        VKey::M => 0x2E,
        VKey::Num0 => 0x1D,
        VKey::Num1 => 0x12,
        VKey::Num2 => 0x13,
        VKey::Num3 => 0x14,
        VKey::Num4 => 0x15,
        VKey::Num5 => 0x17,
        VKey::Num6 => 0x16,
        VKey::Num7 => 0x1A,
        VKey::Num8 => 0x1C,
        VKey::Num9 => 0x19,
        VKey::F1 => 0x7A,
        VKey::F2 => 0x78,
        VKey::F3 => 0x63,
        VKey::F4 => 0x76,
        VKey::F5 => 0x60,
        VKey::F6 => 0x61,
        VKey::F7 => 0x62,
        VKey::F8 => 0x64,
        VKey::F9 => 0x65,
        VKey::F10 => 0x6D,
        VKey::F11 => 0x67,
        VKey::F12 => 0x6F,
        VKey::Escape => 0x35,
        VKey::Tab => 0x30,
        VKey::CapsLock => 0x39,
        VKey::Space => 0x31,
        VKey::Enter => 0x24,
        VKey::Backspace => 0x33,
        VKey::Delete => 0x75,
        VKey::Left => 0x7B,
        VKey::Right => 0x7C,
        VKey::Up => 0x7E,
        VKey::Down => 0x7D,
        VKey::Home => 0x73,
        VKey::End => 0x77,
        VKey::PageUp => 0x74,
        VKey::PageDown => 0x79,
        VKey::Insert => 0x72,
        VKey::PrintScreen => 0x69,
        VKey::ScrollLock => 0x6B,
        VKey::Pause => 0x71,
        VKey::LShift => 0x38,
        VKey::RShift => 0x3C,
        VKey::LCtrl => 0x3B,
        VKey::RCtrl => 0x3E,
        VKey::LAlt => 0x3A,
        VKey::RAlt => 0x3D,
        VKey::LWin => 0x37,
        VKey::RWin => 0x36,
        VKey::Semicolon => 0x29,
        VKey::Quote => 0x27,
        VKey::Comma => 0x2B,
        VKey::Period => 0x2F,
        VKey::Slash => 0x2C,
        VKey::Backslash => 0x2A,
        VKey::LBracket => 0x21,
        VKey::RBracket => 0x1E,
        VKey::Minus => 0x1B,
        VKey::Equal => 0x18,
        VKey::Grave => 0x32,
        VKey::Hangul => 0x68,
        VKey::Hanja => 0x68,
    }
}

fn cg_to_vkey(keycode: u16) -> Option<VKey> {
    match keycode {
        0x00 => Some(VKey::A),
        0x01 => Some(VKey::S),
        0x02 => Some(VKey::D),
        0x03 => Some(VKey::F),
        0x04 => Some(VKey::H),
        0x05 => Some(VKey::G),
        0x06 => Some(VKey::Z),
        0x07 => Some(VKey::X),
        0x08 => Some(VKey::C),
        0x09 => Some(VKey::V),
        0x0B => Some(VKey::B),
        0x0C => Some(VKey::Q),
        0x0D => Some(VKey::W),
        0x0E => Some(VKey::E),
        0x0F => Some(VKey::R),
        0x10 => Some(VKey::Y),
        0x11 => Some(VKey::T),
        0x1F => Some(VKey::O),
        0x20 => Some(VKey::U),
        0x22 => Some(VKey::I),
        0x23 => Some(VKey::P),
        0x25 => Some(VKey::L),
        0x26 => Some(VKey::J),
        0x28 => Some(VKey::K),
        0x2D => Some(VKey::N),
        0x2E => Some(VKey::M),
        0x1D => Some(VKey::Num0),
        0x12 => Some(VKey::Num1),
        0x13 => Some(VKey::Num2),
        0x14 => Some(VKey::Num3),
        0x15 => Some(VKey::Num4),
        0x17 => Some(VKey::Num5),
        0x16 => Some(VKey::Num6),
        0x1A => Some(VKey::Num7),
        0x1C => Some(VKey::Num8),
        0x19 => Some(VKey::Num9),
        0x7A => Some(VKey::F1),
        0x78 => Some(VKey::F2),
        0x63 => Some(VKey::F3),
        0x76 => Some(VKey::F4),
        0x60 => Some(VKey::F5),
        0x61 => Some(VKey::F6),
        0x62 => Some(VKey::F7),
        0x64 => Some(VKey::F8),
        0x65 => Some(VKey::F9),
        0x6D => Some(VKey::F10),
        0x67 => Some(VKey::F11),
        0x6F => Some(VKey::F12),
        0x35 => Some(VKey::Escape),
        0x30 => Some(VKey::Tab),
        0x39 => Some(VKey::CapsLock),
        0x31 => Some(VKey::Space),
        0x24 => Some(VKey::Enter),
        0x33 => Some(VKey::Backspace),
        0x75 => Some(VKey::Delete),
        0x7B => Some(VKey::Left),
        0x7C => Some(VKey::Right),
        0x7E => Some(VKey::Up),
        0x7D => Some(VKey::Down),
        0x73 => Some(VKey::Home),
        0x77 => Some(VKey::End),
        0x74 => Some(VKey::PageUp),
        0x79 => Some(VKey::PageDown),
        0x38 => Some(VKey::LShift),
        0x3C => Some(VKey::RShift),
        0x3B => Some(VKey::LCtrl),
        0x3E => Some(VKey::RCtrl),
        0x3A => Some(VKey::LAlt),
        0x3D => Some(VKey::RAlt),
        0x37 => Some(VKey::LWin),
        0x36 => Some(VKey::RWin),
        0x29 => Some(VKey::Semicolon),
        0x27 => Some(VKey::Quote),
        0x2B => Some(VKey::Comma),
        0x2F => Some(VKey::Period),
        0x2C => Some(VKey::Slash),
        0x2A => Some(VKey::Backslash),
        0x21 => Some(VKey::LBracket),
        0x1E => Some(VKey::RBracket),
        0x1B => Some(VKey::Minus),
        0x18 => Some(VKey::Equal),
        0x32 => Some(VKey::Grave),
        _ => None,
    }
}

// ── 키 시뮬레이션 ────────────────────────────────────────────────────────────

fn modifiers_to_flags(mods: &[VKey]) -> u64 {
    let mut flags = 0u64;
    for m in mods {
        match m {
            VKey::LShift | VKey::RShift => flags |= CG_EVENT_FLAG_MASK_SHIFT,
            VKey::LCtrl | VKey::RCtrl => flags |= CG_EVENT_FLAG_MASK_CONTROL,
            VKey::LAlt | VKey::RAlt => flags |= CG_EVENT_FLAG_MASK_ALTERNATE,
            VKey::LWin | VKey::RWin => flags |= CG_EVENT_FLAG_MASK_COMMAND,
            _ => {}
        }
    }
    flags
}

fn send_key_event(keycode: u16, key_down: bool, flags: u64) {
    unsafe {
        let source = CGEventSourceCreate(CG_EVENT_SOURCE_STATE_PRIVATE);
        if source.is_null() {
            return;
        }
        let event = CGEventCreateKeyboardEvent(source, keycode, key_down);
        if event.is_null() {
            CFRelease(source);
            return;
        }
        CGEventSetFlags(event, flags);
        CGEventSetIntegerValueField(event, CG_EVENT_SOURCE_USER_DATA, MAGIC_USER_DATA);
        CGEventPost(CG_SESSION_EVENT_TAP, event);
        CFRelease(event);
        CFRelease(source);
    }
}

fn send_key_press(keycode: u16) {
    send_key_event(keycode, true, 0);
    send_key_event(keycode, false, 0);
}

fn send_combo(modifier_vkeys: &[VKey], key: VKey) {
    let flags = modifiers_to_flags(modifier_vkeys);
    let kc = vkey_to_cg(key);
    // modifier down → key down → key up → modifier up (Windows 방식과 동일)
    for m in modifier_vkeys {
        send_key_event(vkey_to_cg(*m), true, modifiers_to_flags(&[*m]));
    }
    send_key_event(kc, true, flags);
    send_key_event(kc, false, flags);
    for m in modifier_vkeys.iter().rev() {
        send_key_event(vkey_to_cg(*m), false, 0);
    }
}

// ── 입력 소스 전환 (한/영 토글) ───────────────────────────────────────────────

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

/// macOS 한/영 입력 소스 토글.
///
/// TIS API(TISSelectInputSource)로 CJK 입력기를 선택하면 선택(메뉴바)만 바뀌고
/// 포커스된 앱의 입력 컨텍스트가 따라오지 않는 경우가 있다. 로그상 선택은
/// 매번 attempt=0에 확인되지만 실제 입력은 영문으로 남았다 — 선택 상태
/// 검증으로는 이 반쪽 전환을 감지할 수 없다.
///
/// CapsLock 같은 OS 네이티브 전환은 항상 완전 동기화되므로, 시스템 단축키
/// "이전 입력 소스 선택"(Ctrl+Space, symbolic hotkey 60)을 합성 이벤트로
/// 주입해 동일한 네이티브 경로(HIToolbox)를 태운다. 입력 소스가 2개(영문/한글)
/// 구성에서 "이전 소스" = 토글. IME의 미확정 조합 커밋도 정상 입력 경로로
/// 흘러 글자 순서 뒤섞임을 방지한다. 주입이 반영되지 않으면(단축키 비활성 등)
/// TIS API로 폴백한다.
fn toggle_input_source() {
    let to_korean = unsafe {
        let current = TISCopyCurrentKeyboardInputSource();
        if current.is_null() {
            tracing::warn!("TISCopyCurrentKeyboardInputSource 실패");
            return;
        }
        let is_korean = tis_first_language_matches(current, b"ko\0");
        CFRelease(current);
        !is_korean
    };
    let target: &'static [u8] = if to_korean { b"ko\0" } else { b"en\0" };
    let target_name = if to_korean { "ko" } else { "en" };
    let seq_at_start = INPUT_SEQ.load(Ordering::Relaxed);

    std::thread::spawn(move || unsafe {
        // 물리 수정자(Shift 등)가 해제될 때까지 대기 — Shift가 남은 채 주입하면
        // Ctrl+Shift+Space가 되어 다른 단축키(hotkey 61)로 해석된다. (최대 500ms)
        const MOD_MASK: u64 = CG_EVENT_FLAG_MASK_SHIFT
            | CG_EVENT_FLAG_MASK_CONTROL
            | CG_EVENT_FLAG_MASK_ALTERNATE
            | CG_EVENT_FLAG_MASK_COMMAND;
        for _ in 0..50 {
            if CGEventSourceFlagsState(1) & MOD_MASK == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        tracing::info!("입력 소스 전환: Ctrl+Space 주입 → {target_name}");
        send_combo(&[VKey::LCtrl], VKey::Space);

        // 주입 반영 확인 (네이티브 경로라 선택==실제 적용)
        for attempt in 0..4 {
            std::thread::sleep(std::time::Duration::from_millis(60));
            let cur = TISCopyCurrentKeyboardInputSource();
            if cur.is_null() {
                continue;
            }
            let applied = tis_first_language_matches(cur, target);
            CFRelease(cur);
            if applied {
                tracing::info!("입력 소스 전환 확인: {target_name} (attempt={attempt})");
                return;
            }
        }

        // 주입 미반영 → TIS 폴백. 단 사용자가 이미 타이핑 중이면 조합 보호를
        // 위해 개입하지 않는다 (소스 재선택은 진행 중인 조합을 취소시킨다).
        if INPUT_SEQ.load(Ordering::Relaxed) != seq_at_start {
            tracing::info!("입력 소스 전환: 타이핑 감지 — TIS 폴백 생략 ({target_name})");
            return;
        }
        tracing::warn!("Ctrl+Space 주입 미반영 — TIS 폴백: {target_name}");
        if !tis_select_source_by_language(target) {
            tracing::warn!("입력 소스 전환 실패: {target_name} 소스를 찾을 수 없음");
        }
    });
}

unsafe fn tis_first_language_matches(source: *mut c_void, lang: &[u8]) -> bool {
    let langs = TISGetInputSourceProperty(source, kTISPropertyInputSourceLanguages);
    if langs.is_null() {
        return false;
    }
    if CFArrayGetCount(langs) < 1 {
        return false;
    }
    let first = CFArrayGetValueAtIndex(langs, 0);
    let target =
        CFStringCreateWithCString(std::ptr::null(), lang.as_ptr(), K_CF_STRING_ENCODING_UTF8);
    let eq = CFStringCompare(first, target, 0) == 0;
    CFRelease(target as *mut c_void);
    eq
}

unsafe fn tis_select_source_by_language(lang: &[u8]) -> bool {
    let all = TISCreateInputSourceList(std::ptr::null(), false);
    if all.is_null() {
        return false;
    }
    let count = CFArrayGetCount(all);

    // 언어가 일치하는 후보 중 "입력 모드"(예: 2SetKorean)를 부모 입력기
    // (Keyboard Input Method)보다 우선 선택한다. 한국어 IM은 부모와 모드가
    // 각각 리스트에 오르는데, 부모만 선택하면 모드가 지정되지 않은 반선택
    // 상태가 되어 전환이 겉돌 수 있다.
    let mut chosen: *mut c_void = std::ptr::null_mut();
    let mut fallback: *mut c_void = std::ptr::null_mut();
    for i in 0..count {
        let source = CFArrayGetValueAtIndex(all, i) as *mut c_void;
        if !tis_first_language_matches(source, lang) {
            continue;
        }
        let ty = TISGetInputSourceProperty(source, kTISPropertyInputSourceType);
        if !ty.is_null() && CFStringCompare(ty, kTISTypeKeyboardInputMode, 0) == 0 {
            chosen = source;
            break;
        }
        if fallback.is_null() {
            fallback = source;
        }
    }
    if chosen.is_null() {
        chosen = fallback;
    }

    let mut found = false;
    if !chosen.is_null() {
        let status = TISSelectInputSource(chosen);
        if status == 0 {
            found = true;
        } else {
            tracing::warn!("TISSelectInputSource 실패: status={status}");
        }
    }

    CFRelease(all);
    found
}

// ── 액션 실행 ────────────────────────────────────────────────────────────────

fn execute_action(action: &BindAction) {
    match action {
        BindAction::SendKey(key) => {
            if matches!(key, VKey::Hangul | VKey::Hanja) {
                toggle_input_source();
            } else {
                send_key_press(vkey_to_cg(*key));
            }
        }
        BindAction::SendCombo { modifiers, key } => {
            send_combo(modifiers, *key);
        }
        BindAction::Macro(steps) => {
            for step in steps {
                match step {
                    MacroStep::KeyPress(k) => send_key_event(vkey_to_cg(*k), true, 0),
                    MacroStep::KeyRelease(k) => send_key_event(vkey_to_cg(*k), false, 0),
                    MacroStep::Combo { modifiers, key } => {
                        send_combo(modifiers, *key);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        BindAction::Launch(cmd) => {
            let resolved = resolve_launch_cmd(cmd);
            tracing::info!("프로그램 실행: {resolved}");
            std::thread::spawn(move || {
                // macOS에서 Alt/Space 단축키 직후 즉시 실행하면 modifier 해제/IME 초기 조합
                // 타이밍과 겹쳐 첫 한글 조합이 깨질 수 있다. 짧은 지연 후 실행한다.
                std::thread::sleep(std::time::Duration::from_millis(45));
                if let Err(e) = std::process::Command::new(&resolved).spawn() {
                    tracing::error!("프로그램 실행 실패: {resolved} — {e}");
                }
            });
        }
    }
}

/// 레이어 내 액션 실행 — 트리거 modifier(Alt 등)를 일시 해제한 뒤 액션을 보낸다.
/// 물리적으로 Alt를 누르고 있으면 합성 이벤트(Cmd+Left 등)에 잔여 Alt 플래그가
/// 간섭할 수 있다. 특히 한글 IME 활성 시 이 간섭이 문제가 된다.
///
/// 단순 keyUp이 아닌 flagsChanged(타입 12) 이벤트를 직접 생성하여
/// OS modifier 상태를 확실히 클리어한다.
fn execute_layer_action(action: &BindAction, trigger: VKey) {
    // Launch 액션은 합성 키 조합을 보내지 않으므로, trigger modifier(Alt) 강제 해제가
    // 오히려 런처 최초 IME 조합 컨텍스트를 깨뜨릴 수 있다.
    if matches!(action, BindAction::Launch(_)) {
        execute_action(action);
        return;
    }

    unsafe {
        let source = CGEventSourceCreate(CG_EVENT_SOURCE_STATE_PRIVATE);
        if !source.is_null() {
            let trigger_kc = vkey_to_cg(trigger);
            let ev = CGEventCreateKeyboardEvent(source, trigger_kc, false);
            if !ev.is_null() {
                CGEventSetType(ev, CG_EVENT_FLAGS_CHANGED);
                CGEventSetFlags(ev, 0);
                CGEventSetIntegerValueField(ev, CG_EVENT_SOURCE_USER_DATA, MAGIC_USER_DATA);
                CGEventPost(CG_SESSION_EVENT_TAP, ev);
                CFRelease(ev);
            }
            CFRelease(source);
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(3));

    execute_action(action);
}

// ── 글로벌 상태 ──────────────────────────────────────────────────────────────
//
// 바인딩 판정 로직은 전부 keybind::engine(플랫폼 독립, 단위 테스트 가능)에
// 있다. 이 파일은 CGEventTap 설치/이벤트 변환/액션 실행만 담당한다.

static HOOK_STATE: OnceLock<Arc<Mutex<EngineState>>> = OnceLock::new();

/// CGEventTap 포인터 — 타임아웃 후 재활성화에 사용
static EVENT_TAP_PTR: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

/// 물리 keyDown마다 증가하는 시퀀스. 입력 소스 전환 워커가 "사용자가
/// 타이핑을 시작했는지"를 감지해 재선택(조합 취소 유발)을 중단하는 데 쓴다.
static INPUT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 엔진에 넘길 밀리초 단조 tick (u32 wrapping — 엔진의 wrapping_sub 규약)
fn tick_ms() -> u32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u32
}

/// 엔진 결정 실행 — 레이어 컨텍스트가 있으면 트리거 수정자 해제 경로 사용
fn run_action(action: &BindAction, layer_trigger: Option<VKey>) {
    match layer_trigger {
        Some(trigger) => execute_layer_action(action, trigger),
        None => execute_action(action),
    }
}

// ── CGEventTap 콜백 ─────────────────────────────────────────────────────────

unsafe extern "C" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    type_: u32,
    event: CGEventRef,
    _user_info: *mut c_void,
) -> CGEventRef {
    // null 이벤트 방어 (OS 오류 시)
    if event.is_null() && type_ != CG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        return event;
    }

    // 타임아웃으로 비활성화된 경우 재활성화 + 일시 상태 리셋
    if type_ == CG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        let tap = EVENT_TAP_PTR.load(Ordering::Relaxed);
        if !tap.is_null() {
            tracing::warn!("CGEventTap 타임아웃 감지 — 재활성화 및 상태 리셋");
            CGEventTapEnable(tap, true);
        }
        if let Some(state) = HOOK_STATE.get() {
            if let Ok(mut guard) = state.lock() {
                guard.reset_transient_state();
            }
        }
        return event;
    }

    // keyDown, keyUp, flagsChanged 만 처리
    if type_ != CG_EVENT_KEY_DOWN && type_ != CG_EVENT_KEY_UP && type_ != CG_EVENT_FLAGS_CHANGED {
        return event;
    }

    // 자체 생성 이벤트 패스스루
    if CGEventGetIntegerValueField(event, CG_EVENT_SOURCE_USER_DATA) == MAGIC_USER_DATA {
        return event;
    }

    let keycode = CGEventGetIntegerValueField(event, CG_KEYBOARD_EVENT_KEYCODE) as u16;
    let Some(vkey) = cg_to_vkey(keycode) else {
        return event;
    };

    let state = match HOOK_STATE.get() {
        Some(s) => s,
        None => {
            tracing::warn!("HOOK_STATE 미초기화 — 이벤트 패스스루 (keycode={keycode:#x})");
            return event;
        }
    };
    let mut guard = match state.lock() {
        Ok(g) => g,
        Err(_) => return event,
    };

    let tick = tick_ms();

    // ── flagsChanged 이벤트: 수정자 키를 down/up으로 변환해 엔진에 위임 ──
    if type_ == CG_EVENT_FLAGS_CHANGED {
        // 실제 OS 플래그에서 modifier 상태 판단 (토글 방식 대신)
        let flags = CGEventGetFlags(event);
        let is_down = match vkey {
            VKey::LShift | VKey::RShift => (flags & CG_EVENT_FLAG_MASK_SHIFT) != 0,
            VKey::LCtrl | VKey::RCtrl => (flags & CG_EVENT_FLAG_MASK_CONTROL) != 0,
            VKey::LAlt | VKey::RAlt => (flags & CG_EVENT_FLAG_MASK_ALTERNATE) != 0,
            VKey::LWin | VKey::RWin => (flags & CG_EVENT_FLAG_MASK_COMMAND) != 0,
            VKey::CapsLock => (flags & 0x0001_0000) != 0,
            _ => !guard.is_modifier_held(vkey),
        };

        let decision = guard.process_key(vkey, is_down, tick);

        // 놓친 keyup으로 인한 stuck modifier 방지 — OS 플래그와 동기화
        guard.sync_modifier_flags(
            (flags & CG_EVENT_FLAG_MASK_SHIFT) != 0,
            (flags & CG_EVENT_FLAG_MASK_CONTROL) != 0,
            (flags & CG_EVENT_FLAG_MASK_ALTERNATE) != 0,
            (flags & CG_EVENT_FLAG_MASK_COMMAND) != 0,
        );

        return match decision {
            KeyDecision::PassThrough => {
                drop(guard);
                event
            }
            KeyDecision::Suppress => std::ptr::null_mut(),
            KeyDecision::Execute {
                action,
                layer_trigger,
            } => {
                drop(guard);
                run_action(&action, layer_trigger);
                std::ptr::null_mut()
            }
        };
    }

    // ── keyDown / keyUp 이벤트 ──
    let is_down = type_ == CG_EVENT_KEY_DOWN;

    if is_down {
        // 타이핑 활동 시퀀스 (입력 소스 전환 워커의 재선택 중단 신호)
        INPUT_SEQ.fetch_add(1, Ordering::Relaxed);
    }

    match guard.process_key(vkey, is_down, tick) {
        KeyDecision::PassThrough => {
            drop(guard);
            event
        }
        KeyDecision::Suppress => std::ptr::null_mut(),
        KeyDecision::Execute {
            action,
            layer_trigger,
        } => {
            drop(guard);
            run_action(&action, layer_trigger);
            std::ptr::null_mut()
        }
    }
}

// ── Backend 구현 ─────────────────────────────────────────────────────────────

/// CFRunLoopRef 등 raw pointer를 스레드 간 안전하게 전달하기 위한 래퍼.
/// CFRunLoopStop은 다른 스레드에서 호출해도 안전하다.
struct SendPtr(*mut c_void);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

pub struct MacOSKeyboardBackend {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    run_loop: Arc<Mutex<Option<SendPtr>>>,
}

impl MacOSKeyboardBackend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
            run_loop: Arc::new(Mutex::new(None)),
        }
    }
}

/// 접근성(손쉬운 사용) 권한 확인. 권한이 없으면 시스템 다이얼로그를 띄운다.
fn check_accessibility_permission() -> bool {
    unsafe {
        // kAXTrustedCheckOptionPrompt = "AXTrustedCheckOptionPrompt"
        let key_str = CFStringCreateWithCString(
            std::ptr::null(),
            c"AXTrustedCheckOptionPrompt".as_ptr().cast(),
            0x0800_0100, // kCFStringEncodingUTF8
        );
        if key_str.is_null() {
            return AXIsProcessTrustedWithOptions(std::ptr::null());
        }
        let keys = [key_str];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const u8 as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const u8 as *const c_void,
        );
        let trusted = AXIsProcessTrustedWithOptions(options);
        if !options.is_null() {
            CFRelease(options);
        }
        CFRelease(key_str as *mut c_void);
        trusted
    }
}

impl KeyboardBackend for MacOSKeyboardBackend {
    fn start(&mut self, config: KeybindConfig) -> Result<(), String> {
        if self.running.load(Ordering::Relaxed) {
            return Err("키 바인딩이 이미 실행 중입니다.".into());
        }

        if !check_accessibility_permission() {
            tracing::warn!(
                "접근성 권한이 없습니다. 시스템 설정 > 개인 정보 보호 및 보안 > \
                 손쉬운 사용에서 이 앱을 허용해주세요. (권한 요청 다이얼로그가 표시됩니다)"
            );
        }

        // OnceLock은 최초 1회만 set 가능하므로, 재시작 시에는 기존 Arc 내부의
        // 상태를 새 config로 교체한다 — 그렇지 않으면 stop/start 후에도
        // 이전 설정이 계속 적용된다.
        let new_state = EngineState::new(config);
        if let Some(existing) = HOOK_STATE.get() {
            match existing.lock() {
                Ok(mut guard) => *guard = new_state,
                Err(_) => return Err("훅 상태 잠금 실패".into()),
            }
        } else {
            let _ = HOOK_STATE.set(Arc::new(Mutex::new(new_state)));
        }

        let running = self.running.clone();
        running.store(true, Ordering::Relaxed);
        let rl_store = self.run_loop.clone();

        let thread = std::thread::spawn(move || {
            unsafe {
                let trusted = check_accessibility_permission();
                tracing::info!("AXIsProcessTrusted = {trusted}");

                let events_of_interest: u64 = (1u64 << CG_EVENT_KEY_DOWN)
                    | (1u64 << CG_EVENT_KEY_UP)
                    | (1u64 << CG_EVENT_FLAGS_CHANGED);

                // kCGHIDEventTap(0) — Alt(Option) 키를 시스템 처리 전에 억제해야
                // 특수문자(˙∆˚¬) 생성을 방지할 수 있음
                let mut tap = CGEventTapCreate(
                    0, // kCGHIDEventTap
                    CG_HEAD_INSERT_EVENT_TAP,
                    CG_EVENT_TAP_OPTION_DEFAULT,
                    events_of_interest,
                    event_tap_callback,
                    std::ptr::null_mut(),
                );

                if tap.is_null() {
                    tracing::warn!("kCGHIDEventTap 실패, kCGSessionEventTap 재시도...");
                    tap = CGEventTapCreate(
                        CG_SESSION_EVENT_TAP,
                        CG_HEAD_INSERT_EVENT_TAP,
                        CG_EVENT_TAP_OPTION_DEFAULT,
                        events_of_interest,
                        event_tap_callback,
                        std::ptr::null_mut(),
                    );
                }

                if tap.is_null() {
                    tracing::error!(
                        "CGEventTapCreate 실패 — 접근성(손쉬운 사용) 권한을 확인하세요. \
                         (AXIsProcessTrusted={trusted}, pid={})",
                        std::process::id()
                    );
                    running.store(false, Ordering::Relaxed);
                    return;
                }

                // 타임아웃 후 재활성화를 위해 tap 포인터 저장
                EVENT_TAP_PTR.store(tap, Ordering::Relaxed);

                let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                if source.is_null() {
                    tracing::error!("CFMachPortCreateRunLoopSource 실패");
                    CFRelease(tap);
                    running.store(false, Ordering::Relaxed);
                    return;
                }

                let rl = CFRunLoopGetCurrent();
                CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
                CGEventTapEnable(tap, true);

                if let Ok(mut store) = rl_store.lock() {
                    *store = Some(SendPtr(rl));
                }

                tracing::info!("CGEventTap 키보드 훅 설치 완료");

                CFRunLoopRun();

                CGEventTapEnable(tap, false);
                CFRelease(source);
                CFRelease(tap);

                tracing::info!("CGEventTap 키보드 훅 해제 완료");
            }
        });

        self.thread = Some(thread);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        // stuck modifier 방지: 종료 전에 held 상태인 modifier key-up 전송
        if let Some(state) = HOOK_STATE.get() {
            if let Ok(mut guard) = state.lock() {
                for vkey in guard.held_modifiers() {
                    send_key_event(vkey_to_cg(vkey), false, 0);
                }
                guard.reset_transient_state();
            }
        }

        self.running.store(false, Ordering::Relaxed);
        EVENT_TAP_PTR.store(std::ptr::null_mut(), Ordering::Relaxed);
        if let Ok(store) = self.run_loop.lock() {
            if let Some(ref rl) = *store {
                unsafe {
                    CFRunLoopStop(rl.0);
                }
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}
