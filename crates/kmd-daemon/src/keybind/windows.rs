//! Windows 키 바인딩 백엔드 — WH_KEYBOARD_LL 글로벌 키보드 훅
//!
//! 저수준 키보드 훅으로 키 이벤트를 가로채고,
//! 바인딩 테이블에 따라 키를 리매핑하거나 억제한다.

use super::engine::{EngineState, KeyDecision};
use super::{resolve_launch_cmd, BindAction, KeybindConfig, KeyboardBackend, MacroStep, VKey};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
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

/// 액션 워커로 보내는 채널. FIFO이므로 키 입력 순서가 보존된다.
static ACTION_TX: Mutex<Option<mpsc::Sender<BindAction>>> = Mutex::new(None);

/// 액션을 워커 스레드 큐에 넣는다 (훅 콜백에서 호출 — 블로킹 없음)
fn queue_action(action: BindAction) {
    let sender = ACTION_TX.lock().ok().and_then(|g| g.clone());
    match sender {
        Some(tx) => {
            if tx.send(action).is_err() {
                tracing::warn!("액션 워커가 종료되어 액션을 실행하지 못했습니다");
            }
        }
        None => tracing::warn!("액션 워커 미시작 — 액션 무시"),
    }
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
        // TODO(P2, docs/08): 트리거 down + key down 지속 주입 구현 전까지는
        // plain과 동일하게 물리 키만 통과시킨다
        KeyDecision::EngageChord { trigger, key } => {
            tracing::debug!("chord engage 스텁 (P2/P3 예정): {trigger:?}+{key:?} — plain 통과");
            drop(guard);
            CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param)
        }
        // TODO(P2): 주입 트리거 해제. 스텁에서는 지연 액션만 실행
        KeyDecision::ReleaseChord {
            trigger,
            deferred_action,
        } => {
            tracing::debug!("chord release 스텁 (P2/P3 예정): {trigger:?}");
            drop(guard);
            if let Some(action) = deferred_action {
                queue_action(action);
            }
            1
        }
    }
}

// ── Backend 구현 ────────────────────────────────────────────────────────────

pub struct WindowsKeyboardBackend {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    action_worker: Option<std::thread::JoinHandle<()>>,
}

impl WindowsKeyboardBackend {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
            action_worker: None,
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
        let (action_tx, action_rx) = mpsc::channel::<BindAction>();
        match ACTION_TX.lock() {
            Ok(mut g) => *g = Some(action_tx),
            Err(_) => return Err("액션 큐 잠금 실패".into()),
        }
        self.action_worker = Some(std::thread::spawn(move || {
            for action in action_rx {
                execute_action(&action);
            }
            tracing::debug!("액션 워커 종료");
        }));

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
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                HOOK_THREAD_ID.store(0, Ordering::SeqCst);
                running.store(false, Ordering::Relaxed);
                UnhookWindowsHookEx(hook);
                tracing::info!("키보드 훅 해제 완료");
            }
        });

        self.thread = Some(thread);
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
        // 센더를 드롭하면 워커의 recv 루프가 끝난다
        if let Ok(mut g) = ACTION_TX.lock() {
            *g = None;
        }
        if let Some(worker) = self.action_worker.take() {
            let _ = worker.join();
        }
        Ok(())
    }
}
