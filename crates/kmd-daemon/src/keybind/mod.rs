//! 키 바인딩 엔진 — OS-native 키보드 훅 기반 키 리매핑
//!
//! kanata 외부 의존 없이 직접 키 바인딩을 처리한다.
//! Windows: SetWindowsHookEx(WH_KEYBOARD_LL)
//! macOS: CGEventTap (향후)
//! Linux: evdev + uinput (향후)

pub mod engine;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ── 가상 키 코드 (플랫폼 공용) ──────────────────────────────────────────────

/// 플랫폼 독립적 가상 키 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VKey {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Escape,
    Tab,
    CapsLock,
    Space,
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    PrintScreen,
    ScrollLock,
    Pause,
    LShift,
    RShift,
    LCtrl,
    RCtrl,
    LAlt,
    RAlt,
    LWin,
    RWin,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backslash,
    LBracket,
    RBracket,
    Minus,
    Equal,
    Grave,
    Hangul,
    Hanja,
}

/// 키 이름 문자열 → VKey 파싱 (config 파일 파싱 시 사용)
impl VKey {
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim().to_ascii_lowercase();
        match n.as_str() {
            "a" => Some(Self::A),
            "b" => Some(Self::B),
            "c" => Some(Self::C),
            "d" => Some(Self::D),
            "e" => Some(Self::E),
            "f" => Some(Self::F),
            "g" => Some(Self::G),
            "h" => Some(Self::H),
            "i" => Some(Self::I),
            "j" => Some(Self::J),
            "k" => Some(Self::K),
            "l" => Some(Self::L),
            "m" => Some(Self::M),
            "n" => Some(Self::N),
            "o" => Some(Self::O),
            "p" => Some(Self::P),
            "q" => Some(Self::Q),
            "r" => Some(Self::R),
            "s" => Some(Self::S),
            "t" => Some(Self::T),
            "u" => Some(Self::U),
            "v" => Some(Self::V),
            "w" => Some(Self::W),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            "0" => Some(Self::Num0),
            "1" => Some(Self::Num1),
            "2" => Some(Self::Num2),
            "3" => Some(Self::Num3),
            "4" => Some(Self::Num4),
            "5" => Some(Self::Num5),
            "6" => Some(Self::Num6),
            "7" => Some(Self::Num7),
            "8" => Some(Self::Num8),
            "9" => Some(Self::Num9),
            "f1" => Some(Self::F1),
            "f2" => Some(Self::F2),
            "f3" => Some(Self::F3),
            "f4" => Some(Self::F4),
            "f5" => Some(Self::F5),
            "f6" => Some(Self::F6),
            "f7" => Some(Self::F7),
            "f8" => Some(Self::F8),
            "f9" => Some(Self::F9),
            "f10" => Some(Self::F10),
            "f11" => Some(Self::F11),
            "f12" => Some(Self::F12),
            "esc" | "escape" => Some(Self::Escape),
            "tab" => Some(Self::Tab),
            "caps" | "capslock" => Some(Self::CapsLock),
            "space" | "spc" => Some(Self::Space),
            "enter" | "return" | "ret" => Some(Self::Enter),
            "backspace" | "bspc" | "bs" => Some(Self::Backspace),
            "delete" | "del" => Some(Self::Delete),
            "left" => Some(Self::Left),
            "right" | "rght" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "home" => Some(Self::Home),
            "end" => Some(Self::End),
            "pageup" | "pgup" => Some(Self::PageUp),
            "pagedown" | "pgdn" => Some(Self::PageDown),
            "insert" | "ins" => Some(Self::Insert),
            "printscreen" | "prtsc" => Some(Self::PrintScreen),
            "scrolllock" => Some(Self::ScrollLock),
            "pause" => Some(Self::Pause),
            "lshift" => Some(Self::LShift),
            "rshift" => Some(Self::RShift),
            "lctrl" | "lcontrol" => Some(Self::LCtrl),
            "rctrl" | "rcontrol" => Some(Self::RCtrl),
            "lalt" => Some(Self::LAlt),
            "ralt" => Some(Self::RAlt),
            "lwin" | "lsuper" | "lmeta" | "lcmd" | "cmd" => Some(Self::LWin),
            "rwin" | "rsuper" | "rmeta" | "rcmd" => Some(Self::RWin),
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

// ── 플랫폼별 키 헬퍼 (프리셋 생성 시 사용) ──────────────────────────────────

/// 복사/붙여넣기에 사용할 수정자 (macOS: Cmd, Windows/Linux: Ctrl)
#[cfg(target_os = "macos")]
fn platform_copy_modifier() -> VKey {
    VKey::LWin
}
#[cfg(not(target_os = "macos"))]
fn platform_copy_modifier() -> VKey {
    VKey::LCtrl
}

/// 단어 이동에 사용할 수정자 (macOS: Option, Windows/Linux: Ctrl)
#[cfg(target_os = "macos")]
fn platform_word_modifier() -> VKey {
    VKey::LAlt
}
#[cfg(not(target_os = "macos"))]
fn platform_word_modifier() -> VKey {
    VKey::LCtrl
}

/// 줄 시작 액션 (macOS: Cmd+Left — 한글 IME에서도 안정 동작, Windows/Linux: Home)
#[cfg(target_os = "macos")]
fn platform_home_action() -> BindAction {
    BindAction::SendCombo {
        modifiers: vec![VKey::LWin],
        key: VKey::Left,
    }
}
#[cfg(not(target_os = "macos"))]
fn platform_home_action() -> BindAction {
    BindAction::SendKey(VKey::Home)
}

/// 줄 끝 액션 (macOS: Cmd+Right — 한글 IME에서도 안정 동작, Windows/Linux: End)
#[cfg(target_os = "macos")]
fn platform_end_action() -> BindAction {
    BindAction::SendCombo {
        modifiers: vec![VKey::LWin],
        key: VKey::Right,
    }
}
#[cfg(not(target_os = "macos"))]
fn platform_end_action() -> BindAction {
    BindAction::SendKey(VKey::End)
}

/// 줄 전체 복사 매크로 스텝 (줄 시작 → 줄 끝까지 선택 → 복사)
/// macOS: Cmd+Left → Shift+Cmd+Right → Cmd+C (한글 IME 안정)
/// Windows/Linux: Home → Shift+End → Ctrl+C
#[cfg(target_os = "macos")]
fn line_copy_steps(copy_mod: VKey) -> Vec<MacroStep> {
    vec![
        MacroStep::Combo {
            modifiers: vec![VKey::LWin],
            key: VKey::Left,
        },
        MacroStep::Combo {
            modifiers: vec![VKey::LShift, VKey::LWin],
            key: VKey::Right,
        },
        MacroStep::Combo {
            modifiers: vec![copy_mod],
            key: VKey::C,
        },
    ]
}

/// 줄 전체 삭제 액션
/// macOS: Cmd+Left → Shift+Cmd+Right → Delete (한글 IME 안정)
/// Windows/Linux: Home → Shift+End → Delete
#[cfg(target_os = "macos")]
fn platform_line_delete_action() -> BindAction {
    BindAction::Macro(vec![
        MacroStep::Combo {
            modifiers: vec![VKey::LWin],
            key: VKey::Left,
        },
        MacroStep::Combo {
            modifiers: vec![VKey::LShift, VKey::LWin],
            key: VKey::Right,
        },
        MacroStep::KeyPress(VKey::Delete),
        MacroStep::KeyRelease(VKey::Delete),
    ])
}
#[cfg(not(target_os = "macos"))]
fn platform_line_delete_action() -> BindAction {
    BindAction::Macro(vec![
        MacroStep::KeyPress(VKey::Home),
        MacroStep::KeyRelease(VKey::Home),
        MacroStep::Combo {
            modifiers: vec![VKey::LShift],
            key: VKey::End,
        },
        MacroStep::KeyPress(VKey::Delete),
        MacroStep::KeyRelease(VKey::Delete),
    ])
}
#[cfg(not(target_os = "macos"))]
fn line_copy_steps(copy_mod: VKey) -> Vec<MacroStep> {
    vec![
        MacroStep::KeyPress(VKey::Home),
        MacroStep::KeyRelease(VKey::Home),
        MacroStep::Combo {
            modifiers: vec![VKey::LShift],
            key: VKey::End,
        },
        MacroStep::Combo {
            modifiers: vec![copy_mod],
            key: VKey::C,
        },
    ]
}

// ── Launch 커맨드 경로 해석 ──────────────────────────────────────────────────

/// `launch:name` 액션에서 실행 경로를 결정한다.
/// 실행 파일과 같은 디렉토리에서 먼저 찾고, 없으면 원래 이름을 반환한다.
/// 경로 순회(../) 공격을 방지하기 위해 파일 이름만 허용.
pub fn resolve_launch_cmd(name: &str) -> String {
    let safe_name = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name);

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(windows)]
            let bin = format!("{safe_name}.exe");
            #[cfg(not(windows))]
            let bin = safe_name.to_string();

            let sibling = dir.join(&bin);
            if sibling.exists() {
                return sibling.to_string_lossy().into_owned();
            }
        }
    }
    safe_name.to_string()
}

// ── 수정자 키 헬퍼 (플랫폼 공용) ─────────────────────────────────────────────

pub fn is_modifier_key(vkey: &VKey) -> bool {
    matches!(
        vkey,
        VKey::LShift
            | VKey::RShift
            | VKey::LCtrl
            | VKey::RCtrl
            | VKey::LAlt
            | VKey::RAlt
            | VKey::LWin
            | VKey::RWin
    )
}

pub fn modifier_satisfied(m: &Modifier, held: &HashSet<VKey>) -> bool {
    match m {
        Modifier::Shift => held.contains(&VKey::LShift) || held.contains(&VKey::RShift),
        Modifier::Ctrl => held.contains(&VKey::LCtrl) || held.contains(&VKey::RCtrl),
        Modifier::Alt => held.contains(&VKey::LAlt) || held.contains(&VKey::RAlt),
        Modifier::Win => held.contains(&VKey::LWin) || held.contains(&VKey::RWin),
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
    /// Runtime toggle hotkey. When disabled, this and Launch combos still run.
    pub toggle_keymap: Option<ComboTrigger>,
}

impl KeybindConfig {
    pub fn empty() -> Self {
        Self {
            remaps: HashMap::new(),
            layers: Vec::new(),
            combos: Vec::new(),
            double_taps: Vec::new(),
            toggle_keymap: None,
        }
    }

    /// vim-nav 프리셋: Alt(Option) 홀드 → Vim 네비게이션
    ///
    /// macOS/Windows 차이:
    /// - 단어 이동: macOS=Option+화살표, Windows=Ctrl+화살표
    /// - 복사/붙여넣기: macOS=Cmd+C/V, Windows=Ctrl+C/V
    /// - 줄 시작/끝: macOS=Cmd+화살표, Windows=Home/End
    pub fn vim_nav_preset() -> Self {
        let copy_mod = platform_copy_modifier();
        let word_mod = platform_word_modifier();

        let mut mappings = HashMap::new();
        mappings.insert(VKey::H, BindAction::SendKey(VKey::Left));
        mappings.insert(VKey::J, BindAction::SendKey(VKey::Down));
        mappings.insert(VKey::K, BindAction::SendKey(VKey::Up));
        mappings.insert(VKey::L, BindAction::SendKey(VKey::Right));
        mappings.insert(VKey::N, BindAction::SendKey(VKey::PageUp));
        mappings.insert(VKey::M, BindAction::SendKey(VKey::PageDown));
        mappings.insert(VKey::Period, BindAction::SendKey(VKey::Backspace));
        mappings.insert(VKey::Space, BindAction::Launch("kmd-desktop".into()));
        // y → 줄 복사
        mappings.insert(VKey::Y, BindAction::Macro(line_copy_steps(copy_mod)));
        // p → 붙여넣기
        mappings.insert(
            VKey::P,
            BindAction::SendCombo {
                modifiers: vec![copy_mod],
                key: VKey::V,
            },
        );
        let mut double_tap_mappings = HashMap::new();
        // I: 탭 → 단어 왼쪽, 더블탭 → 줄 시작
        double_tap_mappings.insert(
            VKey::I,
            LayerDoubleTap {
                single_action: BindAction::SendCombo {
                    modifiers: vec![word_mod],
                    key: VKey::Left,
                },
                double_action: platform_home_action(),
                timeout_ms: 300,
            },
        );
        // O: 탭 → 단어 오른쪽, 더블탭 → 줄 끝
        double_tap_mappings.insert(
            VKey::O,
            LayerDoubleTap {
                single_action: BindAction::SendCombo {
                    modifiers: vec![word_mod],
                    key: VKey::Right,
                },
                double_action: platform_end_action(),
                timeout_ms: 300,
            },
        );
        // /: 탭 → Delete, 더블탭 → 현재 줄 삭제
        double_tap_mappings.insert(
            VKey::Slash,
            LayerDoubleTap {
                single_action: BindAction::SendKey(VKey::Delete),
                double_action: platform_line_delete_action(),
                timeout_ms: 300,
            },
        );

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
            combos: default_hangul_combos(),
            double_taps: default_hangul_double_taps(),
            toggle_keymap: None,
        }
    }

    /// minimal 프리셋: CapsLock → Escape
    pub fn minimal_preset() -> Self {
        let mut remaps = HashMap::new();
        remaps.insert(VKey::CapsLock, BindAction::SendKey(VKey::Escape));
        Self {
            remaps,
            layers: vec![],
            combos: default_hangul_combos(),
            double_taps: default_hangul_double_taps(),
            toggle_keymap: None,
        }
    }

    /// base(프리셋)에 custom(사용자 config)을 deep merge.
    /// - remaps/combos/double_taps: custom이 base를 덮어씀
    /// - layers: 같은 name → 키 단위 병합 (custom이 우선), 새 name → 추가
    pub fn merge(mut self, custom: Self) -> Self {
        for (k, v) in custom.remaps {
            self.remaps.insert(k, v);
        }

        for custom_layer in custom.layers {
            if let Some(pos) = self
                .layers
                .iter()
                .position(|layer| layer.name == custom_layer.name)
            {
                let bl = &mut self.layers[pos];
                bl.trigger = custom_layer.trigger;
                if custom_layer.tap_action.is_some() {
                    bl.tap_action = custom_layer.tap_action;
                }
                bl.tap_hold_ms = custom_layer.tap_hold_ms;
                for (k, v) in custom_layer.mappings {
                    bl.mappings.insert(k, v);
                }
                for (k, v) in custom_layer.double_tap_mappings {
                    bl.double_tap_mappings.insert(k, v);
                }
            } else {
                self.layers.push(custom_layer);
            }
        }

        for (trigger, action) in custom.combos {
            if let Some(pos) = self
                .combos
                .iter()
                .position(|(t, _)| t.key == trigger.key && t.modifiers == trigger.modifiers)
            {
                self.combos[pos] = (trigger, action);
            } else {
                self.combos.push((trigger, action));
            }
        }

        for dt in custom.double_taps {
            if let Some(pos) = self.double_taps.iter().position(|x| x.key == dt.key) {
                self.double_taps[pos] = dt;
            } else {
                self.double_taps.push(dt);
            }
        }

        if custom.toggle_keymap.is_some() {
            self.toggle_keymap = custom.toggle_keymap;
        }

        self
    }
}

/// 한/영 전환 콤보 기본값 — 전 플랫폼 공통 Shift+Space.
///
/// Windows는 VKey::Hangul(0x15)을 직접 전송하고, macOS는 execute_action에서
/// TIS API(toggle_input_source)로 입력 소스를 전환한다. 어느 쪽이든 물리적
/// 한/영 키가 없는 60% 키보드에서 동일한 제스처(Shift+Space)를 제공한다.
fn default_hangul_combos() -> Vec<(ComboTrigger, BindAction)> {
    vec![(
        ComboTrigger {
            modifiers: vec![Modifier::Shift],
            key: VKey::Space,
        },
        BindAction::SendKey(VKey::Hangul),
    )]
}

#[cfg(target_os = "windows")]
fn default_hangul_double_taps() -> Vec<DoubleTapBinding> {
    vec![DoubleTapBinding {
        key: VKey::RShift,
        action: BindAction::SendKey(VKey::Hangul),
        timeout_ms: 300,
    }]
}

#[cfg(not(target_os = "windows"))]
fn default_hangul_double_taps() -> Vec<DoubleTapBinding> {
    Vec::new()
}

// ── TOML 설정 → KeybindConfig 변환 ───────────────────────────────────────────

impl KeybindConfig {
    /// kmd-core의 TOML 설정을 런타임 KeybindConfig로 변환
    pub fn from_config(cfg: &kmd_core::config::KeymapConfig) -> Option<Self> {
        let has_custom = !cfg.remaps.is_empty()
            || !cfg.layers.is_empty()
            || !cfg.combos.is_empty()
            || !cfg.double_taps.is_empty();
        if !has_custom {
            return None;
        }

        let mut remaps = HashMap::new();
        for (key_str, action_str) in &cfg.remaps {
            if let (Some(key), Some(action)) = (VKey::from_name(key_str), parse_action(action_str))
            {
                remaps.insert(key, action);
            } else {
                tracing::warn!("리맵 파싱 실패: {key_str} = {action_str}");
            }
        }

        let mut layers = Vec::new();
        for (name, layer_cfg) in &cfg.layers {
            if layer_cfg.trigger.is_empty() {
                tracing::warn!("레이어 '{name}': trigger가 비어있어 건너뜀");
                continue;
            }
            let Some(trigger) = VKey::from_name(&layer_cfg.trigger) else {
                tracing::warn!(
                    "레이어 '{name}': 알 수 없는 trigger '{}'",
                    layer_cfg.trigger
                );
                continue;
            };

            let tap_action = layer_cfg.tap_action.as_deref().and_then(VKey::from_name);

            let mut mappings = HashMap::new();
            for (k, v) in &layer_cfg.mappings {
                if let (Some(key), Some(action)) = (VKey::from_name(k), parse_action(v)) {
                    mappings.insert(key, action);
                } else {
                    tracing::warn!("레이어 '{name}' 매핑 파싱 실패: {k} = {v}");
                }
            }

            let mut double_tap_mappings = HashMap::new();
            for (k, dt) in &layer_cfg.double_taps {
                let Some(key) = VKey::from_name(k) else {
                    tracing::warn!("레이어 '{name}' 더블탭 키 파싱 실패: {k}");
                    continue;
                };
                let (Some(single), Some(double)) =
                    (parse_action(&dt.single), parse_action(&dt.double))
                else {
                    tracing::warn!(
                        "레이어 '{name}' 더블탭 액션 파싱 실패: {k} (single={}, double={})",
                        dt.single,
                        dt.double
                    );
                    continue;
                };
                double_tap_mappings.insert(
                    key,
                    LayerDoubleTap {
                        single_action: single,
                        double_action: double,
                        timeout_ms: dt.timeout_ms.unwrap_or(300),
                    },
                );
            }

            layers.push(Layer {
                name: name.clone(),
                trigger,
                tap_action,
                tap_hold_ms: layer_cfg.tap_hold_ms.unwrap_or(200),
                mappings,
                double_tap_mappings,
            });
        }

        let mut combos = Vec::new();
        for combo_cfg in &cfg.combos {
            if let (Some(trigger), Some(action)) = (
                parse_combo_trigger(&combo_cfg.trigger),
                parse_action(&combo_cfg.action),
            ) {
                combos.push((trigger, action));
            } else {
                tracing::warn!(
                    "콤보 파싱 실패: trigger={}, action={}",
                    combo_cfg.trigger,
                    combo_cfg.action
                );
            }
        }

        let mut double_taps = Vec::new();
        for dt_cfg in &cfg.double_taps {
            if let (Some(key), Some(action)) =
                (VKey::from_name(&dt_cfg.key), parse_action(&dt_cfg.action))
            {
                double_taps.push(DoubleTapBinding {
                    key,
                    action,
                    timeout_ms: dt_cfg.timeout_ms.unwrap_or(300),
                });
            } else {
                tracing::warn!(
                    "더블탭 파싱 실패: key={}, action={}",
                    dt_cfg.key,
                    dt_cfg.action
                );
            }
        }

        tracing::info!(
            "커스텀 키 설정 로드: 리맵 {}개, 레이어 {}개, 콤보 {}개, 더블탭 {}개",
            remaps.len(),
            layers.len(),
            combos.len(),
            double_taps.len()
        );

        Some(Self {
            remaps,
            layers,
            combos,
            double_taps,
            toggle_keymap: None,
        })
    }
}

/// 액션 문자열 파서.
///
/// 지원 형식:
/// - `"Home"` → SendKey(Home)
/// - `"Ctrl+Left"` → SendCombo { mods: [LCtrl], key: Left }
/// - `"Ctrl+Shift+End"` → SendCombo { mods: [LCtrl, LShift], key: End }
/// - `"launch:kmd-desktop"` → Launch("kmd-desktop")
/// - `"macro:Home;Shift+End;Ctrl+C"` → Macro([...])
fn parse_action(s: &str) -> Option<BindAction> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(cmd) = s.strip_prefix("launch:") {
        return Some(BindAction::Launch(cmd.trim().to_string()));
    }

    if let Some(macro_str) = s.strip_prefix("macro:") {
        return parse_macro(macro_str);
    }

    // 수정자+키 콤보 (예: "Ctrl+Left", "Ctrl+Shift+End")
    if s.contains('+') {
        if let Some((mod_strs, key)) = parse_combo_vkeys(s) {
            let modifiers: Vec<VKey> = mod_strs
                .iter()
                .filter_map(|m| parse_modifier_vkey(m.trim()))
                .collect();
            if !modifiers.is_empty() {
                return Some(BindAction::SendCombo { modifiers, key });
            }
        }
    }

    VKey::from_name(s).map(BindAction::SendKey)
}

/// 수정자 이름 → VKey (액션 실행용, L/R 구분)
fn parse_modifier_vkey(name: &str) -> Option<VKey> {
    match name.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(VKey::LCtrl),
        "shift" => Some(VKey::LShift),
        "alt" => Some(VKey::LAlt),
        "win" | "super" | "meta" | "cmd" | "command" => Some(VKey::LWin),
        _ => VKey::from_name(name),
    }
}

/// 수정자 이름 → Modifier (콤보 트리거용, L/R 무관)
fn parse_modifier(name: &str) -> Option<Modifier> {
    match name.to_ascii_lowercase().as_str() {
        "shift" | "lshift" | "rshift" => Some(Modifier::Shift),
        "ctrl" | "control" | "lctrl" | "rctrl" => Some(Modifier::Ctrl),
        "alt" | "lalt" | "ralt" => Some(Modifier::Alt),
        "win" | "super" | "meta" | "cmd" | "command" | "lwin" | "rwin" | "lcmd" | "rcmd" => {
            Some(Modifier::Win)
        }
        _ => None,
    }
}

/// 콤보 트리거 문자열 파서 (예: "Shift+Space", "Alt+Space" → ComboTrigger)
pub fn parse_combo_trigger(s: &str) -> Option<ComboTrigger> {
    let (mod_strs, key) = parse_combo_vkeys(s)?;
    let modifiers: Vec<Modifier> = mod_strs
        .iter()
        .filter_map(|m| parse_modifier(m.trim()))
        .collect();
    if modifiers.is_empty() {
        return None;
    }
    Some(ComboTrigger { modifiers, key })
}

/// 매크로 문자열 파서: "Home;Shift+End;Ctrl+C" → Macro steps
fn parse_macro(s: &str) -> Option<BindAction> {
    let mut steps = Vec::new();
    for part in s.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('+') {
            let (mod_strs, key) = parse_combo_vkeys(part)?;
            let modifiers: Vec<VKey> = mod_strs
                .iter()
                .filter_map(|m| parse_modifier_vkey(m.trim()))
                .collect();
            steps.push(MacroStep::Combo { modifiers, key });
        } else {
            let key = VKey::from_name(part)?;
            steps.push(MacroStep::KeyPress(key));
            steps.push(MacroStep::KeyRelease(key));
        }
    }
    if steps.is_empty() {
        None
    } else {
        Some(BindAction::Macro(steps))
    }
}

fn parse_combo_vkeys(s: &str) -> Option<(Vec<&str>, VKey)> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    if parts.len() < 2 {
        return None;
    }
    let (key_str, mod_strs) = parts.split_last()?;
    if key_str.is_empty() {
        return None;
    }
    if mod_strs.iter().any(|m| m.is_empty()) {
        return None;
    }
    let key = VKey::from_name(key_str)?;
    Some((mod_strs.to_vec(), key))
}

// ── Backend trait ────────────────────────────────────────────────────────────

/// 키보드 훅 백엔드 인터페이스 (플랫폼별 구현)
pub trait KeyboardBackend: Send {
    fn start(&mut self, config: KeybindConfig) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
}

/// 현재 플랫폼에 맞는 KeyboardBackend 생성
pub fn create_backend() -> Box<dyn KeyboardBackend> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsKeyboardBackend::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSKeyboardBackend::new())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Box::new(StubBackend)
    }
}

/// 미구현 플랫폼 스텁
#[cfg(not(any(windows, target_os = "macos")))]
struct StubBackend;

#[cfg(not(any(windows, target_os = "macos")))]
impl KeyboardBackend for StubBackend {
    fn start(&mut self, _config: KeybindConfig) -> Result<(), String> {
        Err("이 플랫폼에서는 키 바인딩이 아직 지원되지 않습니다.".into())
    }
    fn stop(&mut self) -> Result<(), String> {
        Ok(())
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
        assert!(layer.mappings.contains_key(&VKey::N), "Alt+N은 PageUp 매핑");
        assert!(
            !layer.mappings.contains_key(&VKey::Slash),
            "/는 double_tap_mappings로 이동"
        );
        assert!(
            !layer.mappings.contains_key(&VKey::I),
            "I는 double_tap_mappings로 이동"
        );
        assert!(
            !layer.mappings.contains_key(&VKey::O),
            "O는 double_tap_mappings로 이동"
        );
        assert!(
            layer.double_tap_mappings.contains_key(&VKey::I),
            "Alt+I 더블탭"
        );
        assert!(
            layer.double_tap_mappings.contains_key(&VKey::O),
            "Alt+O 더블탭"
        );
        assert!(
            layer.double_tap_mappings.contains_key(&VKey::Slash),
            "Alt+/ 더블탭"
        );
        #[cfg(target_os = "windows")]
        {
            assert_eq!(config.combos.len(), 1, "Shift+Space 콤보");
            assert_eq!(config.double_taps.len(), 1, "RShift 더블탭");
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(
                config.combos.len(),
                1,
                "Shift+Space 한/영 콤보 (크로스플랫폼 통일)"
            );
            assert!(
                config.double_taps.is_empty(),
                "RShift 더블탭은 Windows 전용"
            );
        }
    }

    #[test]
    fn test_minimal_preset() {
        let config = KeybindConfig::minimal_preset();
        assert!(config.remaps.contains_key(&VKey::CapsLock));
        assert!(config.layers.is_empty());
        #[cfg(target_os = "windows")]
        {
            assert_eq!(config.combos.len(), 1, "Shift+Space 콤보 (Windows)");
            assert_eq!(config.double_taps.len(), 1, "RShift 더블탭 (Windows)");
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(
                config.combos.len(),
                1,
                "macOS/Linux: Shift+Space 한/영 콤보 (크로스플랫폼 통일)"
            );
            assert!(
                config.double_taps.is_empty(),
                "RShift 더블탭은 Windows 전용"
            );
        }
    }

    #[test]
    fn test_hangul_vkey() {
        assert_eq!(VKey::from_name("hangul"), Some(VKey::Hangul));
        assert_eq!(VKey::from_name("han"), Some(VKey::Hangul));
        assert_eq!(VKey::from_name("kor"), Some(VKey::Hangul));
        assert_eq!(VKey::from_name("hanja"), Some(VKey::Hanja));
    }

    #[test]
    fn test_parse_action_simple() {
        assert!(matches!(
            parse_action("Home"),
            Some(BindAction::SendKey(VKey::Home))
        ));
        assert!(matches!(
            parse_action("Escape"),
            Some(BindAction::SendKey(VKey::Escape))
        ));
        assert!(parse_action("").is_none());
        assert!(parse_action("nonexistent").is_none());
    }

    #[test]
    fn test_parse_action_combo() {
        let action = parse_action("Ctrl+Left").unwrap();
        match action {
            BindAction::SendCombo { modifiers, key } => {
                assert_eq!(modifiers, vec![VKey::LCtrl]);
                assert_eq!(key, VKey::Left);
            }
            _ => panic!("Ctrl+Left -> SendCombo 예상"),
        }

        let action = parse_action("Ctrl+Shift+End").unwrap();
        match action {
            BindAction::SendCombo { modifiers, key } => {
                assert_eq!(modifiers.len(), 2);
                assert_eq!(key, VKey::End);
            }
            _ => panic!("Ctrl+Shift+End -> SendCombo 예상"),
        }

        assert!(parse_action("Ctrl+").is_none(), "키 누락 콤보는 실패");
        assert!(parse_action("+End").is_none(), "수정자 누락 콤보는 실패");
    }

    #[test]
    fn test_parse_action_launch() {
        let action = parse_action("launch:kmd-desktop").unwrap();
        match action {
            BindAction::Launch(cmd) => assert_eq!(cmd, "kmd-desktop"),
            _ => panic!("launch 예상"),
        }
    }

    #[test]
    fn test_parse_action_macro() {
        let action = parse_action("macro:Home;Shift+End;Ctrl+C").unwrap();
        match action {
            BindAction::Macro(steps) => {
                assert_eq!(steps.len(), 4); // Home(press+release) + Shift+End(combo) + Ctrl+C(combo)
            }
            _ => panic!("macro 예상"),
        }
    }

    #[test]
    fn test_parse_combo_trigger() {
        let trigger = parse_combo_trigger("Shift+Space").unwrap();
        assert_eq!(trigger.modifiers, vec![Modifier::Shift]);
        assert_eq!(trigger.key, VKey::Space);

        assert!(
            parse_combo_trigger("Space").is_none(),
            "수정자 없는 트리거는 실패"
        );
        assert!(
            parse_combo_trigger("Shift+").is_none(),
            "키가 비어 있으면 실패"
        );
        assert!(
            parse_combo_trigger("+Space").is_none(),
            "수정자가 비어 있으면 실패"
        );
    }

    #[test]
    fn test_from_config_custom() {
        use kmd_core::config::*;
        let mut keymap = KeymapConfig::default();
        keymap.active_profile = "custom".to_string();
        keymap.remaps.insert("CapsLock".into(), "Escape".into());
        keymap.combos.push(ComboToml {
            trigger: "Shift+Space".into(),
            action: "Hangul".into(),
        });
        keymap.double_taps.push(DoubleTapToml {
            key: "RShift".into(),
            action: "Hangul".into(),
            timeout_ms: Some(300),
        });

        let mut layer = LayerToml::default();
        layer.trigger = "LAlt".into();
        layer.tap_action = Some("Escape".into());
        layer.mappings.insert("H".into(), "Left".into());
        layer.mappings.insert("J".into(), "Down".into());
        layer.double_taps.insert(
            "I".into(),
            LayerDoubleTapToml {
                single: "Ctrl+Left".into(),
                double: "Home".into(),
                timeout_ms: Some(300),
            },
        );
        keymap.layers.insert("nav".into(), layer);

        let config = KeybindConfig::from_config(&keymap).unwrap();
        assert!(config.remaps.contains_key(&VKey::CapsLock));
        assert_eq!(config.layers.len(), 1);
        assert_eq!(config.layers[0].trigger, VKey::LAlt);
        assert!(config.layers[0].mappings.contains_key(&VKey::H));
        assert!(config.layers[0].double_tap_mappings.contains_key(&VKey::I));
        assert_eq!(config.combos.len(), 1);
        assert_eq!(config.double_taps.len(), 1);
    }
}
