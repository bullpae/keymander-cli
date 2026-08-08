//! Windows 키 바인딩 백엔드 — WH_KEYBOARD_LL 글로벌 키보드 훅
//!
//! 저수준 키보드 훅으로 키 이벤트를 가로채고,
//! 바인딩 테이블에 따라 키를 리매핑하거나 억제한다.

use super::engine::{EngineState, KeyDecision};
use super::mouse::{MouseSink, MouseWorker};
use super::{
    resolve_launch_cmd, BindAction, KeybindConfig, KeyboardBackend, MacroStep, MouseBind, VKey,
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// ── 글로벌 상태 (콜백에서 접근) ──────────────────────────────────────────────
//
// 바인딩 판정 로직은 전부 keybind::engine(플랫폼 독립, 단위 테스트 가능)에
// 있다. 이 파일은 OS 훅 설치/이벤트 변환/액션 큐잉만 담당한다.
//
// 액션(SendInput/매크로/Launch)은 전용 워커 스레드에서 실행한다.
// LL 훅 콜백이 LowLevelHooksTimeout(기본 수백 ms)을 초과하면 OS가 훅을
// 조용히 제거하므로, 콜백에서는 채널로 큐잉만 하고 즉시 반환한다.
// SendInput으로 주입된 이벤트는 LLKHF_INJECTED 플래그로 걸러지므로
// 재진입 문제가 없다.

static HOOK_STATE: OnceLock<Arc<Mutex<EngineState>>> = OnceLock::new();

/// 메시지 루프 스레드 ID — stop() 시 WM_QUIT를 보내 GetMessageW를 깨운다
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

// ── 훅 생존 감시 (워치독) ───────────────────────────────────────────────────
//
// Windows는 LL 훅을 **조용히** 제거한다. 콜백이 LowLevelHooksTimeout(기본
// 300ms)을 넘기거나, Modern Standby(S0 저전력 대기) 복귀 과정에서 훅 스레드가
// 스로틀/서스펜드되면 OS가 훅을 체인에서 떼어낸다. 이때 통지는 없다 —
// HHOOK 핸들은 그대로 유효해 보이고, GetMessageW는 계속 블로킹하며,
// 프로세스는 멀쩡히 살아 있다. 결과적으로 "데몬은 실행 중인데 키맵만 죽는"
// 상태가 되고, 재시작 전까지 영구히 복구되지 않았다.
//
// 대응: 훅 콜백이 하트비트를 남기고, 워치독이 "사용자는 입력 중인데 훅은
// 조용한" 상태를 감지하면 무해한 키(VK_NONAME)를 주입해 생존을 확인한 뒤,
// 죽었으면 훅 스레드에 재설치를 요청한다.

/// 훅 콜백이 마지막으로 호출된 시각 (GetTickCount). 콜백 최상단에서 갱신하므로
/// 주입 이벤트로도 갱신된다 — 워치독의 생존 프로브가 성립하는 근거.
static HOOK_LAST_SEEN: AtomicU32 = AtomicU32::new(0);

/// 훅 재설치 누적 횟수 (진단용 — `kmd daemon status`로 노출)
static HOOK_REINSTALLS: AtomicU32 = AtomicU32::new(0);

/// 훅 스레드에 재설치를 요청하는 커스텀 메시지.
/// LL 훅은 설치한 스레드의 메시지 큐로 디스패치되므로 재설치도 그 스레드에서
/// 해야 한다 (다른 스레드에서 SetWindowsHookExW를 부르면 훅이 그쪽에 붙는다).
const WM_KMD_REINSTALL_HOOK: u32 = WM_APP + 0x10;

/// 워치독 점검 주기
const WATCHDOG_INTERVAL_MS: u64 = 2000;

/// 시스템 입력보다 훅 하트비트가 이만큼 뒤처지면 "의심" 상태
const HOOK_STALE_MS: i32 = 1500;

/// 시스템 전체 입력이 이보다 오래 없으면 판단을 보류한다.
/// 사용자가 자리를 비운 상태에서 프로브를 쏘면 유휴 타이머가 리셋돼
/// 화면 꺼짐·절전이 영영 걸리지 않는다 — 이 가드가 그걸 막는다.
const IDLE_SKIP_MS: i32 = 5000;

/// 프로브 최소 간격 — 마우스만 쓰는 구간에서 매 주기 주입하지 않도록 제한
const PROBE_MIN_INTERVAL_MS: i32 = 10_000;

/// 프로브 주입 후 훅 콜백을 기다리는 시간
const PROBE_WAIT_MS: u64 = 120;

/// 훅 재설치 누적 횟수 (진단용)
pub fn hook_reinstall_count() -> u32 {
    HOOK_REINSTALLS.load(Ordering::Relaxed)
}

/// 마지막 훅 이벤트 이후 경과 ms. 훅 미설치면 None.
pub fn hook_idle_ms() -> Option<u32> {
    let last = HOOK_LAST_SEEN.load(Ordering::Relaxed);
    if last == 0 {
        return None;
    }
    Some(unsafe { GetTickCount() }.wrapping_sub(last))
}

/// 시스템 전체 마지막 입력 시각 (키보드+마우스, GetTickCount 기준)
fn last_system_input_tick() -> Option<u32> {
    unsafe {
        let mut lii: LASTINPUTINFO = std::mem::zeroed();
        lii.cbSize = std::mem::size_of::<LASTINPUTINFO>() as u32;
        if GetLastInputInfo(&mut lii) == 0 {
            return None;
        }
        Some(lii.dwTime)
    }
}

/// 워커 스레드 작업 — 일회성 액션 또는 코드(chord) 모드 주입/해제
enum WorkerJob {
    Action(BindAction),
    /// 코드 진입: 트리거 down → 키 down을 원자적으로 주입.
    /// 트리거 up은 [`WorkerJob::ChordRelease`]에서 — 그 사이 OS는
    /// 트리거가 눌린 상태를 유지한다 (Alt+Tab 스위처 등).
    ChordEngage {
        trigger_vk: u16,
        key_vk: u16,
    },
    /// 코드 해제: 트리거 up 주입
    ChordRelease {
        trigger_vk: u16,
    },
    /// 마우스 바인딩 시작 — 이동/휠은 모션 워커, 버튼은 즉시 down 주입
    MouseEngage(MouseBind),
    /// 마우스 바인딩 정지 — 이동/휠 해제, 버튼 up 주입
    MouseRelease(MouseBind),
    /// 활성 마우스 바인딩 전체 정지 (stuck-mouse 방지)
    MouseStopAll,
}

/// 액션 워커로 보내는 채널. FIFO이므로 키 입력 순서가 보존된다.
static ACTION_TX: Mutex<Option<mpsc::Sender<WorkerJob>>> = Mutex::new(None);

/// 작업을 워커 스레드 큐에 넣는다 (훅 콜백에서 호출 — 블로킹 없음)
fn queue_job(job: WorkerJob) {
    let sender = ACTION_TX.lock().ok().and_then(|g| g.clone());
    match sender {
        Some(tx) => {
            if tx.send(job).is_err() {
                tracing::warn!("액션 워커가 종료되어 작업을 실행하지 못했습니다");
            }
        }
        None => tracing::warn!("액션 워커 미시작 — 작업 무시"),
    }
}

fn queue_action(action: BindAction) {
    queue_job(WorkerJob::Action(action));
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

/// Windows VK 코드 → VKey 역변환.
///
/// 정방향 [`vkey_to_vk`](컴파일러가 변형 누락을 잡는 exhaustive match)에서
/// 자동 생성한다 — 거울상 match 두 벌을 손으로 유지하다 어긋나는 것 방지.
/// 정합성은 왕복 테스트(`vk_왕복_변환_일치`)가 보장한다.
fn vk_to_vkey(vk: u16) -> Option<VKey> {
    static REVERSE: OnceLock<std::collections::HashMap<u16, VKey>> = OnceLock::new();
    REVERSE
        .get_or_init(|| VKey::ALL.iter().map(|&k| (vkey_to_vk(k), k)).collect())
        .get(&vk)
        .copied()
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

/// 코드 진입 주입: 트리거 down → 키 down을 **한 번의 SendInput 호출**로 보낸다.
/// 두 이벤트 사이에 다른 입력이 끼어들 수 없어 Alt+Tab 같은 조합의
/// 순서(트리거가 먼저)가 보장된다. 트리거 up은 ChordRelease에서 별도 주입.
fn send_chord_engage(trigger_vk: u16, key_vk: u16) {
    unsafe {
        let mut inputs: [INPUT; 2] = std::mem::zeroed();
        for (i, vk) in [trigger_vk, key_vk].into_iter().enumerate() {
            inputs[i].r#type = INPUT_KEYBOARD;
            inputs[i].Anonymous.ki.wVk = vk;
            inputs[i].Anonymous.ki.wScan = MapVirtualKeyW(vk as u32, 0) as u16;
            inputs[i].Anonymous.ki.dwFlags = if is_extended_vk(vk) {
                KEYEVENTF_EXTENDEDKEY
            } else {
                0
            };
        }
        SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

// ── 마우스 주입 헬퍼 ────────────────────────────────────────────────────────

/// 휠 1노치 델타 (WinUser.h WHEEL_DELTA)
const WHEEL_DELTA_UNIT: i32 = 120;

fn send_mouse_input(dx: i32, dy: i32, mouse_data: i32, flags: u32) {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.r#type = INPUT_MOUSE;
        input.Anonymous.mi.dx = dx;
        input.Anonymous.mi.dy = dy;
        // DWORD 필드지만 휠 델타는 부호 있는 값 — 2의 보수 캐스팅
        input.Anonymous.mi.mouseData = mouse_data as u32;
        input.Anonymous.mi.dwFlags = flags;
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

fn send_mouse_move(dx: i32, dy: i32) {
    send_mouse_input(dx, dy, 0, MOUSEEVENTF_MOVE);
}

fn send_mouse_wheel(notches: i32) {
    send_mouse_input(0, 0, notches * WHEEL_DELTA_UNIT, MOUSEEVENTF_WHEEL);
}

fn send_mouse_button(bind: MouseBind, down: bool) {
    let flags = match (bind, down) {
        (MouseBind::BtnLeft, true) => MOUSEEVENTF_LEFTDOWN,
        (MouseBind::BtnLeft, false) => MOUSEEVENTF_LEFTUP,
        (MouseBind::BtnRight, true) => MOUSEEVENTF_RIGHTDOWN,
        (MouseBind::BtnRight, false) => MOUSEEVENTF_RIGHTUP,
        (MouseBind::BtnMiddle, true) => MOUSEEVENTF_MIDDLEDOWN,
        (MouseBind::BtnMiddle, false) => MOUSEEVENTF_MIDDLEUP,
        _ => return,
    };
    send_mouse_input(0, 0, 0, flags);
}

fn is_mouse_button(bind: MouseBind) -> bool {
    matches!(
        bind,
        MouseBind::BtnLeft | MouseBind::BtnRight | MouseBind::BtnMiddle
    )
}

/// SendInput 기반 [`MouseSink`] — 모션 워커가 틱마다 호출
struct WinMouseSink;

impl MouseSink for WinMouseSink {
    fn move_rel(&mut self, dx: i32, dy: i32) {
        send_mouse_move(dx, dy);
    }
    fn wheel(&mut self, notches: i32) {
        send_mouse_wheel(notches);
    }
}

/// [P3 스파이크] 현재 전경 앱에 Ctrl+V 주입 — 클립보드 붙여넣기 확장의 핵심 능력.
pub(super) fn inject_paste() {
    const VK_CONTROL: u16 = 0x11;
    const VK_V: u16 = 0x56;
    send_combo(&[VK_CONTROL], VK_V);
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

// ── 바인딩 액션 실행 (워커 스레드에서 호출) ─────────────────────────────────

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
        // 상태형 마우스 바인딩이 콤보/리맵/더블탭 등 일회성 경로로 온 경우 —
        // 단발 동작으로 처리 (클릭 1회, 휠 1노치, 짧은 이동)
        BindAction::Mouse(mb) => match mb {
            MouseBind::BtnLeft | MouseBind::BtnRight | MouseBind::BtnMiddle => {
                send_mouse_button(*mb, true);
                send_mouse_button(*mb, false);
            }
            MouseBind::WheelUp => send_mouse_wheel(1),
            MouseBind::WheelDown => send_mouse_wheel(-1),
            MouseBind::MoveUp => send_mouse_move(0, -25),
            MouseBind::MoveDown => send_mouse_move(0, 25),
            MouseBind::MoveLeft => send_mouse_move(-25, 0),
            MouseBind::MoveRight => send_mouse_move(25, 0),
            MouseBind::Slow => {}
        },
    }
}

// ── 키보드 훅 콜백 ──────────────────────────────────────────────────────────

unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    // 생존 하트비트 — 콜백이 호출됐다는 사실 자체가 훅이 체인에 살아 있다는
    // 증거다. 주입(INJECTED) 이벤트와 code<0도 포함해 최상단에서 기록해야
    // 워치독의 프로브가 성립한다. (catch_unwind 밖 — 원자 저장은 패닉 불가)
    HOOK_LAST_SEEN.store(GetTickCount(), Ordering::Relaxed);

    // `extern "system"` 경계를 넘는 패닉은 정의되지 않은 동작이고, 현대 Rust는
    // 이를 감지하면 프로세스를 abort시킨다 — 이 콜백이 유일한 키 이벤트 처리
    // 지점이라 그 순간 키보드 전체가 잠긴다. 어떤 패닉도 여기서 격리하고,
    // 다음 훅으로 이벤트를 넘겨(remap 없이) OS 기본 동작은 살린다.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        keyboard_hook_proc_inner(code, w_param, l_param)
    }))
    .unwrap_or_else(|_| {
        tracing::error!("keyboard_hook_proc 내부 패닉 — 이벤트 통과 (훅 유지)");
        CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param)
    })
}

unsafe fn keyboard_hook_proc_inner(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
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

    // 바인딩 판정은 순수 엔진에 위임 — 이 콜백은 큐잉만 하고 즉시 반환.
    // layer_trigger는 무시한다: Windows SendInput은 물리 modifier가 눌린
    // 상태에서도 합성 이벤트에 간섭하지 않는다 (macOS 전용 컨텍스트).
    match guard.process_key(vkey, is_down, kb.time) {
        KeyDecision::PassThrough => {
            drop(guard);
            CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param)
        }
        KeyDecision::Suppress => 1,
        KeyDecision::Execute { action, .. } => {
            drop(guard);
            queue_action(action);
            1
        }
        // 코드 진입: 물리 이벤트를 억제하고 트리거+키를 묶어 주입.
        // 이후 이 홀드의 키들은 엔진이 PassThrough — OS에는 주입된
        // 트리거가 눌려 있으므로 Alt+Tab 등 조합이 그대로 동작한다.
        KeyDecision::EngageChord { trigger, key } => {
            drop(guard);
            // 어떤 키인지는 로그하지 않는다 — 훅 프로그램의 로그에 실제 타이핑
            // 내용이 남으면 안 된다 (트리거는 config에 있는 값이라 무방)
            tracing::debug!("chord engage: {trigger:?}+미매핑 키");
            queue_job(WorkerJob::ChordEngage {
                trigger_vk: vkey_to_vk(trigger),
                key_vk: vkey_to_vk(key),
            });
            1
        }
        // 코드 해제: 물리 트리거 up을 억제하고 주입 트리거 up으로 대체.
        // 지연 Launch는 해제 뒤에 실행 (FIFO 큐가 순서 보장).
        KeyDecision::ReleaseChord {
            trigger,
            deferred_action,
        } => {
            drop(guard);
            tracing::debug!("chord release: {trigger:?}");
            queue_job(WorkerJob::ChordRelease {
                trigger_vk: vkey_to_vk(trigger),
            });
            if let Some(action) = deferred_action {
                queue_action(action);
            }
            1
        }
        // 마우스 바인딩 — 물리 키 이벤트를 억제하고 워커에 위임
        KeyDecision::MouseEngage(mb) => {
            drop(guard);
            queue_job(WorkerJob::MouseEngage(mb));
            1
        }
        KeyDecision::MouseRelease(mb) => {
            drop(guard);
            queue_job(WorkerJob::MouseRelease(mb));
            1
        }
        KeyDecision::MouseStopAll => {
            drop(guard);
            queue_job(WorkerJob::MouseStopAll);
            1
        }
    }
}

// ── 훅 재설치 / 워치독 ──────────────────────────────────────────────────────

/// 죽은 훅을 떼고 다시 설치한다 (훅 스레드에서만 호출).
///
/// 훅이 사라진 동안의 keyup을 놓쳤을 수 있으므로 엔진의 일시 상태도 함께
/// 리셋한다 — 그러지 않으면 "LAlt를 누른 채 훅이 죽었다가 되살아난" 경우
/// 레이어가 계속 활성으로 남아 모든 키가 화살표로 나간다.
unsafe fn reinstall_hook(hook: &mut HHOOK) {
    // 코드 모드로 주입해 둔 트리거가 있으면 먼저 해제 (stuck-Alt 방지)
    if let Some(state) = HOOK_STATE.get() {
        if let Ok(mut guard) = state.lock() {
            if let Some(trigger) = guard.engaged_chord_trigger() {
                send_key_up(vkey_to_vk(trigger));
            }
            guard.reset_transient_state();
        }
    }

    UnhookWindowsHookEx(*hook);
    let fresh = SetWindowsHookExW(
        WH_KEYBOARD_LL,
        Some(keyboard_hook_proc),
        std::ptr::null_mut(),
        0,
    );
    if fresh.is_null() {
        tracing::error!("키보드 훅 재설치 실패 — SetWindowsHookExW가 NULL 반환");
        return;
    }
    *hook = fresh;
    HOOK_LAST_SEEN.store(GetTickCount(), Ordering::Relaxed);
    let n = HOOK_REINSTALLS.fetch_add(1, Ordering::Relaxed) + 1;
    tracing::warn!(
        "키보드 훅 재설치 완료 (누적 {n}회) — OS가 훅을 조용히 제거했음 \
         (Modern Standby 복귀 / LowLevelHooksTimeout 초과 등)"
    );
}

/// 훅 생존 워치독.
///
/// "사용자는 입력 중인데(GetLastInputInfo가 최신) 우리 훅만 조용하다"를
/// 의심 신호로 삼고, 무해한 키(VK_NONAME = 0xFC, 예약된 no-op)를 주입해
/// 콜백이 도는지 확인한다. 콜백 최상단에서 주입 이벤트로도 하트비트를
/// 남기므로 이 프로브는 훅 생존 여부를 확정적으로 알려준다.
///
/// 프로브는 **시스템에 최근 입력이 있을 때만** 쏜다. 유휴 상태에서 주입하면
/// 유휴 타이머가 리셋돼 화면 꺼짐·절전이 걸리지 않기 때문이다.
fn spawn_hook_watchdog(running: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut last_probe: Option<u32> = None;
        while running.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(WATCHDOG_INTERVAL_MS));
            if !running.load(Ordering::Relaxed) {
                break;
            }

            let now = unsafe { GetTickCount() };
            let last_hook = HOOK_LAST_SEEN.load(Ordering::Relaxed);
            let Some(last_input) = last_system_input_tick() else {
                continue;
            };
            if !should_probe_hook(now, last_hook, last_input, last_probe) {
                continue;
            }
            last_probe = Some(now);

            // ── 생존 프로브 ──
            // 콜백은 주입 이벤트로도 하트비트를 남기므로, 이 주입 뒤에도
            // HOOK_LAST_SEEN이 그대로면 훅이 체인에서 빠진 것이 확정된다.
            let before = HOOK_LAST_SEEN.load(Ordering::Relaxed);
            send_key_press(VK_NONAME);
            std::thread::sleep(std::time::Duration::from_millis(PROBE_WAIT_MS));
            if HOOK_LAST_SEEN.load(Ordering::Relaxed) != before {
                continue; // 살아 있다
            }

            tracing::warn!("키보드 훅 무응답 감지 — 재설치 요청");
            let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
            if tid != 0 {
                unsafe {
                    PostThreadMessageW(tid, WM_KMD_REINSTALL_HOOK, 0, 0);
                }
            }
        }
    })
}

/// 워치독 판정 (순수 함수 — 시각 산술만 담당해 단위 테스트 가능).
///
/// `last_hook == 0`은 훅 미설치를 뜻한다. 모든 시각은 GetTickCount 기반
/// u32로 49.7일마다 랩어라운드하므로, 차이는 `wrapping_sub` 후 `i32`로
/// 해석해 부호를 살린다 (훅이 시스템 입력보다 **앞선** 경우도 음수로 나온다).
fn should_probe_hook(now: u32, last_hook: u32, last_input: u32, last_probe: Option<u32>) -> bool {
    if last_hook == 0 {
        return false; // 훅 미설치 — 판단 대상 아님
    }
    // 훅이 시스템 입력보다 뒤처진 정도
    if (last_input.wrapping_sub(last_hook) as i32) < HOOK_STALE_MS {
        return false; // 정상 — 훅이 최근 입력을 보고 있다
    }
    // 사용자가 자리를 비웠으면 판단 보류. 유휴 상태에서 프로브를 주입하면
    // 시스템 유휴 타이머가 리셋돼 화면 꺼짐·절전이 걸리지 않는다.
    if (now.wrapping_sub(last_input) as i32) > IDLE_SKIP_MS {
        return false;
    }
    // 프로브 레이트 제한 — 마우스만 쓰는 구간에서 매 주기 주입하지 않도록
    match last_probe {
        Some(prev) if (now.wrapping_sub(prev) as i32) < PROBE_MIN_INTERVAL_MS => false,
        _ => true,
    }
}

// ── Backend 구현 ────────────────────────────────────────────────────────────

pub struct WindowsKeyboardBackend {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    action_worker: Option<std::thread::JoinHandle<()>>,
    watchdog: Option<std::thread::JoinHandle<()>>,
}

impl WindowsKeyboardBackend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
            action_worker: None,
            watchdog: None,
        }
    }
}

impl KeyboardBackend for WindowsKeyboardBackend {
    fn start(&mut self, config: KeybindConfig) -> Result<(), String> {
        if self.running.load(Ordering::Relaxed) {
            return Err("키 바인딩이 이미 실행 중입니다.".into());
        }

        // 글로벌 상태 설정 (콜백에서 접근).
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

        // 액션 워커 시작 — 훅 콜백은 큐잉만 하고 실행은 이 스레드가 담당
        let (action_tx, action_rx) = mpsc::channel::<WorkerJob>();
        match ACTION_TX.lock() {
            Ok(mut g) => *g = Some(action_tx),
            Err(_) => return Err("액션 큐 잠금 실패".into()),
        }
        self.action_worker = Some(std::thread::spawn(move || {
            // 모션 워커(이동/휠 틱 스레드)와 버튼 상태는 액션 워커가 소유한다
            let motion = MouseWorker::start(WinMouseSink);
            let mut pressed_buttons: Vec<MouseBind> = Vec::new();

            for job in action_rx {
                match job {
                    WorkerJob::Action(action) => execute_action(&action),
                    WorkerJob::ChordEngage { trigger_vk, key_vk } => {
                        send_chord_engage(trigger_vk, key_vk);
                    }
                    WorkerJob::ChordRelease { trigger_vk } => send_key_up(trigger_vk),
                    WorkerJob::MouseEngage(mb) => {
                        if is_mouse_button(mb) {
                            if !pressed_buttons.contains(&mb) {
                                pressed_buttons.push(mb);
                                send_mouse_button(mb, true);
                            }
                        } else {
                            motion.engage(mb);
                        }
                    }
                    WorkerJob::MouseRelease(mb) => {
                        if is_mouse_button(mb) {
                            if let Some(pos) = pressed_buttons.iter().position(|&b| b == mb) {
                                pressed_buttons.remove(pos);
                                send_mouse_button(mb, false);
                            }
                        } else {
                            motion.release(mb);
                        }
                    }
                    WorkerJob::MouseStopAll => {
                        motion.stop_all();
                        for mb in pressed_buttons.drain(..) {
                            send_mouse_button(mb, false);
                        }
                    }
                }
            }
            // 종료 정리 — 눌린 버튼 해제 (모션은 MouseWorker Drop이 정지)
            for mb in pressed_buttons.drain(..) {
                send_mouse_button(mb, false);
            }
            tracing::debug!("액션 워커 종료");
        }));

        let running = self.running.clone();
        running.store(true, Ordering::Relaxed);

        let thread = std::thread::spawn(move || {
            unsafe {
                let mut hook = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    std::ptr::null_mut(),
                    0,
                );

                if hook.is_null() {
                    super::set_hook_error(Some(
                        "키보드 훅 설치 실패 — SetWindowsHookExW 실패".into(),
                    ));
                    tracing::error!("SetWindowsHookExW 실패");
                    running.store(false, Ordering::Relaxed);
                    return;
                }

                super::set_hook_error(None);
                tracing::info!("키보드 훅 설치 완료");
                HOOK_LAST_SEEN.store(GetTickCount(), Ordering::Relaxed);

                // stop()이 WM_QUIT를 보낼 수 있도록 스레드 ID 공개
                HOOK_THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);

                // 메시지 루프 (훅이 동작하려면 필수).
                // GetMessageW 블로킹 대기 — 기존 PeekMessageW + 1ms sleep은
                // 상시 데몬이 초당 ~1000회 깨어나는 busy-wait였다.
                // WM_QUIT(ret==0) 또는 에러(ret==-1)에서 종료.
                let mut msg: MSG = std::mem::zeroed();
                loop {
                    let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                    if ret == 0 || ret == -1 {
                        break;
                    }
                    // 워치독이 훅 사망을 확인하면 이 메시지로 재설치를 요청한다.
                    // (LL 훅은 설치한 스레드에 묶이므로 반드시 여기서 처리)
                    if msg.message == WM_KMD_REINSTALL_HOOK {
                        reinstall_hook(&mut hook);
                        continue;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                HOOK_THREAD_ID.store(0, Ordering::SeqCst);
                running.store(false, Ordering::Relaxed);
                UnhookWindowsHookEx(hook);
                HOOK_LAST_SEEN.store(0, Ordering::Relaxed);
                tracing::info!("키보드 훅 해제 완료");
            }
        });

        self.thread = Some(thread);
        self.watchdog = Some(spawn_hook_watchdog(self.running.clone()));
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.running.store(false, Ordering::Relaxed);
        // GetMessageW에서 블로킹 중인 메시지 루프를 WM_QUIT로 깨운다
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                PostThreadMessageW(tid, WM_QUIT, 0, 0);
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // 워치독은 running=false를 보고 다음 주기에 빠져나온다
        if let Some(watchdog) = self.watchdog.take() {
            let _ = watchdog.join();
        }
        // 센더를 드롭하면 워커의 recv 루프가 끝난다
        if let Ok(mut g) = ACTION_TX.lock() {
            *g = None;
        }
        if let Some(worker) = self.action_worker.take() {
            let _ = worker.join();
        }
        // 코드 모드 중 중지된 경우 주입돼 있는 트리거를 해제한다 (stuck-Alt 방지).
        // 훅/워커가 모두 종료된 뒤라 여기서 직접 주입해도 경합이 없다.
        if let Some(state) = HOOK_STATE.get() {
            if let Ok(guard) = state.lock() {
                if let Some(trigger) = guard.engaged_chord_trigger() {
                    tracing::info!("중지 시 코드 트리거 해제: {trigger:?}");
                    send_key_up(vkey_to_vk(trigger));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 자동 생성 역변환의 정합성: 모든 VKey가 왕복 변환에서 자기 자신으로
    /// 돌아와야 한다. VK 코드가 겹치면(비단사) 여기서 즉시 실패한다.
    #[test]
    fn vk_왕복_변환_일치() {
        for &k in VKey::ALL {
            assert_eq!(vk_to_vkey(vkey_to_vk(k)), Some(k), "{k:?} 왕복 불일치");
        }
    }

    // ── 훅 워치독 판정 ──
    //
    // OS가 LL 훅을 조용히 제거하는 사고(Modern Standby 복귀 등)를 감지하는
    // 조건식. 시각 산술이 전부라 여기서 검증한다.

    #[test]
    fn 훅이_최근_입력을_보고_있으면_프로브_안함() {
        // 사용자가 방금 친 키를 훅도 봤다 → 정상
        let now = 100_000;
        assert!(!should_probe_hook(now, now - 100, now - 100, None));
    }

    #[test]
    fn 입력은_있는데_훅만_조용하면_프로브() {
        // 시스템은 0.2초 전 입력을 봤는데 훅은 10초째 조용 → 사망 의심
        let now = 100_000;
        assert!(should_probe_hook(now, now - 10_000, now - 200, None));
    }

    #[test]
    fn 훅_미설치면_판단_대상_아님() {
        let now = 100_000;
        assert!(!should_probe_hook(now, 0, now - 200, None));
    }

    #[test]
    fn 자리비움_상태에서는_프로브_금지() {
        // 유휴 상태에서 키를 주입하면 시스템 유휴 타이머가 리셋돼
        // 화면 꺼짐·절전이 영영 안 걸린다 — 반드시 걸러야 한다.
        let now = 100_000;
        let last_input = now - (IDLE_SKIP_MS as u32) - 1_000;
        assert!(!should_probe_hook(
            now,
            last_input - 10_000,
            last_input,
            None
        ));
    }

    #[test]
    fn 프로브_레이트_제한() {
        let now = 100_000;
        let stale_hook = now - 10_000;
        let fresh_input = now - 200;
        // 직전 프로브 직후 → 억제
        assert!(!should_probe_hook(
            now,
            stale_hook,
            fresh_input,
            Some(now - 1_000)
        ));
        // 최소 간격 경과 → 허용
        assert!(should_probe_hook(
            now,
            stale_hook,
            fresh_input,
            Some(now - (PROBE_MIN_INTERVAL_MS as u32) - 1)
        ));
    }

    #[test]
    fn 틱_랩어라운드_구간에서도_정상_판정() {
        // GetTickCount는 49.7일마다 0으로 되감긴다. 그 경계에서 뺄셈이
        // 거대한 양수가 되면 "멀쩡한 훅"을 죽은 것으로 오판한다.
        let now = 500u32; // 랩어라운드 직후
        let last_hook = u32::MAX - 500; // 되감기 직전 (실제로는 1초 전)
        let last_input = u32::MAX - 400;
        assert!(
            !should_probe_hook(now, last_hook, last_input, None),
            "랩어라운드 경계에서 정상 훅을 사망으로 오판하면 안 된다"
        );

        // 같은 경계에서 진짜로 뒤처진 경우는 정상적으로 잡아낸다
        let stale_hook = u32::MAX - 20_000;
        assert!(should_probe_hook(now, stale_hook, now - 200, None));
    }
}
