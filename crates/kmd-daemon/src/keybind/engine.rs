//! 키 바인딩 결정 엔진 — OS 훅과 분리된 순수 상태 머신
//!
//! 플랫폼 훅 콜백(unsafe, 테스트 불가)에서 로직을 분리해 단위 테스트를
//! 가능하게 한다. 입력은 (VKey, down/up, tick_ms), 출력은 [`KeyDecision`].
//! 타이밍은 호출자가 넘겨주는 밀리초 tick(u32, wrapping)만 사용하므로
//! 테스트에서 시간을 자유롭게 시뮬레이션할 수 있다.

use std::collections::HashSet;

use super::{
    is_modifier_key, modifier_satisfied, BindAction, ComboTrigger, KeybindConfig, UnmappedBehavior,
    VKey,
};

/// 키 이벤트 처리 결정
#[derive(Debug, Clone)]
pub enum KeyDecision {
    /// OS에 그대로 전달
    PassThrough,
    /// 이벤트 억제 (실행할 액션 없음)
    Suppress,
    /// 이벤트 억제 + 액션 실행.
    ///
    /// `layer_trigger`는 활성 레이어 매핑에서 발동된 경우 그 레이어의
    /// 트리거 키다. macOS는 물리적으로 눌린 트리거 수정자(Alt 등)의 잔여
    /// 플래그가 합성 이벤트에 간섭하므로, 실행 전에 해제해야 한다.
    Execute {
        action: BindAction,
        layer_trigger: Option<VKey>,
    },
    /// 코드(chord) 모드 진입 — 레이어 패스쓰루에서 미매핑 키가 눌렸다.
    ///
    /// 어댑터는 물리 이벤트를 억제하고 `trigger` down → `key` down을
    /// 순서대로 주입해 OS가 트리거 조합(Alt+Tab 등)으로 인식하게 한다.
    /// 이후 홀드가 끝날 때까지 후속 키는 `PassThrough`로 전달된다
    /// (OS 쪽에는 주입된 트리거가 눌려 있는 상태).
    EngageChord { trigger: VKey, key: VKey },
    /// 코드 모드 종료 — 어댑터는 물리 이벤트를 억제하고 `trigger` up을
    /// 주입한다 (Alt+Tab 스위처 확정 등). `deferred_action`은 코드 진입
    /// 전에 지연돼 있던 레이어 Launch가 있으면 트리거 해제 후 실행한다.
    ReleaseChord {
        trigger: VKey,
        deferred_action: Option<BindAction>,
    },
}

impl KeyDecision {
    fn execute(action: BindAction) -> Self {
        Self::Execute {
            action,
            layer_trigger: None,
        }
    }

    fn execute_in_layer(action: BindAction, trigger: VKey) -> Self {
        Self::Execute {
            action,
            layer_trigger: Some(trigger),
        }
    }
}

fn combo_trigger_matches(
    trigger: &ComboTrigger,
    vkey: VKey,
    modifiers_held: &HashSet<VKey>,
) -> bool {
    trigger.key == vkey
        && trigger
            .modifiers
            .iter()
            .all(|m| modifier_satisfied(m, modifiers_held))
}

/// 키 바인딩 런타임 상태 (플랫폼 공용)
pub struct EngineState {
    pub config: KeybindConfig,
    keymap_enabled: bool,
    /// 현재 홀드 중인 레이어 트리거 키
    active_layer: Option<usize>,
    /// 레이어 트리거 키가 다운된 시각 (tick count)
    trigger_down_tick: u32,
    /// 레이어 활성 중 다른 키가 눌렸는지 (tap vs hold 판정)
    layer_key_used: bool,
    /// 레이어 매핑의 Launch 액션은 트리거 키를 뗄 때까지 지연 실행한다.
    /// 트리거 수정자(Alt 등)가 눌린 채 실행하면 새 프로세스의 초기
    /// 포커스/IME 조합에 간섭한다 (0.5.0 "Layer Launch deferral").
    pending_layer_launch: Option<BindAction>,
    /// 현재 물리적으로 눌린 수정자 키 추적
    modifiers_held: HashSet<VKey>,
    /// 홀드 중 다른 키와 조합되어 "수정자로 사용된" 키 집합.
    /// 사용된 수정자의 keyup은 더블탭의 탭으로 기록하지 않는다.
    /// (예: RShift+ㅅ=ㅆ 입력 직후 RShift+/=? 가 더블탭 오판정되는 것 방지)
    mods_used_while_held: HashSet<VKey>,
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
    /// 패스쓰루 코드(chord) 모드 — 트리거가 OS에 주입된 상태.
    /// true인 동안 모든 키는 OS 조합으로 통과하고, 트리거 up에서
    /// [`KeyDecision::ReleaseChord`]로 해제된다 (A안, docs/08 참조).
    chord_engaged: bool,
}

impl EngineState {
    pub fn new(config: KeybindConfig) -> Self {
        Self {
            config,
            keymap_enabled: true,
            active_layer: None,
            trigger_down_tick: 0,
            layer_key_used: false,
            pending_layer_launch: None,
            modifiers_held: HashSet::new(),
            mods_used_while_held: HashSet::new(),
            last_tap_key: None,
            last_tap_tick: 0,
            combo_consumed_key: None,
            dt_consumed_key: None,
            layer_dt_last_key: None,
            layer_dt_last_tick: 0,
            chord_engaged: false,
        }
    }

    /// keymap toggle 등으로 상태를 초기화할 때 호출
    fn reset_runtime_state(&mut self) {
        self.active_layer = None;
        self.layer_key_used = false;
        self.pending_layer_launch = None;
        self.mods_used_while_held.clear();
        self.last_tap_key = None;
        self.combo_consumed_key = None;
        self.dt_consumed_key = None;
        self.layer_dt_last_key = None;
        self.chord_engaged = false;
    }

    /// 코드 모드가 걸린 상태에서 상태 초기화가 필요할 때, 주입된 트리거를
    /// 해제할 [`KeyDecision::ReleaseChord`]가 필요한지 확인한다.
    /// (keymap 토글 등이 코드 모드를 끊을 때 stuck-modifier 방지)
    fn take_engaged_chord_trigger(&mut self) -> Option<VKey> {
        if !self.chord_engaged {
            return None;
        }
        let trigger = self
            .active_layer
            .and_then(|idx| self.config.layers.get(idx))
            .map(|l| l.trigger);
        self.chord_engaged = false;
        trigger
    }

    // ── macOS 어댑터용 헬퍼 ──────────────────────────────────────────────
    // (Windows는 훅 이벤트만으로 상태가 완결되므로 사용하지 않는다)

    /// 훅/탭이 타임아웃 등으로 이벤트를 놓쳤을 때 일시 상태 전체 초기화
    #[allow(dead_code)]
    pub fn reset_transient_state(&mut self) {
        self.modifiers_held.clear();
        self.reset_runtime_state();
    }

    /// 해당 키가 현재 눌린 수정자로 추적 중인지 (flagsChanged is_down 판정 폴백)
    #[allow(dead_code)]
    pub fn is_modifier_held(&self, vkey: VKey) -> bool {
        self.modifiers_held.contains(&vkey)
    }

    /// 현재 눌린 것으로 추적 중인 수정자 목록 (stop 시 stuck-modifier 해제용)
    #[allow(dead_code)]
    pub fn held_modifiers(&self) -> Vec<VKey> {
        self.modifiers_held.iter().copied().collect()
    }

    /// OS가 보고한 수정자 플래그와 내부 추적 상태를 동기화.
    /// 플래그가 꺼진 수정자는 Left/Right 키를 모두 제거한다
    /// (macOS flagsChanged — 놓친 keyup으로 인한 stuck modifier 방지).
    #[allow(dead_code)]
    pub fn sync_modifier_flags(&mut self, shift: bool, ctrl: bool, alt: bool, win: bool) {
        if !shift {
            self.modifiers_held.remove(&VKey::LShift);
            self.modifiers_held.remove(&VKey::RShift);
        }
        if !ctrl {
            self.modifiers_held.remove(&VKey::LCtrl);
            self.modifiers_held.remove(&VKey::RCtrl);
        }
        if !alt {
            self.modifiers_held.remove(&VKey::LAlt);
            self.modifiers_held.remove(&VKey::RAlt);
        }
        if !win {
            self.modifiers_held.remove(&VKey::LWin);
            self.modifiers_held.remove(&VKey::RWin);
        }
    }

    /// 키 이벤트 하나를 처리하고 억제/실행 여부를 결정한다.
    ///
    /// `tick`은 밀리초 단위 단조 증가 카운터 (wrapping 허용 —
    /// Windows의 GetTickCount / KBDLLHOOKSTRUCT.time과 동일 규약).
    pub fn process_key(&mut self, vkey: VKey, is_down: bool, tick: u32) -> KeyDecision {
        let is_up = !is_down;

        // ── 1. 수정자 키 물리 상태 추적 ──
        if is_modifier_key(&vkey) {
            if is_down {
                // 새 수정자가 합류하면 기존 홀드 중 수정자들은 "사용됨"으로 표시
                let held: Vec<VKey> = self.modifiers_held.iter().copied().collect();
                self.mods_used_while_held.extend(held);
                self.modifiers_held.insert(vkey);
                // 새로 눌린 키 자신은 깨끗한 탭 후보로 시작
                self.mods_used_while_held.remove(&vkey);
            } else {
                self.modifiers_held.remove(&vkey);
            }
        } else if is_down {
            // 일반 키 down과 함께 홀드 중인 수정자는 전부 "사용됨" — 이후 keyup이
            // 더블탭의 탭으로 기록되지 않게 한다 (콤보로 소비되기 전에 마킹)
            let held: Vec<VKey> = self.modifiers_held.iter().copied().collect();
            self.mods_used_while_held.extend(held);
        }

        // ── keymap on/off 토글 ──
        if is_down && !is_modifier_key(&vkey) {
            if let Some(toggle) = self.config.toggle_keymap.clone() {
                if combo_trigger_matches(&toggle, vkey, &self.modifiers_held) {
                    let enabled = !self.keymap_enabled;
                    // 코드 모드를 끊는 경우 주입된 트리거를 해제해야 한다
                    let chord_trigger = self.take_engaged_chord_trigger();
                    self.reset_runtime_state();
                    self.keymap_enabled = enabled;
                    self.combo_consumed_key = Some(vkey);
                    tracing::info!(
                        "keymap {}",
                        if self.keymap_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                    if let Some(trigger) = chord_trigger {
                        return KeyDecision::ReleaseChord {
                            trigger,
                            deferred_action: None,
                        };
                    }
                    return KeyDecision::Suppress;
                }
            }
        }

        // ── 2. 콤보/더블탭으로 소비된 키의 keyup 억제 ──
        if is_up {
            if self.combo_consumed_key == Some(vkey) {
                self.combo_consumed_key = None;
                return KeyDecision::Suppress;
            }
            if self.dt_consumed_key == Some(vkey) {
                self.dt_consumed_key = None;
                return KeyDecision::Suppress;
            }
        }

        // ── keymap 비활성 시: Launch 콤보만 동작 ──
        if !self.keymap_enabled {
            if is_down && !is_modifier_key(&vkey) {
                let combo_action = self
                    .config
                    .combos
                    .iter()
                    .find(|(trigger, action)| {
                        matches!(action, BindAction::Launch(_))
                            && combo_trigger_matches(trigger, vkey, &self.modifiers_held)
                    })
                    .map(|(_, action)| action.clone());

                if let Some(action) = combo_action {
                    self.combo_consumed_key = Some(vkey);
                    return KeyDecision::execute(action);
                }
            }
            return KeyDecision::PassThrough;
        }

        // ── 3. 레이어 트리거 키 처리 ──
        if is_down {
            if let Some(idx) = self.config.layers.iter().position(|l| l.trigger == vkey) {
                if self.active_layer.is_none() {
                    tracing::debug!("레이어 활성: {}", self.config.layers[idx].name);
                    self.active_layer = Some(idx);
                    self.trigger_down_tick = tick;
                    self.layer_key_used = false;
                    self.pending_layer_launch = None;
                }
                return KeyDecision::Suppress;
            }
        } else if let Some(active_idx) = self.active_layer {
            let is_active_trigger = self
                .config
                .layers
                .get(active_idx)
                .map(|l| l.trigger == vkey)
                .unwrap_or(false);
            if is_active_trigger {
                // 코드 모드 종료 — 주입된 트리거 해제 (tap 판정 없음)
                if self.chord_engaged {
                    self.chord_engaged = false;
                    self.active_layer = None;
                    self.layer_key_used = false;
                    let deferred = self.pending_layer_launch.take();
                    return KeyDecision::ReleaseChord {
                        trigger: vkey,
                        deferred_action: deferred,
                    };
                }
                let elapsed = tick.wrapping_sub(self.trigger_down_tick);
                let layer = &self.config.layers[active_idx];
                let tap_hold_ms = layer.tap_hold_ms;
                let tap_action = layer.tap_action;
                let was_used = self.layer_key_used;
                let pending_launch = self.pending_layer_launch.take();

                self.active_layer = None;
                self.layer_key_used = false;

                if !was_used && elapsed < tap_hold_ms {
                    if let Some(tap_key) = tap_action {
                        return KeyDecision::execute(BindAction::SendKey(tap_key));
                    }
                } else if let Some(action) = pending_launch {
                    // 지연된 Launch — 트리거 키가 떨어진 지금 실행
                    return KeyDecision::execute(action);
                }
                return KeyDecision::Suppress;
            }
        }

        // ── 4. 활성 레이어 매핑 확인 ──
        if let Some(layer_idx) = self.active_layer {
            let trigger = self.config.layers[layer_idx].trigger;

            // 4-pre. 코드 모드 중에는 홀드가 끝날 때까지 매핑 키 포함 전부
            // OS 조합으로 통과한다 (A안 — OS에는 주입된 트리거가 눌려 있음)
            if self.chord_engaged {
                return KeyDecision::PassThrough;
            }

            // 4a. 레이어 내 더블탭 매핑 우선 확인
            let dt_opt = self.config.layers[layer_idx]
                .double_tap_mappings
                .get(&vkey)
                .cloned();

            if let Some(dt) = dt_opt {
                if is_down {
                    self.layer_key_used = true;

                    // 이전 탭과 동일 키이고 timeout 이내면 → 더블탭 액션
                    if self.layer_dt_last_key == Some(vkey) {
                        let elapsed = tick.wrapping_sub(self.layer_dt_last_tick);
                        if elapsed < dt.timeout_ms {
                            self.layer_dt_last_key = None;
                            return KeyDecision::execute_in_layer(dt.double_action, trigger);
                        }
                    }

                    // 첫 번째 탭 → 싱글 액션 즉시 실행, 더블탭 대기 기록
                    self.layer_dt_last_key = Some(vkey);
                    self.layer_dt_last_tick = tick;
                    return KeyDecision::execute_in_layer(dt.single_action, trigger);
                }
                return KeyDecision::Suppress;
            }

            // 4b. 일반 레이어 매핑
            let action_opt = self.config.layers[layer_idx].mappings.get(&vkey).cloned();

            if let Some(action) = action_opt {
                if is_down {
                    self.layer_key_used = true;
                    self.layer_dt_last_key = None;
                    // Launch는 트리거 키를 뗄 때까지 지연 — 트리거 수정자가
                    // 눌린 채 실행하면 새 프로세스 포커스/IME에 간섭한다
                    if matches!(action, BindAction::Launch(_)) {
                        self.pending_layer_launch = Some(action);
                        return KeyDecision::Suppress;
                    }
                    return KeyDecision::execute_in_layer(action, trigger);
                }
                return KeyDecision::Suppress;
            }

            // 4c. 미매핑 키 — 레이어별 unmapped 정책 적용
            match self.config.layers[layer_idx].unmapped {
                // 현행 동작: 아래 콤보/더블탭/리맵 검사로 폴스루 (맨키 통과)
                UnmappedBehavior::Plain => {}
                // VIA KC_NO: 비수정자 키 억제 (수정자는 추적을 위해 통과)
                UnmappedBehavior::Block => {
                    if !is_modifier_key(&vkey) {
                        if is_down {
                            self.layer_key_used = true;
                        }
                        return KeyDecision::Suppress;
                    }
                }
                // VIA KC_TRNS: 트리거 조합으로 코드 모드 진입.
                // 트리거가 수정자일 때만 성립 — 아니면 Plain 폴백
                UnmappedBehavior::Passthrough => {
                    if is_down && !is_modifier_key(&vkey) && is_modifier_key(&trigger) {
                        self.chord_engaged = true;
                        self.layer_key_used = true;
                        return KeyDecision::EngageChord { trigger, key: vkey };
                    }
                }
            }
        }

        // ── 5. 콤보 리맵 확인 (수정자+키 조합) ──
        if is_down && !is_modifier_key(&vkey) {
            let combo_action = self
                .config
                .combos
                .iter()
                .find(|(trigger, _)| combo_trigger_matches(trigger, vkey, &self.modifiers_held))
                .map(|(_, action)| action.clone());

            if let Some(action) = combo_action {
                self.combo_consumed_key = Some(vkey);
                return KeyDecision::execute(action);
            }
        }

        // ── 6. 더블탭 확인 ──
        if is_down {
            let dt_binding = self
                .config
                .double_taps
                .iter()
                .find(|dt| dt.key == vkey)
                .cloned();

            if let Some(dt) = dt_binding {
                if self.last_tap_key == Some(vkey) {
                    let elapsed = tick.wrapping_sub(self.last_tap_tick);
                    if elapsed < dt.timeout_ms {
                        self.last_tap_key = None;
                        self.dt_consumed_key = Some(vkey);
                        if is_modifier_key(&vkey) {
                            self.modifiers_held.remove(&vkey);
                        }
                        return KeyDecision::execute(dt.action);
                    }
                }
            } else if !is_modifier_key(&vkey) {
                self.last_tap_key = None;
            }
        }

        // ── 7. 단순 리매핑 확인 ──
        if let Some(action) = self.config.remaps.get(&vkey).cloned() {
            if is_down {
                return KeyDecision::execute(action);
            }
            return KeyDecision::Suppress;
        }

        // ── 8. 더블탭 상태 기록 (keyup 시 탭 완료 기록) ──
        if is_up {
            // 홀드 중 다른 키와 조합된 수정자는 탭으로 기록하지 않는다
            let was_used = self.mods_used_while_held.remove(&vkey);
            let has_dt = self.config.double_taps.iter().any(|dt| dt.key == vkey);
            if has_dt && !was_used {
                self.last_tap_key = Some(vkey);
                self.last_tap_tick = tick;
            }
        }

        KeyDecision::PassThrough
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybind::{DoubleTapBinding, Layer, LayerDoubleTap, Modifier};
    use std::collections::HashMap;

    fn empty_config() -> KeybindConfig {
        KeybindConfig::empty()
    }

    /// CapsLock → Escape 리맵 설정
    fn remap_config() -> KeybindConfig {
        let mut cfg = empty_config();
        cfg.remaps
            .insert(VKey::CapsLock, BindAction::SendKey(VKey::Escape));
        cfg
    }

    /// vim-nav 스타일 레이어 (LAlt 홀드, tap=Escape, H→Left, I 더블탭)
    fn layer_config() -> KeybindConfig {
        let mut mappings = HashMap::new();
        mappings.insert(VKey::H, BindAction::SendKey(VKey::Left));

        let mut dt_mappings = HashMap::new();
        dt_mappings.insert(
            VKey::I,
            LayerDoubleTap {
                single_action: BindAction::SendKey(VKey::Home),
                double_action: BindAction::SendKey(VKey::End),
                timeout_ms: 300,
            },
        );

        let mut cfg = empty_config();
        cfg.layers.push(Layer {
            name: "nav".into(),
            trigger: VKey::LAlt,
            tap_action: Some(VKey::Escape),
            tap_hold_ms: 200,
            unmapped: UnmappedBehavior::Plain,
            mappings,
            double_tap_mappings: dt_mappings,
        });
        cfg
    }

    /// layer_config에서 unmapped 정책만 바꾼 변형
    fn layer_config_unmapped(behavior: UnmappedBehavior) -> KeybindConfig {
        let mut cfg = layer_config();
        cfg.layers[0].unmapped = behavior;
        cfg
    }

    fn assert_execute_sendkey(decision: KeyDecision, expected: VKey) {
        match decision {
            KeyDecision::Execute {
                action: BindAction::SendKey(k),
                ..
            } => assert_eq!(k, expected),
            other => panic!("Execute(SendKey({expected:?})) 기대, 실제: {other:?}"),
        }
    }

    // ── 단순 리맵 ──

    #[test]
    fn remap_down_executes_up_suppressed() {
        let mut e = EngineState::new(remap_config());
        assert_execute_sendkey(e.process_key(VKey::CapsLock, true, 0), VKey::Escape);
        assert!(matches!(
            e.process_key(VKey::CapsLock, false, 50),
            KeyDecision::Suppress
        ));
    }

    #[test]
    fn unbound_key_passes_through() {
        let mut e = EngineState::new(remap_config());
        assert!(matches!(
            e.process_key(VKey::A, true, 0),
            KeyDecision::PassThrough
        ));
        assert!(matches!(
            e.process_key(VKey::A, false, 30),
            KeyDecision::PassThrough
        ));
    }

    // ── 레이어 tap vs hold ──

    #[test]
    fn layer_quick_tap_sends_tap_action() {
        let mut e = EngineState::new(layer_config());
        // 트리거 down → 억제, 레이어 활성
        assert!(matches!(
            e.process_key(VKey::LAlt, true, 1000),
            KeyDecision::Suppress
        ));
        // 199ms 후 up, 다른 키 미사용 → tap 액션 (Escape)
        assert_execute_sendkey(e.process_key(VKey::LAlt, false, 1199), VKey::Escape);
    }

    #[test]
    fn layer_long_hold_does_not_send_tap() {
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, 1000);
        // tap_hold_ms(200) 초과 → tap 없음
        assert!(matches!(
            e.process_key(VKey::LAlt, false, 1300),
            KeyDecision::Suppress
        ));
    }

    #[test]
    fn layer_mapping_fires_and_cancels_tap() {
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, 1000);
        // 레이어 활성 중 H down → Left
        assert_execute_sendkey(e.process_key(VKey::H, true, 1050), VKey::Left);
        // H up → 억제
        assert!(matches!(
            e.process_key(VKey::H, false, 1080),
            KeyDecision::Suppress
        ));
        // 빠르게 뗐어도 키를 사용했으므로 tap 액션 없음
        assert!(matches!(
            e.process_key(VKey::LAlt, false, 1100),
            KeyDecision::Suppress
        ));
    }

    #[test]
    fn layer_deactivates_after_trigger_up() {
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, 1000);
        e.process_key(VKey::LAlt, false, 1500);
        // 레이어 해제 후 H는 일반 키
        assert!(matches!(
            e.process_key(VKey::H, true, 2000),
            KeyDecision::PassThrough
        ));
    }

    // ── 레이어 내 더블탭 ──

    #[test]
    fn layer_double_tap_single_then_double() {
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, 1000);

        // 첫 탭 → 싱글 액션 (Home)
        assert_execute_sendkey(e.process_key(VKey::I, true, 1100), VKey::Home);
        assert!(matches!(
            e.process_key(VKey::I, false, 1130),
            KeyDecision::Suppress
        ));

        // timeout(300ms) 이내 두 번째 탭 → 더블 액션 (End)
        assert_execute_sendkey(e.process_key(VKey::I, true, 1250), VKey::End);
    }

    #[test]
    fn layer_double_tap_timeout_resets_to_single() {
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, 1000);

        assert_execute_sendkey(e.process_key(VKey::I, true, 1100), VKey::Home);
        e.process_key(VKey::I, false, 1130);

        // timeout 초과 → 다시 싱글 액션
        assert_execute_sendkey(e.process_key(VKey::I, true, 1500), VKey::Home);
    }

    // ── 레이어 패스쓰루 (코드 모드) — docs/08 P0/P1 ──

    /// Alt 홀드 + 미매핑 Tab → 코드 진입, Tab up 통과, Alt up → 해제 (tap 없음)
    #[test]
    fn passthrough_unmapped_key_engages_and_releases_chord() {
        let mut e = EngineState::new(layer_config_unmapped(UnmappedBehavior::Passthrough));
        e.process_key(VKey::LAlt, true, 1000);

        match e.process_key(VKey::Tab, true, 1050) {
            KeyDecision::EngageChord {
                trigger: VKey::LAlt,
                key: VKey::Tab,
            } => {}
            other => panic!("EngageChord(LAlt, Tab) 기대, 실제: {other:?}"),
        }
        // 물리 Tab up은 통과 (down은 주입됐지만 up 상태는 일관됨)
        assert!(matches!(
            e.process_key(VKey::Tab, false, 1080),
            KeyDecision::PassThrough
        ));
        // 빠르게 뗐어도 코드 모드였으므로 tap(Escape)이 아니라 ReleaseChord
        match e.process_key(VKey::LAlt, false, 1150) {
            KeyDecision::ReleaseChord {
                trigger: VKey::LAlt,
                deferred_action: None,
            } => {}
            other => panic!("ReleaseChord(LAlt) 기대, 실제: {other:?}"),
        }
    }

    /// 코드 모드 중에는 후속 키(반복 Tab, 매핑 키 H 포함)가 전부 OS로 통과 (A안)
    #[test]
    fn chord_mode_passes_all_keys_until_release() {
        let mut e = EngineState::new(layer_config_unmapped(UnmappedBehavior::Passthrough));
        e.process_key(VKey::LAlt, true, 1000);
        e.process_key(VKey::Tab, true, 1050); // EngageChord

        // Alt 홀드 + Tab Tab (스위처 순회) → 통과
        assert!(matches!(
            e.process_key(VKey::Tab, true, 1200),
            KeyDecision::PassThrough
        ));
        // 매핑된 H도 코드 모드 중엔 레이어 액션이 아니라 OS 조합(Alt+H)
        assert!(matches!(
            e.process_key(VKey::H, true, 1300),
            KeyDecision::PassThrough
        ));
        assert!(matches!(
            e.process_key(VKey::H, false, 1330),
            KeyDecision::PassThrough
        ));
        assert!(matches!(
            e.process_key(VKey::LAlt, false, 1400),
            KeyDecision::ReleaseChord { .. }
        ));
    }

    /// 코드 진입 전의 매핑 키는 정상 실행 — 진입은 미매핑 키에서만
    #[test]
    fn passthrough_mapped_key_executes_before_chord() {
        let mut e = EngineState::new(layer_config_unmapped(UnmappedBehavior::Passthrough));
        e.process_key(VKey::LAlt, true, 1000);
        assert_execute_sendkey(e.process_key(VKey::H, true, 1050), VKey::Left);
        e.process_key(VKey::H, false, 1080);

        // 그 다음 미매핑 키 → 코드 진입
        assert!(matches!(
            e.process_key(VKey::Tab, true, 1100),
            KeyDecision::EngageChord { .. }
        ));
    }

    /// 코드 해제 후 레이어를 다시 활성화하면 매핑이 정상 동작 (상태 오염 없음)
    #[test]
    fn chord_state_clean_after_release() {
        let mut e = EngineState::new(layer_config_unmapped(UnmappedBehavior::Passthrough));
        e.process_key(VKey::LAlt, true, 1000);
        e.process_key(VKey::Tab, true, 1050);
        e.process_key(VKey::Tab, false, 1080);
        e.process_key(VKey::LAlt, false, 1100); // ReleaseChord

        // 재활성화 → 매핑 키 정상, 미매핑 키는 새 코드 진입
        e.process_key(VKey::LAlt, true, 2000);
        assert_execute_sendkey(e.process_key(VKey::H, true, 2050), VKey::Left);
        e.process_key(VKey::H, false, 2080);
        assert!(matches!(
            e.process_key(VKey::Tab, true, 2100),
            KeyDecision::EngageChord { .. }
        ));
    }

    /// plain(기본값) 레이어는 기존 동작 유지 — 코드 진입 없음
    #[test]
    fn plain_layer_unmapped_never_engages_chord() {
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, 1000);
        assert!(matches!(
            e.process_key(VKey::Tab, true, 1050),
            KeyDecision::PassThrough
        ));
    }

    /// block 레이어는 미매핑 키를 억제하고, tap 액션도 발동하지 않는다
    #[test]
    fn block_layer_suppresses_unmapped_and_tap() {
        let mut e = EngineState::new(layer_config_unmapped(UnmappedBehavior::Block));
        e.process_key(VKey::LAlt, true, 1000);
        assert!(matches!(
            e.process_key(VKey::Tab, true, 1050),
            KeyDecision::Suppress
        ));
        assert!(matches!(
            e.process_key(VKey::Tab, false, 1080),
            KeyDecision::Suppress
        ));
        // 억제된 키도 "사용됨" — 빠른 해제에서 tap(Escape) 미발동
        assert!(matches!(
            e.process_key(VKey::LAlt, false, 1150),
            KeyDecision::Suppress
        ));
    }

    /// 비수정자 트리거(CapsLock)의 passthrough는 plain으로 폴백
    #[test]
    fn passthrough_with_non_modifier_trigger_falls_back_to_plain() {
        let mut cfg = layer_config_unmapped(UnmappedBehavior::Passthrough);
        cfg.layers[0].trigger = VKey::CapsLock;
        let mut e = EngineState::new(cfg);

        e.process_key(VKey::CapsLock, true, 1000);
        assert!(matches!(
            e.process_key(VKey::Tab, true, 1050),
            KeyDecision::PassThrough
        ));
    }

    /// keymap 토글이 코드 모드를 끊으면 ReleaseChord로 주입 트리거를 해제
    #[test]
    fn toggle_during_chord_releases_injected_trigger() {
        let mut cfg = layer_config_unmapped(UnmappedBehavior::Passthrough);
        cfg.toggle_keymap = Some(ComboTrigger {
            modifiers: vec![],
            key: VKey::F12,
        });
        let mut e = EngineState::new(cfg);

        e.process_key(VKey::LAlt, true, 1000);
        e.process_key(VKey::Tab, true, 1050); // EngageChord
        e.process_key(VKey::Tab, false, 1080);

        match e.process_key(VKey::F12, true, 1200) {
            KeyDecision::ReleaseChord {
                trigger: VKey::LAlt,
                deferred_action: None,
            } => {}
            other => panic!("ReleaseChord(LAlt) 기대, 실제: {other:?}"),
        }
        // 토글 후 keymap 비활성 — 일반 키는 통과
        assert!(matches!(
            e.process_key(VKey::H, true, 1300),
            KeyDecision::PassThrough
        ));
    }

    /// 코드 진입 전에 지연된 레이어 Launch는 ReleaseChord로 전달된다
    #[test]
    fn pending_launch_delivered_via_release_chord() {
        let mut cfg = layer_config_unmapped(UnmappedBehavior::Passthrough);
        cfg.layers[0]
            .mappings
            .insert(VKey::Space, BindAction::Launch("kmd-desktop".into()));
        let mut e = EngineState::new(cfg);

        e.process_key(VKey::LAlt, true, 1000);
        e.process_key(VKey::Space, true, 1050); // Launch 지연
        e.process_key(VKey::Space, false, 1080);
        e.process_key(VKey::Tab, true, 1100); // EngageChord

        match e.process_key(VKey::LAlt, false, 1200) {
            KeyDecision::ReleaseChord {
                trigger: VKey::LAlt,
                deferred_action: Some(BindAction::Launch(cmd)),
            } => assert_eq!(cmd, "kmd-desktop"),
            other => panic!("ReleaseChord + Launch 기대, 실제: {other:?}"),
        }
    }

    // ── 레이어 Launch 지연 실행 ──

    #[test]
    fn layer_launch_deferred_until_trigger_release() {
        let mut cfg = layer_config();
        cfg.layers[0]
            .mappings
            .insert(VKey::Space, BindAction::Launch("kmd-desktop".into()));

        let mut e = EngineState::new(cfg);
        e.process_key(VKey::LAlt, true, 1000);

        // Launch 매핑 키 down → 즉시 실행하지 않고 억제 (지연)
        assert!(matches!(
            e.process_key(VKey::Space, true, 1100),
            KeyDecision::Suppress
        ));
        assert!(matches!(
            e.process_key(VKey::Space, false, 1150),
            KeyDecision::Suppress
        ));

        // 트리거 키를 떼는 순간 Launch 실행 (tap 아님 — 키 사용됨)
        assert!(matches!(
            e.process_key(VKey::LAlt, false, 1300),
            KeyDecision::Execute {
                action: BindAction::Launch(_),
                layer_trigger: None,
            }
        ));
    }

    #[test]
    fn layer_non_launch_action_carries_trigger_context() {
        // 레이어 매핑 실행 결정에는 트리거 키가 포함되어야 한다
        // (macOS가 잔여 modifier 플래그를 해제하는 데 사용)
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, 1000);
        match e.process_key(VKey::H, true, 1050) {
            KeyDecision::Execute {
                action: BindAction::SendKey(VKey::Left),
                layer_trigger: Some(VKey::LAlt),
            } => {}
            other => panic!("layer_trigger=Some(LAlt) 기대, 실제: {other:?}"),
        }
    }

    #[test]
    fn pending_launch_cleared_on_toggle() {
        let mut cfg = layer_config();
        cfg.layers[0]
            .mappings
            .insert(VKey::Space, BindAction::Launch("kmd-desktop".into()));
        cfg.toggle_keymap = Some(ComboTrigger {
            modifiers: vec![Modifier::Ctrl],
            key: VKey::K,
        });

        let mut e = EngineState::new(cfg);
        e.process_key(VKey::LAlt, true, 1000);
        e.process_key(VKey::Space, true, 1100); // pending launch 저장

        // 토글로 상태 리셋 (LAlt 홀드 중이지만 Ctrl+K)
        e.process_key(VKey::LCtrl, true, 1200);
        e.process_key(VKey::K, true, 1220);
        e.process_key(VKey::K, false, 1240);
        e.process_key(VKey::LCtrl, false, 1260);

        // 트리거 keyup — 리셋됐으므로 pending launch가 실행되면 안 됨
        // (keymap off 상태라 레이어 로직 자체가 비활성 → PassThrough)
        assert!(matches!(
            e.process_key(VKey::LAlt, false, 1300),
            KeyDecision::PassThrough
        ));
    }

    // ── macOS 어댑터 헬퍼 ──

    #[test]
    fn sync_modifier_flags_removes_stale_modifiers() {
        let mut e = EngineState::new(empty_config());
        e.process_key(VKey::LShift, true, 0);
        e.process_key(VKey::LCtrl, true, 10);
        assert!(e.is_modifier_held(VKey::LShift));

        // OS 플래그: shift 꺼짐 (keyup을 놓친 상황) → 동기화로 제거
        e.sync_modifier_flags(false, true, false, false);
        assert!(!e.is_modifier_held(VKey::LShift));
        assert!(e.is_modifier_held(VKey::LCtrl), "플래그 켜진 수정자는 유지");
    }

    #[test]
    fn reset_transient_state_clears_modifiers_and_layer() {
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LShift, true, 0);
        e.process_key(VKey::LAlt, true, 10); // 레이어 활성

        e.reset_transient_state();

        assert!(e.held_modifiers().is_empty());
        // 레이어가 리셋됐으므로 H는 일반 키
        assert!(matches!(
            e.process_key(VKey::H, true, 100),
            KeyDecision::PassThrough
        ));
    }

    // ── 콤보 ──

    fn combo_config() -> KeybindConfig {
        let mut cfg = empty_config();
        cfg.combos.push((
            ComboTrigger {
                modifiers: vec![Modifier::Shift],
                key: VKey::Space,
            },
            BindAction::SendKey(VKey::Hangul),
        ));
        cfg
    }

    #[test]
    fn combo_fires_with_modifier_held_and_suppresses_keyup() {
        let mut e = EngineState::new(combo_config());
        // Shift down → 수정자는 통과
        assert!(matches!(
            e.process_key(VKey::LShift, true, 0),
            KeyDecision::PassThrough
        ));
        // Shift+Space → Hangul
        assert_execute_sendkey(e.process_key(VKey::Space, true, 50), VKey::Hangul);
        // 콤보로 소비된 Space의 keyup도 억제
        assert!(matches!(
            e.process_key(VKey::Space, false, 100),
            KeyDecision::Suppress
        ));
        // Shift up은 통과
        assert!(matches!(
            e.process_key(VKey::LShift, false, 150),
            KeyDecision::PassThrough
        ));
    }

    #[test]
    fn combo_does_not_fire_without_modifier() {
        let mut e = EngineState::new(combo_config());
        assert!(matches!(
            e.process_key(VKey::Space, true, 0),
            KeyDecision::PassThrough
        ));
    }

    // ── 글로벌 더블탭 ──

    fn double_tap_config() -> KeybindConfig {
        let mut cfg = empty_config();
        cfg.double_taps.push(DoubleTapBinding {
            key: VKey::RShift,
            action: BindAction::SendKey(VKey::Hangul),
            timeout_ms: 300,
        });
        cfg
    }

    #[test]
    fn double_tap_fires_within_timeout() {
        let mut e = EngineState::new(double_tap_config());
        // 첫 탭: down 통과, up에서 탭 기록
        assert!(matches!(
            e.process_key(VKey::RShift, true, 1000),
            KeyDecision::PassThrough
        ));
        assert!(matches!(
            e.process_key(VKey::RShift, false, 1050),
            KeyDecision::PassThrough
        ));
        // timeout 이내 두 번째 down → 액션
        assert_execute_sendkey(e.process_key(VKey::RShift, true, 1200), VKey::Hangul);
        // 소비된 keyup 억제
        assert!(matches!(
            e.process_key(VKey::RShift, false, 1250),
            KeyDecision::Suppress
        ));
    }

    #[test]
    fn double_tap_expires_after_timeout() {
        let mut e = EngineState::new(double_tap_config());
        e.process_key(VKey::RShift, true, 1000);
        e.process_key(VKey::RShift, false, 1050);
        // timeout(300ms) 초과 → 발동 안 함
        assert!(matches!(
            e.process_key(VKey::RShift, true, 1400),
            KeyDecision::PassThrough
        ));
    }

    #[test]
    fn modifier_used_in_combo_does_not_register_tap() {
        // RShift+A(대문자 입력) 직후 RShift 재탭이 더블탭으로 오판정되면 안 됨
        let mut e = EngineState::new(double_tap_config());
        e.process_key(VKey::RShift, true, 1000);
        // RShift 홀드 중 A 입력 → RShift는 "수정자로 사용됨"
        e.process_key(VKey::A, true, 1020);
        e.process_key(VKey::A, false, 1050);
        e.process_key(VKey::RShift, false, 1080); // 사용됐으므로 탭 기록 안 됨

        // 곧바로 RShift 다시 → 더블탭 아님 (첫 탭부터 다시 시작)
        assert!(matches!(
            e.process_key(VKey::RShift, true, 1150),
            KeyDecision::PassThrough
        ));
    }

    #[test]
    fn other_key_between_taps_resets_double_tap() {
        let mut e = EngineState::new(double_tap_config());
        e.process_key(VKey::RShift, true, 1000);
        e.process_key(VKey::RShift, false, 1050); // 탭 기록
                                                  // 다른 (비수정자) 키 입력 → 탭 시퀀스 리셋
        e.process_key(VKey::A, true, 1100);
        e.process_key(VKey::A, false, 1130);
        assert!(matches!(
            e.process_key(VKey::RShift, true, 1200),
            KeyDecision::PassThrough
        ));
    }

    // ── keymap 토글 ──

    fn toggle_config() -> KeybindConfig {
        let mut cfg = remap_config();
        cfg.toggle_keymap = Some(ComboTrigger {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: VKey::K,
        });
        cfg.combos.push((
            ComboTrigger {
                modifiers: vec![Modifier::Alt],
                key: VKey::Space,
            },
            BindAction::Launch("kmd-desktop".into()),
        ));
        cfg
    }

    #[test]
    fn toggle_disables_remaps_but_keeps_launch_combo() {
        let mut e = EngineState::new(toggle_config());

        // 토글 전: 리맵 동작
        assert_execute_sendkey(e.process_key(VKey::CapsLock, true, 0), VKey::Escape);
        e.process_key(VKey::CapsLock, false, 30);

        // Ctrl+Alt+K → keymap off
        e.process_key(VKey::LCtrl, true, 100);
        e.process_key(VKey::LAlt, true, 120);
        assert!(matches!(
            e.process_key(VKey::K, true, 140),
            KeyDecision::Suppress
        ));
        // 소비된 K keyup 억제
        assert!(matches!(
            e.process_key(VKey::K, false, 160),
            KeyDecision::Suppress
        ));
        e.process_key(VKey::LCtrl, false, 180);
        e.process_key(VKey::LAlt, false, 200);

        // off 상태: 리맵은 통과
        assert!(matches!(
            e.process_key(VKey::CapsLock, true, 300),
            KeyDecision::PassThrough
        ));

        // off 상태에서도 Launch 콤보는 동작
        e.process_key(VKey::LAlt, true, 400);
        assert!(matches!(
            e.process_key(VKey::Space, true, 420),
            KeyDecision::Execute {
                action: BindAction::Launch(_),
                ..
            }
        ));
        e.process_key(VKey::Space, false, 440);
        e.process_key(VKey::LAlt, false, 460);

        // 다시 토글 → on
        e.process_key(VKey::LCtrl, true, 500);
        e.process_key(VKey::LAlt, true, 520);
        e.process_key(VKey::K, true, 540);
        e.process_key(VKey::K, false, 560);
        e.process_key(VKey::LCtrl, false, 580);
        e.process_key(VKey::LAlt, false, 600);

        assert_execute_sendkey(e.process_key(VKey::CapsLock, true, 700), VKey::Escape);
    }

    // ── tick wrapping ──

    #[test]
    fn tick_wrapping_does_not_break_timing() {
        // u32 tick이 랩어라운드해도 wrapping_sub로 정상 판정
        let mut e = EngineState::new(layer_config());
        e.process_key(VKey::LAlt, true, u32::MAX - 50);
        // 랩어라운드 후 149ms 경과 (< 200ms) → tap
        assert_execute_sendkey(e.process_key(VKey::LAlt, false, 99), VKey::Escape);
    }
}
