//! Windows 키 바인딩 백엔드 — WH_KEYBOARD_LL 글로벌 키보드 훅
//!
//! 저수준 키보드 훅으로 키 이벤트를 가로채고,
//! 바인딩 테이블에 따라 키를 리매핑하거나 억제한다.

use super::{
    is_modifier_key, modifier_satisfied, resolve_launch_cmd, BindAction, KeybindConfig,
    KeyboardBackend, MacroStep, VKey,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// ── 글로벌 상태 (콜백에서 접근) ──────────────────────────────────────────────

static HOOK_STATE: OnceLock<Arc<Mutex<HookState>>> = OnceLock::new();

struct HookState {
    config: KeybindConfig,
    keymap_enabled: bool,
    /// 현재 홀드 중인 레이어 트리거 키
    active_layer: Option<usize>,
    /// 레이어 트리거 키가 다운된 시각 (tick count)
    trigger_down_tick: u32,
    /// 레이어 활성 중 다른 키가 눌렸는지 (tap vs hold 판정)
    layer_key_used: bool,
    /// 훅에서 SendInput 호출 시 재진입 방지
    sending: bool,
    /// 현재 물리적으로 눌린 수정자 키 추적
    modifiers_held: HashSet<VKey>,
    /// 더블탭: 마지막으로 탭 완료(keyup)된 키
    last_tap_key: Option<VKey>,
    /// 더블탭: 마지막 탭 시각 (tick count)
    last_tap_tick: u32,
    /// 콤보로 소비된 키 (해당 키의 keyup도 억제)
    combo_consumed_key: Option<VKey>,
    /// 더블탭으로 소비된 키 (해당 키의 keyup도 억제)
    dt_consumed_key: Option<VKey>,
    /// 레이어 내 더블탭: 마지막으로 실행된 키
    layer_dt_last_key: Option<VKey>,
    /// 레이어 내 더블탭: 마지막 실행 시각 (tick count)
    layer_dt_last_tick: u32,
}

fn combo_trigger_matches(
    trigger: &super::ComboTrigger,
    vkey: VKey,
    modifiers_held: &HashSet<VKey>,
) -> bool {
    trigger.key == vkey
        && trigger
            .modifiers
            .iter()
            .all(|m| modifier_satisfied(m, modifiers_held))
}

fn reset_keymap_runtime_state(guard: &mut HookState) {
    guard.active_layer = None;
    guard.layer_key_used = false;
    guard.last_tap_key = None;
    guard.combo_consumed_key = None;
    guard.dt_consumed_key = None;
    guard.layer_dt_last_key = None;
}

// ── VKey → Windows VK 코드 변환 ─────────────────────────────────────────────

fn vkey_to_vk(key: VKey) -> u16 {
    match key {
        VKey::A => 0x41,
        VKey::B => 0x42,
        VKey::C => 0x43,
        VKey::D => 0x44,
        VKey::E => 0x45,
        VKey::F => 0x46,
        VKey::G => 0x47,
        VKey::H => 0x48,
        VKey::I => 0x49,
        VKey::J => 0x4A,
        VKey::K => 0x4B,
        VKey::L => 0x4C,
        VKey::M => 0x4D,
        VKey::N => 0x4E,
        VKey::O => 0x4F,
        VKey::P => 0x50,
        VKey::Q => 0x51,
        VKey::R => 0x52,
        VKey::S => 0x53,
        VKey::T => 0x54,
        VKey::U => 0x55,
        VKey::V => 0x56,
        VKey::W => 0x57,
        VKey::X => 0x58,
        VKey::Y => 0x59,
        VKey::Z => 0x5A,
        VKey::Num0 => 0x30,
        VKey::Num1 => 0x31,
        VKey::Num2 => 0x32,
        VKey::Num3 => 0x33,
        VKey::Num4 => 0x34,
        VKey::Num5 => 0x35,
        VKey::Num6 => 0x36,
        VKey::Num7 => 0x37,
        VKey::Num8 => 0x38,
        VKey::Num9 => 0x39,
        VKey::F1 => VK_F1,
        VKey::F2 => VK_F2,
        VKey::F3 => VK_F3,
        VKey::F4 => VK_F4,
        VKey::F5 => VK_F5,
        VKey::F6 => VK_F6,
        VKey::F7 => VK_F7,
        VKey::F8 => VK_F8,
        VKey::F9 => VK_F9,
        VKey::F10 => VK_F10,
        VKey::F11 => VK_F11,
        VKey::F12 => VK_F12,
        VKey::Escape => VK_ESCAPE,
        VKey::Tab => VK_TAB,
        VKey::CapsLock => VK_CAPITAL,
        VKey::Space => VK_SPACE,
        VKey::Enter => VK_RETURN,
        VKey::Backspace => VK_BACK,
        VKey::Delete => VK_DELETE,
        VKey::Left => VK_LEFT,
        VKey::Right => VK_RIGHT,
        VKey::Up => VK_UP,
        VKey::Down => VK_DOWN,
        VKey::Home => VK_HOME,
        VKey::End => VK_END,
        VKey::PageUp => VK_PRIOR,
        VKey::PageDown => VK_NEXT,
        VKey::Insert => VK_INSERT,
        VKey::PrintScreen => VK_SNAPSHOT,
        VKey::ScrollLock => VK_SCROLL,
        VKey::Pause => VK_PAUSE,
        VKey::LShift => VK_LSHIFT,
        VKey::RShift => VK_RSHIFT,
        VKey::LCtrl => VK_LCONTROL,
        VKey::RCtrl => VK_RCONTROL,
        VKey::LAlt => VK_LMENU,
        VKey::RAlt => VK_RMENU,
        VKey::LWin => VK_LWIN,
        VKey::RWin => VK_RWIN,
        VKey::Semicolon => VK_OEM_1,
        VKey::Quote => VK_OEM_7,
        VKey::Comma => VK_OEM_COMMA,
        VKey::Period => VK_OEM_PERIOD,
        VKey::Slash => VK_OEM_2,
        VKey::Backslash => VK_OEM_5,
        VKey::LBracket => VK_OEM_4,
        VKey::RBracket => VK_OEM_6,
        VKey::Minus => VK_OEM_MINUS,
        VKey::Equal => VK_OEM_PLUS,
        VKey::Grave => VK_OEM_3,
        VKey::Hangul => 0x15,
        VKey::Hanja => 0x19,
    }
}

/// Windows VK 코드 → VKey 변환
fn vk_to_vkey(vk: u16) -> Option<VKey> {
    match vk {
        0x41 => Some(VKey::A),
        0x42 => Some(VKey::B),
        0x43 => Some(VKey::C),
        0x44 => Some(VKey::D),
        0x45 => Some(VKey::E),
        0x46 => Some(VKey::F),
        0x47 => Some(VKey::G),
        0x48 => Some(VKey::H),
        0x49 => Some(VKey::I),
        0x4A => Some(VKey::J),
        0x4B => Some(VKey::K),
        0x4C => Some(VKey::L),
        0x4D => Some(VKey::M),
        0x4E => Some(VKey::N),
        0x4F => Some(VKey::O),
        0x50 => Some(VKey::P),
        0x51 => Some(VKey::Q),
        0x52 => Some(VKey::R),
        0x53 => Some(VKey::S),
        0x54 => Some(VKey::T),
        0x55 => Some(VKey::U),
        0x56 => Some(VKey::V),
        0x57 => Some(VKey::W),
        0x58 => Some(VKey::X),
        0x59 => Some(VKey::Y),
        0x5A => Some(VKey::Z),
        0x30 => Some(VKey::Num0),
        0x31 => Some(VKey::Num1),
        0x32 => Some(VKey::Num2),
        0x33 => Some(VKey::Num3),
        0x34 => Some(VKey::Num4),
        0x35 => Some(VKey::Num5),
        0x36 => Some(VKey::Num6),
        0x37 => Some(VKey::Num7),
        0x38 => Some(VKey::Num8),
        0x39 => Some(VKey::Num9),
        v if v == VK_F1 => Some(VKey::F1),
        v if v == VK_F2 => Some(VKey::F2),
        v if v == VK_F3 => Some(VKey::F3),
        v if v == VK_F4 => Some(VKey::F4),
        v if v == VK_F5 => Some(VKey::F5),
        v if v == VK_F6 => Some(VKey::F6),
        v if v == VK_F7 => Some(VKey::F7),
        v if v == VK_F8 => Some(VKey::F8),
        v if v == VK_F9 => Some(VKey::F9),
        v if v == VK_F10 => Some(VKey::F10),
        v if v == VK_F11 => Some(VKey::F11),
        v if v == VK_F12 => Some(VKey::F12),
        v if v == VK_ESCAPE => Some(VKey::Escape),
        v if v == VK_TAB => Some(VKey::Tab),
        v if v == VK_CAPITAL => Some(VKey::CapsLock),
        v if v == VK_SPACE => Some(VKey::Space),
        v if v == VK_RETURN => Some(VKey::Enter),
        v if v == VK_BACK => Some(VKey::Backspace),
        v if v == VK_DELETE => Some(VKey::Delete),
        v if v == VK_LEFT => Some(VKey::Left),
        v if v == VK_RIGHT => Some(VKey::Right),
        v if v == VK_UP => Some(VKey::Up),
        v if v == VK_DOWN => Some(VKey::Down),
        v if v == VK_HOME => Some(VKey::Home),
        v if v == VK_END => Some(VKey::End),
        v if v == VK_PRIOR => Some(VKey::PageUp),
        v if v == VK_NEXT => Some(VKey::PageDown),
        v if v == VK_INSERT => Some(VKey::Insert),
        v if v == VK_SNAPSHOT => Some(VKey::PrintScreen),
        v if v == VK_SCROLL => Some(VKey::ScrollLock),
        v if v == VK_PAUSE => Some(VKey::Pause),
        v if v == VK_LSHIFT => Some(VKey::LShift),
        v if v == VK_RSHIFT => Some(VKey::RShift),
        v if v == VK_LCONTROL => Some(VKey::LCtrl),
        v if v == VK_RCONTROL => Some(VKey::RCtrl),
        v if v == VK_LMENU => Some(VKey::LAlt),
        v if v == VK_RMENU => Some(VKey::RAlt),
        v if v == VK_LWIN => Some(VKey::LWin),
        v if v == VK_RWIN => Some(VKey::RWin),
        v if v == VK_OEM_1 => Some(VKey::Semicolon),
        v if v == VK_OEM_7 => Some(VKey::Quote),
        v if v == VK_OEM_COMMA => Some(VKey::Comma),
        v if v == VK_OEM_PERIOD => Some(VKey::Period),
        v if v == VK_OEM_2 => Some(VKey::Slash),
        v if v == VK_OEM_5 => Some(VKey::Backslash),
        v if v == VK_OEM_4 => Some(VKey::LBracket),
        v if v == VK_OEM_6 => Some(VKey::RBracket),
        v if v == VK_OEM_MINUS => Some(VKey::Minus),
        v if v == VK_OEM_PLUS => Some(VKey::Equal),
        v if v == VK_OEM_3 => Some(VKey::Grave),
        0x15 => Some(VKey::Hangul),
        0x19 => Some(VKey::Hanja),
        _ => None,
    }
}

// ── SendInput 헬퍼 ──────────────────────────────────────────────────────────

/// LLKHF_INJECTED 플래그 — SendInput으로 주입된 이벤트 식별
const LLKHF_INJECTED: u32 = 0x00000010;

/// 확장 키 여부 판정 — 이 키들은 KEYEVENTF_EXTENDEDKEY 플래그 필수
fn is_extended_vk(vk: u16) -> bool {
    matches!(
        vk,
        VK_UP
            | VK_DOWN
            | VK_LEFT
            | VK_RIGHT
            | VK_HOME
            | VK_END
            | VK_PRIOR
            | VK_NEXT
            | VK_INSERT
            | VK_DELETE
            | VK_RCONTROL
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
            | VK_SNAPSHOT
    )
}

fn send_key_down(vk: u16) {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = vk;
        input.Anonymous.ki.wScan = MapVirtualKeyW(vk as u32, 0) as u16;
        input.Anonymous.ki.dwFlags = if is_extended_vk(vk) {
            KEYEVENTF_EXTENDEDKEY
        } else {
            0
        };
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

fn send_key_up(vk: u16) {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = vk;
        input.Anonymous.ki.wScan = MapVirtualKeyW(vk as u32, 0) as u16;
        input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP
            | if is_extended_vk(vk) {
                KEYEVENTF_EXTENDEDKEY
            } else {
                0
            };
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

fn send_key_press(vk: u16) {
    send_key_down(vk);
    send_key_up(vk);
}

fn send_combo(modifier_vks: &[u16], key_vk: u16) {
    for &m in modifier_vks {
        send_key_down(m);
    }
    send_key_press(key_vk);
    for &m in modifier_vks.iter().rev() {
        send_key_up(m);
    }
}

// ── 수정자 키 판별 / 콤보 매칭 헬퍼 ─────────────────────────────────────────

// ── 바인딩 액션 실행 ────────────────────────────────────────────────────────

/// 가드를 해제하고 액션을 실행한 뒤 다시 sending 플래그를 해제.
/// 훅 콜백 내에서 반복되는 "sending=true → drop → execute → re-lock" 패턴을 통합.
fn dispatch_action(
    mut guard: std::sync::MutexGuard<'_, HookState>,
    state: &Arc<Mutex<HookState>>,
    action: &BindAction,
) {
    guard.sending = true;
    drop(guard);
    execute_action(action);
    if let Ok(mut g) = state.lock() {
        g.sending = false;
    }
}

fn execute_action(action: &BindAction) {
    match action {
        BindAction::SendKey(key) => {
            send_key_press(vkey_to_vk(*key));
        }
        BindAction::SendCombo { modifiers, key } => {
            let mod_vks: Vec<u16> = modifiers.iter().map(|m| vkey_to_vk(*m)).collect();
            send_combo(&mod_vks, vkey_to_vk(*key));
        }
        BindAction::Macro(steps) => {
            for step in steps {
                match step {
                    MacroStep::KeyPress(k) => send_key_down(vkey_to_vk(*k)),
                    MacroStep::KeyRelease(k) => send_key_up(vkey_to_vk(*k)),
                    MacroStep::Combo { modifiers, key } => {
                        let mod_vks: Vec<u16> = modifiers.iter().map(|m| vkey_to_vk(*m)).collect();
                        send_combo(&mod_vks, vkey_to_vk(*key));
                    }
                }
            }
        }
        BindAction::Launch(cmd) => {
            let resolved = resolve_launch_cmd(cmd);
            tracing::info!("프로그램 실행: {resolved}");
            std::thread::spawn(move || {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                if let Err(e) = std::process::Command::new(&resolved)
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
                {
                    tracing::error!("프로그램 실행 실패: {resolved} — {e}");
                }
            });
        }
    }
}

// ── 키보드 훅 콜백 ──────────────────────────────────────────────────────────

unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param);
    }

    let kb = &*(l_param as *const KBDLLHOOKSTRUCT);

    if kb.flags & LLKHF_INJECTED != 0 {
        return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param);
    }

    let is_down = w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize;
    let is_up = w_param == WM_KEYUP as usize || w_param == WM_SYSKEYUP as usize;

    if !is_down && !is_up {
        return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param);
    }

    let vk = kb.vkCode as u16;
    let Some(vkey) = vk_to_vkey(vk) else {
        return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param);
    };

    let state = match HOOK_STATE.get() {
        Some(s) => s,
        None => return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param),
    };

    let mut guard = match state.lock() {
        Ok(g) => g,
        Err(_) => return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param),
    };

    if guard.sending {
        return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param);
    }

    // ── 1. 수정자 키 물리 상태 추적 ──
    if is_modifier_key(&vkey) {
        if is_down {
            guard.modifiers_held.insert(vkey);
        } else {
            guard.modifiers_held.remove(&vkey);
        }
    }

    if is_down && !is_modifier_key(&vkey) {
        if let Some(toggle) = guard.config.toggle_keymap.clone() {
            if combo_trigger_matches(&toggle, vkey, &guard.modifiers_held) {
                let enabled = !guard.keymap_enabled;
                reset_keymap_runtime_state(&mut guard);
                guard.keymap_enabled = enabled;
                guard.combo_consumed_key = Some(vkey);
                tracing::info!(
                    "keymap {}",
                    if guard.keymap_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                return 1;
            }
        }
    }

    // ── 2. 콤보/더블탭으로 소비된 키의 keyup 억제 ──
    if is_up {
        if guard.combo_consumed_key == Some(vkey) {
            guard.combo_consumed_key = None;
            return 1;
        }
        if guard.dt_consumed_key == Some(vkey) {
            guard.dt_consumed_key = None;
            return 1;
        }
    }

    if !guard.keymap_enabled {
        if is_down && !is_modifier_key(&vkey) {
            let combo_action = guard
                .config
                .combos
                .iter()
                .find(|(trigger, action)| {
                    matches!(action, BindAction::Launch(_))
                        && combo_trigger_matches(trigger, vkey, &guard.modifiers_held)
                })
                .map(|(_, action)| action.clone());

            if let Some(action) = combo_action {
                guard.combo_consumed_key = Some(vkey);
                dispatch_action(guard, state, &action);
                return 1;
            }
        }

        drop(guard);
        return CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param);
    }

    // ── 3. 레이어 트리거 키 처리 ──
    for (idx, layer) in guard.config.layers.iter().enumerate() {
        if vkey == layer.trigger {
            if is_down {
                if guard.active_layer.is_none() {
                    tracing::debug!("레이어 활성: {}", layer.name);
                    guard.active_layer = Some(idx);
                    guard.trigger_down_tick = kb.time;
                    guard.layer_key_used = false;
                }
                return 1;
            }
            if is_up && guard.active_layer == Some(idx) {
                let elapsed = kb.time.wrapping_sub(guard.trigger_down_tick);
                let tap_hold_ms = layer.tap_hold_ms;
                let tap_action = layer.tap_action;
                let was_used = guard.layer_key_used;

                guard.active_layer = None;
                guard.layer_key_used = false;

                if !was_used && elapsed < tap_hold_ms {
                    if let Some(tap_key) = tap_action {
                        let action = BindAction::SendKey(tap_key);
                        dispatch_action(guard, state, &action);
                    }
                }
                return 1;
            }
        }
    }

    // ── 4. 활성 레이어 매핑 확인 ──
    if let Some(layer_idx) = guard.active_layer {
        // 4a. 레이어 내 더블탭 매핑 우선 확인
        let dt_opt = guard.config.layers[layer_idx]
            .double_tap_mappings
            .get(&vkey)
            .cloned();

        if let Some(dt) = dt_opt {
            if is_down {
                guard.layer_key_used = true;

                // 이전 탭과 동일 키이고 timeout 이내면 → 더블탭 액션
                if guard.layer_dt_last_key == Some(vkey) {
                    let elapsed = kb.time.wrapping_sub(guard.layer_dt_last_tick);
                    if elapsed < dt.timeout_ms {
                        guard.layer_dt_last_key = None;
                        dispatch_action(guard, state, &dt.double_action);
                        return 1;
                    }
                }

                // 첫 번째 탭 → 싱글 액션 즉시 실행, 더블탭 대기 기록
                guard.layer_dt_last_key = Some(vkey);
                guard.layer_dt_last_tick = kb.time;
                dispatch_action(guard, state, &dt.single_action);
                return 1;
            }
            if is_up {
                return 1;
            }
        }

        // 4b. 일반 레이어 매핑
        let action_opt = guard.config.layers[layer_idx].mappings.get(&vkey).cloned();

        if let Some(action) = action_opt {
            if is_down {
                guard.layer_key_used = true;
                guard.layer_dt_last_key = None;
                dispatch_action(guard, state, &action);
                return 1;
            }
            if is_up {
                return 1;
            }
        }
    }

    // ── 5. 콤보 리맵 확인 (수정자+키 조합) ──
    if is_down && !is_modifier_key(&vkey) {
        let combo_action = guard
            .config
            .combos
            .iter()
            .find(|(trigger, _)| {
                trigger.key == vkey
                    && trigger
                        .modifiers
                        .iter()
                        .all(|m| modifier_satisfied(m, &guard.modifiers_held))
            })
            .map(|(_, action)| action.clone());

        if let Some(action) = combo_action {
            guard.combo_consumed_key = Some(vkey);
            dispatch_action(guard, state, &action);
            return 1;
        }
    }

    // ── 6. 더블탭 확인 ──
    if is_down {
        let dt_binding = guard
            .config
            .double_taps
            .iter()
            .find(|dt| dt.key == vkey)
            .cloned();

        if let Some(dt) = dt_binding {
            if guard.last_tap_key == Some(vkey) {
                let elapsed = kb.time.wrapping_sub(guard.last_tap_tick);
                if elapsed < dt.timeout_ms {
                    guard.last_tap_key = None;
                    guard.dt_consumed_key = Some(vkey);
                    if is_modifier_key(&vkey) {
                        guard.modifiers_held.remove(&vkey);
                    }
                    dispatch_action(guard, state, &dt.action);
                    return 1;
                }
            }
        } else if !is_modifier_key(&vkey) {
            guard.last_tap_key = None;
        }
    }

    // ── 7. 단순 리매핑 확인 ──
    if let Some(action) = guard.config.remaps.get(&vkey).cloned() {
        if is_down {
            dispatch_action(guard, state, &action);
            return 1;
        }
        if is_up {
            return 1;
        }
    }

    // ── 8. 더블탭 상태 기록 (keyup 시 탭 완료 기록) ──
    if is_up {
        let has_dt = guard.config.double_taps.iter().any(|dt| dt.key == vkey);
        if has_dt {
            guard.last_tap_key = Some(vkey);
            guard.last_tap_tick = kb.time;
        }
    }

    drop(guard);
    CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param)
}

// ── Backend 구현 ────────────────────────────────────────────────────────────

pub struct WindowsKeyboardBackend {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WindowsKeyboardBackend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
}

impl KeyboardBackend for WindowsKeyboardBackend {
    fn start(&mut self, config: KeybindConfig) -> Result<(), String> {
        if self.running.load(Ordering::Relaxed) {
            return Err("키 바인딩이 이미 실행 중입니다.".into());
        }

        let state = Arc::new(Mutex::new(HookState {
            config,
            keymap_enabled: true,
            active_layer: None,
            trigger_down_tick: 0,
            layer_key_used: false,
            sending: false,
            modifiers_held: HashSet::new(),
            last_tap_key: None,
            last_tap_tick: 0,
            combo_consumed_key: None,
            dt_consumed_key: None,
            layer_dt_last_key: None,
            layer_dt_last_tick: 0,
        }));

        // 글로벌 상태 설정 (콜백에서 접근)
        let _ = HOOK_STATE.set(state);

        let running = self.running.clone();
        running.store(true, Ordering::Relaxed);

        let thread = std::thread::spawn(move || {
            unsafe {
                let hook = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    std::ptr::null_mut(),
                    0,
                );

                if hook.is_null() {
                    tracing::error!("SetWindowsHookExW 실패");
                    running.store(false, Ordering::Relaxed);
                    return;
                }

                tracing::info!("키보드 훅 설치 완료");

                // 메시지 루프 (훅이 동작하려면 필수)
                let mut msg: MSG = std::mem::zeroed();
                while running.load(Ordering::Relaxed) {
                    let ret = PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE);
                    if ret != 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }

                UnhookWindowsHookEx(hook);
                tracing::info!("키보드 훅 해제 완료");
            }
        });

        self.thread = Some(thread);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}
