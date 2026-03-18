//! 키 바인딩 엔진 — OS-native 키보드 훅 기반 키 리매핑
//!
//! kanata 외부 의존 없이 직접 키 바인딩을 처리한다.
//! Windows: SetWindowsHookEx(WH_KEYBOARD_LL)
//! macOS: CGEventTap (향후)
//! Linux: evdev + uinput (향후)

#[cfg(windows)]
pub mod windows;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── 가상 키 코드 (플랫폼 공용) ──────────────────────────────────────────────

/// 플랫폼 독립적 가상 키 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VKey {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape, Tab, CapsLock, Space, Enter, Backspace, Delete,
    Left, Right, Up, Down,
    Home, End, PageUp, PageDown,
    Insert, PrintScreen, ScrollLock, Pause,
    LShift, RShift, LCtrl, RCtrl, LAlt, RAlt, LWin, RWin,
    Semicolon, Quote, Comma, Period, Slash, Backslash,
    LBracket, RBracket, Minus, Equal, Grave,
    Hangul, Hanja,
}

/// 키 이름 문자열 → VKey 파싱 (config 파일 파싱 시 사용)
impl VKey {
    #[allow(dead_code)]
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim().to_ascii_lowercase();
        match n.as_str() {
            "a" => Some(Self::A), "b" => Some(Self::B), "c" => Some(Self::C),
            "d" => Some(Self::D), "e" => Some(Self::E), "f" => Some(Self::F),
            "g" => Some(Self::G), "h" => Some(Self::H), "i" => Some(Self::I),
            "j" => Some(Self::J), "k" => Some(Self::K), "l" => Some(Self::L),
            "m" => Some(Self::M), "n" => Some(Self::N), "o" => Some(Self::O),
            "p" => Some(Self::P), "q" => Some(Self::Q), "r" => Some(Self::R),
            "s" => Some(Self::S), "t" => Some(Self::T), "u" => Some(Self::U),
            "v" => Some(Self::V), "w" => Some(Self::W), "x" => Some(Self::X),
            "y" => Some(Self::Y), "z" => Some(Self::Z),
            "0" => Some(Self::Num0), "1" => Some(Self::Num1), "2" => Some(Self::Num2),
            "3" => Some(Self::Num3), "4" => Some(Self::Num4), "5" => Some(Self::Num5),
            "6" => Some(Self::Num6), "7" => Some(Self::Num7), "8" => Some(Self::Num8),
            "9" => Some(Self::Num9),
            "f1" => Some(Self::F1), "f2" => Some(Self::F2), "f3" => Some(Self::F3),
            "f4" => Some(Self::F4), "f5" => Some(Self::F5), "f6" => Some(Self::F6),
            "f7" => Some(Self::F7), "f8" => Some(Self::F8), "f9" => Some(Self::F9),
            "f10" => Some(Self::F10), "f11" => Some(Self::F11), "f12" => Some(Self::F12),
            "esc" | "escape" => Some(Self::Escape),
            "tab" => Some(Self::Tab),
            "caps" | "capslock" => Some(Self::CapsLock),
            "space" | "spc" => Some(Self::Space),
            "enter" | "return" | "ret" => Some(Self::Enter),
            "backspace" | "bspc" | "bs" => Some(Self::Backspace),
            "delete" | "del" => Some(Self::Delete),
            "left" => Some(Self::Left), "right" | "rght" => Some(Self::Right),
            "up" => Some(Self::Up), "down" => Some(Self::Down),
            "home" => Some(Self::Home), "end" => Some(Self::End),
            "pageup" | "pgup" => Some(Self::PageUp),
            "pagedown" | "pgdn" => Some(Self::PageDown),
            "insert" | "ins" => Some(Self::Insert),
            "printscreen" | "prtsc" => Some(Self::PrintScreen),
            "scrolllock" => Some(Self::ScrollLock),
            "pause" => Some(Self::Pause),
            "lshift" => Some(Self::LShift), "rshift" => Some(Self::RShift),
            "lctrl" | "lcontrol" => Some(Self::LCtrl), "rctrl" | "rcontrol" => Some(Self::RCtrl),
            "lalt" => Some(Self::LAlt), "ralt" => Some(Self::RAlt),
            "lwin" | "lsuper" | "lmeta" => Some(Self::LWin),
            "rwin" | "rsuper" | "rmeta" => Some(Self::RWin),
            ";" | "semicolon" => Some(Self::Semicolon),
            "'" | "quote" => Some(Self::Quote),
            "," | "comma" => Some(Self::Comma),
            "." | "period" | "dot" => Some(Self::Period),
            "/" | "slash" => Some(Self::Slash),
            "\\" | "backslash" => Some(Self::Backslash),
            "[" | "lbracket" => Some(Self::LBracket),
            "]" | "rbracket" => Some(Self::RBracket),
            "-" | "minus" => Some(Self::Minus),
            "=" | "equal" => Some(Self::Equal),
            "`" | "grave" => Some(Self::Grave),
            "hangul" | "han" | "kor" => Some(Self::Hangul),
            "hanja" => Some(Self::Hanja),
            _ => None,
        }
    }
}

// ── 바인딩 액션 ─────────────────────────────────────────────────────────────

/// 키 바인딩이 트리거되었을 때 수행할 동작
#[derive(Debug, Clone)]
pub enum BindAction {
    /// 다른 키 하나를 전송
    SendKey(VKey),
    /// 수정자+키 조합 전송 (예: Ctrl+C)
    SendCombo { modifiers: Vec<VKey>, key: VKey },
    /// 매크로: 여러 키 순차 전송
    Macro(Vec<MacroStep>),
    /// 외부 프로그램 실행
    Launch(String),
}

/// 매크로 한 스텝
#[derive(Debug, Clone)]
pub enum MacroStep {
    KeyPress(VKey),
    KeyRelease(VKey),
    Combo { modifiers: Vec<VKey>, key: VKey },
}

// ── 수정자 / 콤보 / 더블탭 ──────────────────────────────────────────────────

/// 수정자 키 종류 (좌/우 구분 없이 매칭)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
    Win,
}

/// 수정자+키 조합 트리거 (예: Shift+Space)
#[derive(Debug, Clone)]
pub struct ComboTrigger {
    pub modifiers: Vec<Modifier>,
    pub key: VKey,
}

/// 더블탭 바인딩: 같은 키를 빠르게 두 번 탭하면 액션 실행
#[derive(Debug, Clone)]
pub struct DoubleTapBinding {
    pub key: VKey,
    pub action: BindAction,
    pub timeout_ms: u32,
}

// ── 레이어 ──────────────────────────────────────────────────────────────────

/// 키 레이어: 특정 키를 홀드하면 활성화되는 리매핑 세트
#[derive(Debug, Clone)]
pub struct Layer {
    #[allow(dead_code)]
    pub name: String,
    /// 이 레이어를 활성화하는 트리거 키
    pub trigger: VKey,
    /// 트리거 키를 짧게 탭했을 때 보낼 키 (tap-hold)
    pub tap_action: Option<VKey>,
    /// tap-hold 판정 시간 (밀리초)
    pub tap_hold_ms: u32,
    /// 레이어 활성 시 키 매핑 (즉시 실행)
    pub mappings: HashMap<VKey, BindAction>,
    /// 레이어 내 더블탭 매핑: 첫 탭 → single_action, 두 번째 탭(timeout 이내) → double_action
    pub double_tap_mappings: HashMap<VKey, LayerDoubleTap>,
}

/// 레이어 내 더블탭 설정
#[derive(Debug, Clone)]
pub struct LayerDoubleTap {
    pub single_action: BindAction,
    pub double_action: BindAction,
    pub timeout_ms: u32,
}

// ── 전체 키 바인딩 설정 ─────────────────────────────────────────────────────

/// 키 바인딩 엔진 설정
#[derive(Debug, Clone)]
pub struct KeybindConfig {
    /// 항상 활성화되는 단순 리매핑
    pub remaps: HashMap<VKey, BindAction>,
    /// 레이어 목록
    pub layers: Vec<Layer>,
    /// 수정자+키 콤보 바인딩 (예: Shift+Space → 한영 전환)
    pub combos: Vec<(ComboTrigger, BindAction)>,
    /// 더블탭 바인딩 (예: RShift 두 번 → 한영 전환)
    pub double_taps: Vec<DoubleTapBinding>,
}

impl KeybindConfig {
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            remaps: HashMap::new(),
            layers: Vec::new(),
            combos: Vec::new(),
            double_taps: Vec::new(),
        }
    }

    /// vim-nav 프리셋: Alt 홀드 → Vim 네비게이션
    pub fn vim_nav_preset() -> Self {
        let mut mappings = HashMap::new();
        mappings.insert(VKey::H, BindAction::SendKey(VKey::Left));
        mappings.insert(VKey::J, BindAction::SendKey(VKey::Down));
        mappings.insert(VKey::K, BindAction::SendKey(VKey::Up));
        mappings.insert(VKey::L, BindAction::SendKey(VKey::Right));
        mappings.insert(VKey::N, BindAction::SendKey(VKey::PageUp));
        mappings.insert(VKey::M, BindAction::SendKey(VKey::PageDown));
        mappings.insert(VKey::Period, BindAction::SendKey(VKey::Backspace));
        mappings.insert(VKey::Space, BindAction::Launch("kmd-desktop".into()));
        // y → 줄 복사 (Home, Shift+End, Ctrl+C)
        mappings.insert(VKey::Y, BindAction::Macro(vec![
            MacroStep::KeyPress(VKey::Home),
            MacroStep::KeyRelease(VKey::Home),
            MacroStep::Combo { modifiers: vec![VKey::LShift], key: VKey::End },
            MacroStep::Combo { modifiers: vec![VKey::LCtrl], key: VKey::C },
        ]));
        // p → 붙여넣기 (Ctrl+V)
        mappings.insert(VKey::P, BindAction::SendCombo {
            modifiers: vec![VKey::LCtrl],
            key: VKey::V,
        });
        // / → Delete
        mappings.insert(VKey::Slash, BindAction::SendKey(VKey::Delete));

        // Alt+I/O: 한 번 → 단어 이동, 더블탭 → Home/End
        let mut double_tap_mappings = HashMap::new();
        double_tap_mappings.insert(VKey::I, LayerDoubleTap {
            single_action: BindAction::SendCombo {
                modifiers: vec![VKey::LCtrl],
                key: VKey::Left,
            },
            double_action: BindAction::SendKey(VKey::Home),
            timeout_ms: 300,
        });
        double_tap_mappings.insert(VKey::O, LayerDoubleTap {
            single_action: BindAction::SendCombo {
                modifiers: vec![VKey::LCtrl],
                key: VKey::Right,
            },
            double_action: BindAction::SendKey(VKey::End),
            timeout_ms: 300,
        });

        Self {
            remaps: HashMap::new(),
            layers: vec![Layer {
                name: "nav".into(),
                trigger: VKey::LAlt,
                tap_action: Some(VKey::Escape),
                tap_hold_ms: 200,
                mappings,
                double_tap_mappings,
            }],
            combos: vec![
                (ComboTrigger { modifiers: vec![Modifier::Shift], key: VKey::Space },
                 BindAction::SendKey(VKey::Hangul)),
            ],
            double_taps: vec![
                DoubleTapBinding {
                    key: VKey::RShift,
                    action: BindAction::SendKey(VKey::Hangul),
                    timeout_ms: 300,
                },
            ],
        }
    }

    /// minimal 프리셋: CapsLock → Escape
    pub fn minimal_preset() -> Self {
        let mut remaps = HashMap::new();
        remaps.insert(VKey::CapsLock, BindAction::SendKey(VKey::Escape));
        Self {
            remaps,
            layers: vec![],
            combos: vec![
                (ComboTrigger { modifiers: vec![Modifier::Shift], key: VKey::Space },
                 BindAction::SendKey(VKey::Hangul)),
            ],
            double_taps: vec![
                DoubleTapBinding {
                    key: VKey::RShift,
                    action: BindAction::SendKey(VKey::Hangul),
                    timeout_ms: 300,
                },
            ],
        }
    }
}

// ── Backend trait ────────────────────────────────────────────────────────────

/// 키보드 훅 백엔드 인터페이스 (플랫폼별 구현)
pub trait KeyboardBackend: Send {
    fn start(&mut self, config: KeybindConfig) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
    #[allow(dead_code)]
    fn is_running(&self) -> bool;
}

/// 현재 플랫폼에 맞는 KeyboardBackend 생성
pub fn create_backend() -> Box<dyn KeyboardBackend> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsKeyboardBackend::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(StubBackend)
    }
}

/// 미구현 플랫폼 스텁
#[cfg(not(windows))]
struct StubBackend;

#[cfg(not(windows))]
impl KeyboardBackend for StubBackend {
    fn start(&mut self, _config: KeybindConfig) -> Result<(), String> {
        Err("이 플랫폼에서는 키 바인딩이 아직 지원되지 않습니다.".into())
    }
    fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn is_running(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vkey_from_name() {
        assert_eq!(VKey::from_name("h"), Some(VKey::H));
        assert_eq!(VKey::from_name("LAlt"), Some(VKey::LAlt));
        assert_eq!(VKey::from_name("escape"), Some(VKey::Escape));
        assert_eq!(VKey::from_name("esc"), Some(VKey::Escape));
        assert_eq!(VKey::from_name("pgup"), Some(VKey::PageUp));
        assert_eq!(VKey::from_name("unknown"), None);
    }

    #[test]
    fn test_vim_nav_preset() {
        let config = KeybindConfig::vim_nav_preset();
        assert_eq!(config.layers.len(), 1);
        let layer = &config.layers[0];
        assert_eq!(layer.trigger, VKey::LAlt);
        assert_eq!(layer.tap_action, Some(VKey::Escape));
        assert!(layer.mappings.contains_key(&VKey::H));
        assert!(layer.mappings.contains_key(&VKey::J));
        assert!(!layer.mappings.contains_key(&VKey::I), "I는 double_tap_mappings로 이동");
        assert!(!layer.mappings.contains_key(&VKey::O), "O는 double_tap_mappings로 이동");
        assert!(layer.double_tap_mappings.contains_key(&VKey::I), "Alt+I 더블탭");
        assert!(layer.double_tap_mappings.contains_key(&VKey::O), "Alt+O 더블탭");
        assert_eq!(config.combos.len(), 1, "Shift+Space 콤보");
        assert_eq!(config.double_taps.len(), 1, "RShift 더블탭");
    }

    #[test]
    fn test_minimal_preset() {
        let config = KeybindConfig::minimal_preset();
        assert!(config.remaps.contains_key(&VKey::CapsLock));
        assert!(config.layers.is_empty());
        assert_eq!(config.combos.len(), 1, "Shift+Space 콤보");
        assert_eq!(config.double_taps.len(), 1, "RShift 더블탭");
    }

    #[test]
    fn test_hangul_vkey() {
        assert_eq!(VKey::from_name("hangul"), Some(VKey::Hangul));
        assert_eq!(VKey::from_name("han"), Some(VKey::Hangul));
        assert_eq!(VKey::from_name("kor"), Some(VKey::Hangul));
        assert_eq!(VKey::from_name("hanja"), Some(VKey::Hanja));
    }
}
