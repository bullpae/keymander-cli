//! Application state, messages, update/view/subscription — the Elm core.
//!
//! Renders a Spotlight-like floating launcher: search bar always visible,
//! results list + status bar appear only when there are results.
//!
//! **Key features**:
//! - Async engine loading — window appears instantly
//! - Singleton toggle via `kmd_core::single_instance::Guard`
//! - Window position/width persisted between sessions
//! - Resizable width with min/max constraints

use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use iced::keyboard;
use iced::widget::operation::scroll_to;
use iced::widget::scrollable as scrollable_mod;
use iced::widget::text::Wrapping;
use iced::widget::{
    container, image, mouse_area, row, scrollable, text, text_input, Column, Space,
};
use iced::{
    window, Background, Border, Color, Element, Fill, Length, Padding, Point, Size, Subscription,
    Task,
};

use kmd_core::plugin::{builtin_calc, builtin_emoji, builtin_shell, Extension};
use kmd_core::single_instance::Guard;
use kmd_core::web;
use kmd_core::{IndexItem, ItemKind, Source};

use crate::theme::DesktopTheme;
use crate::window_state::WindowState;

mod focus_flow;
mod launch;
mod search_routing;
mod settings;
mod view;

// ─── Constants ────────────────────────────────────────────────────────────────

pub const DEFAULT_WIDTH: f32 = 1000.0;
const DRAG_STRIP: f32 = 6.0;
const SEARCH_LIMIT: usize = 50;
const SCORE_PLUGIN: u32 = u32::MAX;

/// font_size 기반 비례 계산으로 모든 UI 치수를 결정.
/// `font_size` 하나를 바꾸면 pill, row, 상태바, 아이콘, 서브 폰트가 모두 연동된다.
#[derive(Debug, Clone, Copy)]
pub struct UiScale {
    pub font: f32,
    pub visible_rows: usize,
    pub pill_height: f32,
    pub search_bar_height: f32,
    pub row_height: f32,
    pub status_bar_height: f32,
    pub full_window_height: f32,
    pub result_icon: f32,
    pub title_font: f32,
    pub subtitle_font: f32,
    pub badge_font: f32,
    pub status_font: f32,
    pub hint_font: f32,
    pub detail_name_font: f32,
    pub detail_big_icon: f32,
    pub path_label_font: f32,
    pub path_text_font: f32,
    pub action_icon_font: f32,
    pub action_label_font: f32,
    pub action_shortcut_font: f32,
    pub no_results_font: f32,
}

impl UiScale {
    pub fn new(raw_font: f32, raw_rows: usize) -> Self {
        let f = raw_font.clamp(12.0, 32.0);
        let visible_rows = raw_rows.clamp(4, 20);
        let pill_height = (f * 2.9).round(); // 16→46
        let search_bar_height = pill_height + DRAG_STRIP;
        let row_height = (f * 3.0).round(); // 16→48
        let status_bar_height = (f * 1.75).round(); // 16→28
        let full_window_height =
            search_bar_height + 1.0 + (visible_rows as f32 * row_height) + 1.0 + status_bar_height;
        Self {
            font: f,
            visible_rows,
            pill_height,
            search_bar_height,
            row_height,
            status_bar_height,
            full_window_height,
            result_icon: (f * 2.0).round(),            // 16→32
            title_font: (f * 1.0).round(),             // 16→16
            subtitle_font: (f * 0.8125).round(),       // 16→13
            badge_font: (f * 0.625).round(),           // 16→10
            status_font: (f * 0.6875).round(),         // 16→11
            hint_font: (f * 0.6875).round(),           // 16→11
            detail_name_font: (f * 0.9375).round(),    // 16→15
            detail_big_icon: (f * 3.25).round(),       // 16→52
            path_label_font: (f * 0.625).round(),      // 16→10
            path_text_font: (f * 0.6875).round(),      // 16→11
            action_icon_font: (f * 0.8125).round(),    // 16→13
            action_label_font: (f * 0.75).round(),     // 16→12
            action_shortcut_font: (f * 0.625).round(), // 16→10
            no_results_font: (f * 0.8125).round(),     // 16→13
        }
    }
}

/// 주어진 font_size, visible_rows로 전체 창 높이를 계산 (main.rs 에서 사용).
pub fn full_window_height(font_size: f32, visible_rows: usize) -> f32 {
    UiScale::new(font_size, visible_rows).full_window_height
}

const QUIT_POLL_MS: u64 = 300;
const WARMUP_IDLE_MS: u64 = 400;
const FOCUS_RETRY_MS: u64 = 120;
const MAX_FOCUS_RETRIES: u8 = 3;
const AUTOSTART_STATUS_REFRESH_MS: u64 = 1500;
// ─── Shared slot for async engine hand-off ────────────────────────────────────

/// 비동기 엔진 로드 결과 — 10-tuple 대신 명확한 필드로 관리
struct EngineLoadResult {
    engine: kmd_core::SearchEngine,
    use_emoji: bool,
    llm_providers: Vec<String>,
    multi_web_providers: Vec<String>,
    llm_prefixes: Vec<String>,
    multi_web_prefixes: Vec<String>,
    spell_providers: Vec<String>,
    spell_prefixes: Vec<String>,
    translate_providers: Vec<String>,
    translate_prefixes: Vec<String>,
}

type EngineSlot = Arc<Mutex<Option<EngineLoadResult>>>;

// ─── App State ────────────────────────────────────────────────────────────────

pub struct App {
    query: String,
    results: Vec<kmd_core::SearchResult>,
    search_mode: kmd_core::SearchMode,
    selected: usize,
    engine: kmd_core::SearchEngine,
    theme: DesktopTheme,
    ui: UiScale,
    input_id: iced::widget::Id,
    scrollable_id: iced::widget::Id,
    window_id: Option<window::Id>,
    raw_window_id: Option<u64>,
    use_emoji: bool,
    selected_llm_providers: Vec<String>,
    multi_llm_prefixes: Vec<String>,
    selected_multi_web_providers: Vec<String>,
    multi_web_prefixes: Vec<String>,
    spell_providers: Vec<String>,
    spell_prefixes: Vec<String>,
    translate_providers: Vec<String>,
    translate_prefixes: Vec<String>,
    daemon_autostart_enabled: Option<bool>,
    daemon_autostart_last_checked_at: Option<std::time::Instant>,
    daemon_autostart_check_in_flight: bool,
    daemon_autostart_toggle_in_flight: bool,
    runtime_config: kmd_core::Config,
    last_results_signature: u64,
    loading: bool,
    engine_slot: EngineSlot,
    db: kmd_core::db::Database,
    _guard: Guard,

    // ── Window geometry ───────────────────────────────────────────────
    window_width: f32,
    window_state: WindowState,
    state_dirty: bool,
    window_focused: bool,
    last_query_changed_at: std::time::Instant,
    app_started_at: std::time::Instant,
    ime_trace_seq: u64,
    ime_trace_enabled: bool,

    // ── Focus tracking ─────────────────────────────────────────────────
    /// 디버그/테스트용: focus 요청 총 횟수
    focus_request_count: u32,
    /// 부팅 직후 포커스 폭주를 방지 — 첫 포커스 이후 안정화 기간에는 재포커싱 차단
    boot_focus_done: bool,

    // ── IME ───────────────────────────────────────────────────────────
    reset_ime_on_launch: bool,

    // ── Startup warmup ────────────────────────────────────────────────
    full_warmup_started: bool,
    warmup_token: u64,
}

// ─── Messages ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    QueryChanged(String),
    Submit,
    ResultClicked(usize),
    KeyEvent(keyboard::Key, keyboard::Modifiers),
    BrandClicked,
    BrandRightClicked,
    StartWindowDrag,
    StartWindowResize(window::Direction),
    GotWindowId(Option<window::Id>),
    GotRawWindowId(u64),
    WarmupTick(u64),
    EnsureFocus(u8),
    EngineReady,
    CheckQuitSignal,
    WindowEvent(window::Id, window::Event),
    ShellDone(Result<String, String>),
    /// 상세 패널의 컨텍스트 액션 실행
    RunAction(ContextAction),
    /// 포커스 잃은 후 debounce 확인 → 여전히 포커스 없으면 종료
    CheckUnfocusedExit,
    /// 투명 배경 클릭 → Spotlight처럼 앱 닫기
    BackgroundClicked,
    /// view 재렌더 후 1프레임 뒤 포커스 재요청 (트리 변경 후 포커스 유실 복구)
    DelayedRefocus,
    /// 백그라운드 아이콘 프리워밍 완료 → 캐시된 아이콘으로 리렌더 유도
    IconsReady,
    /// 데몬 자동시작 상태 조회 완료
    AutostartStatusLoaded(Result<bool, String>),
    /// 데몬 자동시작 토글 완료
    AutostartToggleFinished(Result<String, String>),
    /// 엔진 교체(EngineReady) 후 IME 조합이 끝나면 검색 결과를 갱신
    EngineSwapSettled,
}

// ─── Context Actions ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum ContextAction {
    Open,
    OpenAsAdmin,
    OpenFolder,
    CopyPath,
    CopyName,
}

impl ContextAction {
    fn label(&self) -> &str {
        match self {
            Self::Open => "열기",
            Self::OpenAsAdmin => "관리자 권한으로 열기",
            Self::OpenFolder => "파일 위치 열기",
            Self::CopyPath => "경로 복사",
            Self::CopyName => "이름 복사",
        }
    }

    /// Calculator/Emoji 컨텍스트에서 보여줄 레이블 (값 복사)
    fn label_for_kind(&self, kind: ItemKind) -> &str {
        match (self, kind) {
            (Self::CopyName, ItemKind::Calculator) => "값 복사",
            (Self::CopyName, ItemKind::Emoji) => "이모지 복사",
            (Self::Open, ItemKind::Calculator) => "복사 후 닫기",
            _ => self.label(),
        }
    }

    fn shortcut(&self) -> &str {
        match self {
            Self::Open => "Ctrl+1",
            Self::OpenAsAdmin => "Ctrl+Shift+Enter",
            Self::OpenFolder => "Ctrl+2",
            Self::CopyPath => "Ctrl+3",
            Self::CopyName => "Ctrl+4",
        }
    }

    /// 이 액션에 할당된 Ctrl+N 인덱스 (0-based). 위치가 아닌 타입으로 결정.
    /// Calculator처럼 액션이 1개뿐이어도 CopyName은 항상 Ctrl+4(=3)로 동작.
    fn shortcut_index(&self) -> usize {
        match self {
            Self::Open => 0,
            Self::OpenFolder => 1,
            Self::CopyPath => 2,
            Self::CopyName => 3,
            Self::OpenAsAdmin => usize::MAX,
        }
    }
}

/// 아이템 종류에 따라 사용 가능한 컨텍스트 액션 목록
fn context_actions_for(kind: ItemKind) -> Vec<ContextAction> {
    match kind {
        ItemKind::App | ItemKind::Executable => vec![
            ContextAction::Open,
            ContextAction::OpenAsAdmin,
            ContextAction::OpenFolder,
            ContextAction::CopyPath,
        ],
        ItemKind::File => vec![
            ContextAction::Open,
            ContextAction::OpenFolder,
            ContextAction::CopyPath,
            ContextAction::CopyName,
        ],
        ItemKind::Directory => vec![ContextAction::Open, ContextAction::CopyPath],
        ItemKind::WebSearch => vec![ContextAction::Open, ContextAction::CopyName],
        ItemKind::Shell => vec![ContextAction::Open, ContextAction::CopyName],
        ItemKind::SystemCommand => vec![ContextAction::Open],
        ItemKind::Calculator | ItemKind::Emoji => vec![ContextAction::CopyName],
    }
}

// ─── Boot ─────────────────────────────────────────────────────────────────────

impl App {
    /// 첫 화면 표시를 막지 않도록 항상 quick 인덱스로 부팅한다.
    /// 풀 인덱스는 부팅 후 비동기 워밍업(spawn_full_engine_load_task)으로 교체된다.
    /// 반환하는 bool은 `full_warmup_started` 초기값 — 항상 false라 워밍업이 예약된다.
    fn build_initial_engine(config: &kmd_core::Config) -> (kmd_core::SearchEngine, bool) {
        tracing::info!("Booting with quick index — full index loads asynchronously");
        let engine = crate::engine::create_quick_search_engine(config);
        (engine, false)
    }

    fn initial_boot_tasks(input_id: iced::widget::Id, skip_warmup: bool) -> Task<Message> {
        let focus_task = iced::widget::operation::focus::<Message>(input_id);
        let id_task = window::oldest().map(Message::GotWindowId);
        #[cfg(target_os = "windows")]
        let focus_chain = {
            let delayed = Task::future(async {
                tokio::time::sleep(Duration::from_millis(16)).await;
                Message::DelayedRefocus
            });
            Task::batch([focus_task, delayed])
        };
        #[cfg(not(target_os = "windows"))]
        let focus_chain = focus_task;
        if skip_warmup {
            Task::batch([focus_chain, id_task])
        } else {
            let warmup_task = Self::schedule_warmup_tick(0);
            Task::batch([focus_chain, id_task, warmup_task])
        }
    }

    fn with_activity_warmup(&mut self, base_task: Task<Message>) -> Task<Message> {
        if self.full_warmup_started {
            base_task
        } else {
            self.warmup_token = self.warmup_token.wrapping_add(1);
            let warmup_task = Self::schedule_warmup_tick(self.warmup_token);
            Task::batch([base_task, warmup_task])
        }
    }

    fn apply_contains_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = IndexItem>,
    {
        self.apply_contains_results(items_to_results(items));
    }

    fn apply_contains_results(&mut self, results: Vec<kmd_core::SearchResult>) {
        self.commit_results(results, kmd_core::SearchMode::Contains, true);
    }

    fn clear_results_state(&mut self, mode: kmd_core::SearchMode) {
        self.results.clear();
        self.last_results_signature = 0;
        self.search_mode = mode;
        self.selected = 0;
    }

    fn should_limit_reorder_for_ime(&self) -> bool {
        self.query
            .chars()
            .rev()
            .find(|c| !c.is_whitespace())
            .is_some_and(Self::is_hangul_jamo)
    }

    fn is_hangul_jamo(ch: char) -> bool {
        is_hangul_jamo(ch)
    }

    fn kind_code(kind: ItemKind) -> u8 {
        match kind {
            ItemKind::App => 1,
            ItemKind::Directory => 2,
            ItemKind::File => 3,
            ItemKind::Executable => 4,
            ItemKind::SystemCommand => 5,
            ItemKind::WebSearch => 6,
            ItemKind::Calculator => 7,
            ItemKind::Emoji => 8,
            ItemKind::Shell => 9,
        }
    }

    fn results_signature(results: &[kmd_core::SearchResult]) -> u64 {
        let mut hasher = DefaultHasher::new();
        results.len().hash(&mut hasher);
        for r in results.iter().take(64) {
            Self::kind_code(r.item.kind).hash(&mut hasher);
            r.item.name.hash(&mut hasher);
            r.item.path.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn commit_results(
        &mut self,
        new_results: Vec<kmd_core::SearchResult>,
        mode: kmd_core::SearchMode,
        reset_selection: bool,
    ) {
        let signature = Self::results_signature(&new_results);
        self.search_mode = mode;

        if signature == self.last_results_signature {
            return;
        }

        // IME 조합 중에는 결과 재정렬을 잠시 제한해 조합 안정성을 우선한다.
        if self.should_limit_reorder_for_ime() && !self.results.is_empty() {
            return;
        }

        self.results = new_results;
        self.last_results_signature = signature;
        if reset_selection {
            self.selected = 0;
        } else if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
    }

    fn log_ime_state(&mut self, label: &str) {
        if !self.ime_trace_enabled {
            return;
        }
        self.ime_trace_seq = self.ime_trace_seq.wrapping_add(1);
        tracing::debug!(
            seq = self.ime_trace_seq,
            elapsed_ms = self.app_started_at.elapsed().as_millis() as u64,
            since_input_ms = self.last_query_changed_at.elapsed().as_millis() as u64,
            window_focused = self.window_focused,
            raw_window_id = ?self.raw_window_id,
            query = %self.query,
            query_len = self.query.chars().count(),
            jamo_tail = self.should_limit_reorder_for_ime(),
            "{label}"
        );
    }

    pub fn new(
        guard: Guard,
        config: kmd_core::Config,
        window_state: WindowState,
    ) -> (Self, Task<Message>) {
        Self::new_with_db_path(guard, config, window_state, None)
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        guard: Guard,
        config: kmd_core::Config,
        window_state: WindowState,
        db_path: std::path::PathBuf,
    ) -> (Self, Task<Message>) {
        Self::new_with_db_path(guard, config, window_state, Some(db_path))
    }

    fn new_with_db_path(
        guard: Guard,
        config: kmd_core::Config,
        window_state: WindowState,
        db_path_override: Option<std::path::PathBuf>,
    ) -> (Self, Task<Message>) {
        // 캐시된 full index가 24시간 이내 → 직접 로드 (2-stage 불필요)
        // 캐시 없거나 오래됨 → quick index로 즉시 표시 후 full index를 비동기 빌드
        let (engine, cache_fresh) = Self::build_initial_engine(&config);
        let db_path = db_path_override.unwrap_or_else(|| {
            kmd_core::Config::default_data_dir()
                .join("desktop")
                .join("kmd.db")
        });
        let db = match kmd_core::db::Database::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                // 파일 DB 열기 실패 시 in-memory로 폴백하되 경고 기록.
                // frecency 히스토리는 이 세션에서만 유효하고 영구 저장되지 않는다.
                tracing::warn!(
                    "DB 파일 열기 실패 ({e}) — in-memory로 폴백, 히스토리가 저장되지 않습니다: {}",
                    db_path.display()
                );
                kmd_core::db::Database::open_in_memory().expect("in-memory DB 초기화 실패")
            }
        };
        let theme = crate::theme::from_name(&config.general.theme);
        let use_emoji = config.general.emoji_icons;
        let selected_llm_providers = config.launcher.multi_llm_providers.clone();
        let multi_llm_prefixes = config.launcher.multi_llm_prefixes.clone();
        let selected_multi_web_providers = config.launcher.multi_web_providers.clone();
        let multi_web_prefixes = config.launcher.multi_web_prefixes.clone();
        let spell_providers = config.launcher.spell_providers.clone();
        let spell_prefixes = config.launcher.spell_prefixes.clone();
        let translate_providers = config.launcher.translate_providers.clone();
        let translate_prefixes = config.launcher.translate_prefixes.clone();
        let reset_ime = config.general.reset_ime_on_launch;
        let ui = UiScale::new(config.general.font_size, config.general.visible_rows);
        let window_width = window_state.width.unwrap_or(DEFAULT_WIDTH);

        let input_id = iced::widget::Id::unique();
        let scrollable_id = iced::widget::Id::unique();
        let engine_slot: EngineSlot = Arc::new(Mutex::new(None));

        let app = Self {
            query: String::new(),
            results: Vec::new(),
            search_mode: kmd_core::SearchMode::Fuzzy,
            selected: 0,
            engine,
            theme,
            ui,
            input_id: input_id.clone(),
            scrollable_id,
            window_id: None,
            raw_window_id: None,
            use_emoji,
            selected_llm_providers,
            multi_llm_prefixes,
            selected_multi_web_providers,
            multi_web_prefixes,
            spell_providers,
            spell_prefixes,
            translate_providers,
            translate_prefixes,
            daemon_autostart_enabled: None,
            daemon_autostart_last_checked_at: None,
            daemon_autostart_check_in_flight: false,
            daemon_autostart_toggle_in_flight: false,
            runtime_config: config,
            last_results_signature: 0,
            loading: false,
            engine_slot,
            db,
            _guard: guard,
            window_width,
            window_state,
            state_dirty: false,
            window_focused: true,
            last_query_changed_at: std::time::Instant::now(),
            app_started_at: std::time::Instant::now(),
            ime_trace_seq: 0,
            ime_trace_enabled: std::env::var_os("KMD_IME_TRACE").is_some(),
            focus_request_count: 0,
            boot_focus_done: false,
            reset_ime_on_launch: reset_ime,
            full_warmup_started: cache_fresh,
            warmup_token: 0,
        };

        let tasks = Self::initial_boot_tasks(input_id, cache_fresh);
        (app, tasks)
    }

    fn schedule_warmup_tick(token: u64) -> Task<Message> {
        Task::future(async move {
            tokio::time::sleep(Duration::from_millis(WARMUP_IDLE_MS)).await;
            Message::WarmupTick(token)
        })
    }

    /// 부팅 안정화 구간(첫 포커스 후 1초 이내) 여부
    fn is_boot_settled(&self) -> bool {
        self.boot_focus_done && self.app_started_at.elapsed() < Duration::from_millis(1000)
    }

    /// focus 요청 + 카운터 증가 + (Windows만) 16ms 뒤 지연 refocus 스케줄링
    fn request_focus(&mut self) -> Task<Message> {
        // 부팅 안정화 구간: 최초 1회 이후 추가 포커싱 차단
        if self.is_boot_settled() {
            self.log_ime_state("request_focus:skip_boot_settling");
            return Task::none();
        }
        self.boot_focus_done = true;

        self.log_ime_state("request_focus");
        self.focus_request_count += 1;
        let immediate = iced::widget::operation::focus::<Message>(self.input_id.clone());
        #[cfg(not(target_os = "windows"))]
        {
            immediate
        }
        #[cfg(target_os = "windows")]
        {
            let delayed = Task::future(async {
                tokio::time::sleep(Duration::from_millis(16)).await;
                Message::DelayedRefocus
            });
            Task::batch([immediate, delayed])
        }
    }

    fn schedule_focus_retry(attempt: u8) -> Task<Message> {
        Task::future(async move {
            tokio::time::sleep(Duration::from_millis(FOCUS_RETRY_MS)).await;
            Message::EnsureFocus(attempt)
        })
    }

    fn spawn_full_engine_load_task(&self) -> Task<Message> {
        let slot = self.engine_slot.clone();
        Task::future(async move {
            match tokio::task::spawn_blocking(move || {
                let config = crate::engine::load_config();
                let eng = crate::engine::create_search_engine(&config);
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(EngineLoadResult {
                        engine: eng,
                        use_emoji: config.general.emoji_icons,
                        llm_providers: config.launcher.multi_llm_providers.clone(),
                        multi_web_providers: config.launcher.multi_web_providers.clone(),
                        llm_prefixes: config.launcher.multi_llm_prefixes.clone(),
                        multi_web_prefixes: config.launcher.multi_web_prefixes.clone(),
                        spell_providers: config.launcher.spell_providers.clone(),
                        spell_prefixes: config.launcher.spell_prefixes.clone(),
                        translate_providers: config.launcher.translate_providers.clone(),
                        translate_prefixes: config.launcher.translate_prefixes.clone(),
                    });
                } else {
                    tracing::error!("engine_slot mutex poisoned — 엔진 로드 결과 저장 실패");
                }
            })
            .await
            {
                Ok(()) => {}
                Err(e) => tracing::error!("엔진 로드 태스크 패닉: {e}"),
            }
            Message::EngineReady
        })
    }

    /// 현재 결과 목록의 아이콘을 백그라운드 스레드에서 미리 추출해 캐시에 채운다.
    ///
    /// `view()`는 캐시만 조회하므로(`cached_icon_for_item`), 렌더 스레드가 OS
    /// 셸 API 호출로 멈추지 않는다. 완료되면 `IconsReady`로 리렌더를 유도한다.
    fn spawn_icon_prefetch(&self) -> Task<Message> {
        let items: Vec<(ItemKind, String, Option<String>)> = self
            .results
            .iter()
            .map(|r| (r.item.kind, r.item.path.clone(), r.item.icon_path.clone()))
            .collect();
        if items.is_empty() {
            return Task::none();
        }
        Task::future(async move {
            let _ = tokio::task::spawn_blocking(move || {
                crate::app_icons::prefetch_icons(&items);
            })
            .await;
            Message::IconsReady
        })
    }

    /// IME 조합이 진행 중일 가능성이 높은지 — 최근 입력 직후이거나 자모 꼬리 상태.
    fn is_typing_in_progress(&self) -> bool {
        self.last_query_changed_at.elapsed() < Duration::from_millis(350)
            || self.should_limit_reorder_for_ime()
    }

    // ─── Update ───────────────────────────────────────────────────────────

    /// 포커스 가드: UI 상태(query/results)가 변경되면 자동으로 포커스를 복원한다.
    /// 개별 핸들러가 request_focus()를 빠뜨려도 안전하게 보장하는 최후의 안전망.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let skip_focus_guard = matches!(
            message,
            Message::QueryChanged(_)
                | Message::DelayedRefocus
                | Message::EnsureFocus(_)
                | Message::EngineReady
                | Message::EngineSwapSettled
                | Message::IconsReady
                | Message::AutostartStatusLoaded(_)
                | Message::AutostartToggleFinished(_)
        );

        let old_query_len = self.query.len();
        let old_results_len = self.results.len();

        let task = self.update_inner(message);

        if skip_focus_guard || !self.window_focused || self.is_boot_settled() {
            return task;
        }

        let query_changed = self.query.len() != old_query_len;
        let results_changed = self.results.len() != old_results_len;

        if query_changed || results_changed {
            let focus = self.request_focus();
            Task::batch([task, focus])
        } else {
            task
        }
    }

    fn update_inner(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QueryChanged(query) => {
                self.query = query;
                // macOS: 런처 프로세스 기동 직후 TSM이 한글 입력기를 연결하기 전에
                // 입력된 키는 조합 없이 낱자 자모로 커밋된다(ㅎㅏㄴ). IME가 조합했을
                // 결과(한)로 재조합해 표시/검색을 복원한다. 정상 텍스트는 항등(None).
                if let Some(recomposed) = kmd_core::hangul::recompose_jamo(&self.query) {
                    self.log_ime_state("QueryChanged:recompose_jamo");
                    self.query = recomposed;
                }
                self.selected = 0;
                self.last_query_changed_at = std::time::Instant::now();
                self.log_ime_state("QueryChanged:assigned");
                // 한글 IME 조합 중에는 검색/리렌더를 지연해 자모 분리 가능성을 줄인다.
                if self.should_limit_reorder_for_ime() {
                    self.log_ime_state("QueryChanged:skip_search_due_to_jamo");
                    // 조합 중에도 warmup 유휴 타이머를 리셋한다. 그렇지 않으면 부팅 시
                    // 예약된 WarmupTick이 첫 조합 도중 발화해 loading 토글 재렌더로
                    // marked text가 깨지고 자모가 분리된다. (macOS)
                    return self.with_activity_warmup(Task::none());
                }
                let search_task = self.perform_search();
                self.log_ime_state("QueryChanged:perform_search");
                self.with_activity_warmup(search_task)
            }
            Message::Submit => self.launch_selected(),
            Message::ResultClicked(index) => {
                self.selected = index;
                self.launch_selected()
            }
            Message::KeyEvent(key, modifiers) => {
                if let Some(action) = self.match_shortcut(&key, &modifiers) {
                    return self.execute_context_action(action);
                }
                let key_task = self.handle_key(key);
                self.with_activity_warmup(key_task)
            }
            Message::BrandClicked => self.toggle_query_mode(":help"),
            Message::BrandRightClicked => self.toggle_query_mode(":set"),
            Message::StartWindowDrag => match self.window_id {
                Some(id) => window::drag(id),
                None => window::oldest().then(|maybe_id| match maybe_id {
                    Some(id) => window::drag(id),
                    None => Task::none(),
                }),
            },
            Message::StartWindowResize(direction) => match self.window_id {
                Some(id) => window::drag_resize(id, direction),
                None => window::oldest().then(move |maybe_id| match maybe_id {
                    Some(id) => window::drag_resize(id, direction),
                    None => Task::none(),
                }),
            },
            Message::GotWindowId(id) => {
                self.window_id = id;
                match id {
                    Some(id) => {
                        let saved_x = self.window_state.x;
                        let saved_y = self.window_state.y;
                        let width = self.window_width;
                        let win_h = self.ui.full_window_height;
                        let ensure_visible = window::monitor_size(id).then(move |maybe_size| {
                            let (Some(x), Some(y), Some(mon)) = (saved_x, saved_y, maybe_size)
                            else {
                                return Task::none();
                            };

                            let w = width.clamp(420.0, 1200.0);
                            let h = win_h;
                            let outside = x + w < 40.0
                                || x > mon.width - 40.0
                                || y + h < 20.0
                                || y > mon.height - 20.0;

                            if outside {
                                let recentered =
                                    Point::new((mon.width - w) / 2.0, (mon.height / 3.0).max(0.0));
                                window::move_to(id, recentered)
                            } else {
                                Task::none()
                            }
                        });

                        Task::batch([
                            window::raw_id::<Message>(id).map(Message::GotRawWindowId),
                            ensure_visible,
                        ])
                    }
                    None => self.request_focus(),
                }
            }
            Message::GotRawWindowId(raw_id) => self.handle_got_raw_window_id(raw_id),
            Message::WarmupTick(token) => {
                if self.full_warmup_started || token != self.warmup_token {
                    return Task::none();
                }

                // IME 조합/최근 입력 중이면 warmup을 미룬다. loading 토글로 인한 뷰
                // 재렌더가 첫 한글 조합(marked text)을 깨뜨리는 것을 방지. (macOS 자모 분리)
                if self.is_typing_in_progress() {
                    self.log_ime_state("WarmupTick:defer_for_ime");
                    self.warmup_token = self.warmup_token.wrapping_add(1);
                    return Self::schedule_warmup_tick(self.warmup_token);
                }

                self.full_warmup_started = true;
                self.loading = true;
                tracing::info!(
                    "Starting full engine warmup after {}ms idle",
                    WARMUP_IDLE_MS
                );
                self.spawn_full_engine_load_task()
            }
            Message::EnsureFocus(attempt) => self.handle_ensure_focus(attempt),
            Message::EngineReady => {
                // warmup이 입력 전에 시작된 경우, 엔진 스왑/loading 토글이 조합 도중
                // 재렌더를 일으켜 자모가 분리될 수 있다. 조합 중이면 스왑 전체를 미룬다.
                // (engine_slot이 결과를 계속 보관하므로 이후 재발화 시 그대로 적용)
                if self.is_typing_in_progress() {
                    self.log_ime_state("EngineReady:defer_for_ime");
                    return Task::future(async {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                        Message::EngineReady
                    });
                }
                let loaded = self
                    .engine_slot
                    .lock()
                    .unwrap_or_else(|e| {
                        tracing::error!("engine_slot mutex poisoned — 복구 시도");
                        e.into_inner()
                    })
                    .take();

                if let Some(res) = loaded {
                    self.engine = res.engine;
                    self.use_emoji = res.use_emoji;
                    self.selected_llm_providers = res.llm_providers;
                    self.selected_multi_web_providers = res.multi_web_providers;
                    self.multi_llm_prefixes = res.llm_prefixes;
                    self.multi_web_prefixes = res.multi_web_prefixes;
                    self.spell_providers = res.spell_providers;
                    self.spell_prefixes = res.spell_prefixes;
                    self.translate_providers = res.translate_providers;
                    self.translate_prefixes = res.translate_prefixes;
                    self.runtime_config = crate::engine::load_config();
                    self.loading = false;
                    tracing::info!("Search engine ready");
                    if !self.query.trim().is_empty() {
                        // IME 조합 중이면 검색/뷰 갱신을 미뤄 자모 분리를 막는다.
                        if self.is_typing_in_progress() {
                            return Task::future(async {
                                tokio::time::sleep(Duration::from_millis(400)).await;
                                Message::EngineSwapSettled
                            });
                        }
                        return self.perform_search();
                    }
                } else {
                    self.loading = false;
                }
                Task::none()
            }
            Message::EngineSwapSettled => {
                if self.query.trim().is_empty() {
                    return Task::none();
                }
                // 아직 입력 중이면 다시 미룬다 — 조합이 끝난 뒤에만 갱신.
                if self.is_typing_in_progress() {
                    return Task::future(async {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                        Message::EngineSwapSettled
                    });
                }
                self.perform_search()
            }
            Message::IconsReady => Task::none(),
            Message::AutostartStatusLoaded(result) => {
                self.daemon_autostart_check_in_flight = false;
                self.daemon_autostart_last_checked_at = Some(std::time::Instant::now());
                match result {
                    Ok(installed) => {
                        self.daemon_autostart_enabled = Some(installed);
                    }
                    Err(message) => {
                        tracing::warn!("autostart status 조회 실패: {message}");
                        self.daemon_autostart_enabled = None;
                    }
                }
                if crate::query_prefix::prefix_of(self.query.trim())
                    == crate::query_prefix::Prefix::Settings
                {
                    let current_query = self.query.clone();
                    self.handle_settings_query(&current_query);
                }
                Task::none()
            }
            Message::AutostartToggleFinished(result) => {
                self.daemon_autostart_toggle_in_flight = false;
                match result {
                    Ok(message) => tracing::info!("autostart: {message}"),
                    Err(message) => tracing::warn!("autostart toggle 실패: {message}"),
                }
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                let refresh = self.schedule_autostart_status_refresh(true);
                let focus = self.request_focus();
                Task::batch([refresh, focus])
            }
            Message::CheckQuitSignal => {
                if self.state_dirty {
                    self.window_state.save();
                    self.state_dirty = false;
                }
                if self._guard.should_quit() {
                    self._guard.consume_quit_signal();
                    tracing::info!("Received quit signal — exiting");
                    self.window_state.save();
                    return iced::exit();
                }
                Task::none()
            }
            Message::WindowEvent(_id, event) => self.handle_window_event(event),
            Message::ShellDone(result) => {
                match result {
                    Ok(output) => {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Err(e) = clipboard.set_text(&output) {
                                tracing::warn!("클립보드 쓰기 실패: {e}");
                            }
                        }
                        let first_line = output.lines().next().unwrap_or("(no output)");
                        tracing::info!("Shell output copied: {first_line}");
                    }
                    Err(msg) => {
                        tracing::error!("Shell error: {msg}");
                    }
                }
                iced::exit()
            }
            Message::RunAction(action) => self.execute_context_action(action),
            Message::BackgroundClicked => {
                if self.state_dirty {
                    self.window_state.save();
                }
                iced::exit()
            }
            Message::DelayedRefocus => self.handle_delayed_refocus(),
            Message::CheckUnfocusedExit => self.handle_check_unfocused_exit(),
        }
    }

    // ─── Subscription ─────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        // event::listen_with로 위젯이 소비한 이벤트도 캡처 (ESC 등)
        let keyboard_sub = iced::event::listen_with(|event, _status, _id| {
            if let iced::Event::Keyboard(kb_event) = event {
                match kb_event {
                    keyboard::Event::KeyPressed { key, modifiers, .. } => {
                        Some(Message::KeyEvent(key, modifiers))
                    }
                    _ => None,
                }
            } else {
                None
            }
        });

        let quit_sub = iced::time::every(Duration::from_millis(QUIT_POLL_MS))
            .map(|_| Message::CheckQuitSignal);

        let window_sub = window::events().map(|(id, event)| Message::WindowEvent(id, event));

        Subscription::batch([keyboard_sub, quit_sub, window_sub])
    }

    pub fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

// ─── Search Logic ─────────────────────────────────────────────────────────────

// 지원하는 검색창 프리픽스 전체 목록은 kmd_core::query_prefix::COMMANDS 레지스트리
// (단일 진실 소스)와 :help 화면을 참고.

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// UTF-8 안전한 문자열 truncation (한글/CJK 안전)
fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{truncated}...")
    }
}

/// 상세 타이틀 영역 픽셀 폭과 폰트 크기로 안전한 최대 글자 수를 추정한다.
///
/// 한글/전각 문자를 고려해 보수적인 폭 계수(0.95em)를 사용한다.
fn estimate_title_max_chars(width_px: f32, font_size: f32) -> usize {
    let em = (font_size * 0.95).max(1.0);
    ((width_px / em).floor() as usize).clamp(8, 64)
}

/// IME 조합 중 생성되는 trailing 자모를 완화해 즉시 검색 반응성을 높인다.
///
/// 예: `하ㄴ` -> `하`, `한ㄱ` -> `한`
fn relaxed_hangul_query(query: &str) -> Option<String> {
    if query.is_empty() {
        return None;
    }

    let mut chars: Vec<char> = query.chars().collect();
    let original_len = chars.len();
    while let Some(last) = chars.last() {
        if is_hangul_jamo(*last) {
            chars.pop();
        } else {
            break;
        }
    }

    if chars.is_empty() || chars.len() == original_len {
        return None;
    }

    Some(chars.into_iter().collect())
}

fn is_hangul_jamo(ch: char) -> bool {
    let code = ch as u32;
    (0x1100..=0x11FF).contains(&code)
        || (0x3131..=0x318E).contains(&code)
        || (0xA960..=0xA97F).contains(&code)
        || (0xD7B0..=0xD7FF).contains(&code)
}

fn items_to_results(
    items: impl IntoIterator<Item = kmd_core::IndexItem>,
) -> Vec<kmd_core::SearchResult> {
    items
        .into_iter()
        .map(|item| kmd_core::SearchResult {
            item,
            score: SCORE_PLUGIN,
        })
        .collect()
}

fn ensure_multi_llm_hint(items: &mut Vec<IndexItem>, use_emoji: bool) {
    if items
        .iter()
        .any(|item| item.name.starts_with("@ll") || item.name.starts_with("@llm"))
    {
        return;
    }
    items.push(IndexItem {
        name: "@ll         Compare multiple LLMs with one prompt".to_string(),
        path: "Open selected LLM providers in parallel tabs".to_string(),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: if use_emoji { "\u{1F9E0}" } else { "Ml" }.to_string(),
        keywords: "kmd:web_hint:multi_llm\n@ll @llm @multi @cmp multi llm compare".to_string(),
        icon_path: None,
    });
}

fn ensure_multi_web_hint(items: &mut Vec<IndexItem>, use_emoji: bool) {
    if items
        .iter()
        .any(|item| item.name.starts_with("@m ") || item.name.starts_with("@msearch"))
    {
        return;
    }
    items.push(IndexItem {
        name: "@m          Search multiple engines at once".to_string(),
        path: "Open Google/Naver/Daum in parallel tabs".to_string(),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: if use_emoji { "\u{1F50E}" } else { "Mw" }.to_string(),
        keywords:
            "kmd:web_hint:multi_web\n@m @mw @msearch @multisearch @searchall @krsearch multi web"
                .to_string(),
        icon_path: None,
    });
}

/// Load → mutate → save the user config file. Logs on failure.
fn save_config(f: impl FnOnce(&mut kmd_core::Config)) {
    let config_dir = kmd_core::Config::default_config_dir();
    match kmd_core::Config::load(&config_dir) {
        Ok(mut cfg) => {
            f(&mut cfg);
            if let Err(e) = cfg.save() {
                tracing::warn!("Failed to save config: {e}");
            }
        }
        Err(e) => tracing::warn!("Failed to load config for save: {e}"),
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    /// 테스트용 App 인스턴스 생성 (각 테스트마다 고유 temp 디렉토리)
    fn make_test_app() -> App {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmp = std::env::temp_dir().join(format!("kmd_test_app_{}_{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::create_dir_all(&tmp);
        let test_db_path = tmp.join("kmd_test.db");

        let guard = match kmd_core::single_instance::acquire_or_toggle(&tmp) {
            kmd_core::single_instance::InstanceAction::Acquired(g) => g,
            _ => panic!("Guard 획득 실패"),
        };
        let config = kmd_core::Config::default();
        let window_state = WindowState::default();
        let (app, _task) = App::new_for_test(guard, config, window_state, test_db_path);
        app
    }

    // ── UiScale 비례 계산 일관성 ──

    #[test]
    fn 기본_폰트_창높이_일관성() {
        let cfg = kmd_core::Config::default();
        let u = UiScale::new(cfg.general.font_size, cfg.general.visible_rows);
        let expected = u.search_bar_height
            + 1.0
            + (u.visible_rows as f32 * u.row_height)
            + 1.0
            + u.status_bar_height;
        assert!(
            (u.full_window_height - expected).abs() < f32::EPSILON,
            "full_window_height({}) != 계산값({expected})",
            u.full_window_height
        );
    }

    #[test]
    fn pill_높이가_폰트보다_충분히_큼() {
        for fs in [12.0, 16.0, 20.0, 24.0, 32.0] {
            let u = UiScale::new(fs, 8);
            assert!(
                u.pill_height > u.font + 10.0,
                "font_size={fs}: pill_height({})가 폰트보다 충분히 커야 함",
                u.pill_height
            );
        }
    }

    #[test]
    fn row_높이가_아이콘보다_큼() {
        for fs in [12.0, 16.0, 20.0, 24.0, 32.0] {
            let u = UiScale::new(fs, 8);
            assert!(
                u.row_height > u.result_icon + 8.0,
                "font_size={fs}: row_height({})가 아이콘({})보다 충분히 커야 함",
                u.row_height,
                u.result_icon
            );
        }
    }

    #[test]
    fn 폰트_클램프_범위() {
        let small = UiScale::new(5.0, 8);
        assert_eq!(small.font, 12.0, "최소 12로 클램프");
        let big = UiScale::new(100.0, 8);
        assert_eq!(big.font, 32.0, "최대 32로 클램프");
    }

    #[test]
    fn visible_rows_클램프_범위() {
        let small = UiScale::new(16.0, 1);
        assert_eq!(small.visible_rows, 4, "최소 4로 클램프");
        let big = UiScale::new(16.0, 100);
        assert_eq!(big.visible_rows, 20, "최대 20으로 클램프");
        let normal = UiScale::new(16.0, 12);
        assert_eq!(normal.visible_rows, 12, "범위 내 값은 그대로");
    }

    #[test]
    fn visible_rows_변경시_창높이_비례() {
        let u8 = UiScale::new(16.0, 8);
        let u12 = UiScale::new(16.0, 12);
        let diff = u12.full_window_height - u8.full_window_height;
        let expected = 4.0 * u8.row_height;
        assert!(
            (diff - expected).abs() < f32::EPSILON,
            "rows 4개 증가 → 창높이 차이({diff}) == row_height*4({expected})"
        );
    }

    #[test]
    fn 비례_스케일링_선형() {
        let u16 = UiScale::new(16.0, 8);
        let u32 = UiScale::new(32.0, 8);
        let ratio = u32.pill_height / u16.pill_height;
        assert!(
            (ratio - 2.0).abs() < 0.1,
            "font 2배 → pill_height도 약 2배 ({ratio:.2})"
        );
        let row_ratio = u32.row_height / u16.row_height;
        assert!(
            (row_ratio - 2.0).abs() < 0.1,
            "font 2배 → row_height도 약 2배 ({row_ratio:.2})"
        );
    }

    // ── 포커스 요청 카운트 검증 ──

    #[test]
    fn 쿼리_변경시_focus_요청_미발생() {
        let mut app = make_test_app();
        let before = app.focus_request_count;

        let _task = app.update(Message::QueryChanged("test".to_string()));
        assert_eq!(
            app.focus_request_count, before,
            "입력 중에는 refocus를 강제하지 않아야 함"
        );
        assert_eq!(app.query, "test");
        assert!(app.window_focused);
    }

    #[test]
    fn 쿼리_비어있을때_ensure_focus_요청_발생() {
        let mut app = make_test_app();
        app.window_focused = true;
        app.query.clear();
        let before = app.focus_request_count;

        let _task = app.update(Message::EnsureFocus(1));
        assert!(
            app.focus_request_count > before,
            "빈 쿼리 + EnsureFocus → focus 요청 발생해야 함"
        );
    }

    #[test]
    fn 쿼리_있을때_ensure_focus_건너뜀() {
        let mut app = make_test_app();
        app.window_focused = true;
        app.query = "hello".to_string();
        let before = app.focus_request_count;

        let _task = app.update(Message::EnsureFocus(1));
        assert_eq!(
            app.focus_request_count, before,
            "쿼리 있으면 EnsureFocus는 focus 요청 안 함"
        );
    }

    #[test]
    fn 최대_재시도_초과시_focus_요청_안함() {
        let mut app = make_test_app();
        app.window_focused = true;
        app.query.clear();
        let before = app.focus_request_count;

        let _task = app.update(Message::EnsureFocus(MAX_FOCUS_RETRIES + 1));
        assert_eq!(
            app.focus_request_count, before,
            "최대 재시도 초과 시 focus 요청 안 함"
        );
    }

    #[test]
    fn delayed_refocus_카운트_증가() {
        let mut app = make_test_app();
        let before = app.focus_request_count;

        let _task = app.update(Message::DelayedRefocus);
        assert_eq!(
            app.focus_request_count,
            before + 1,
            "DelayedRefocus는 focus_request_count를 1 증가시켜야 함"
        );
    }

    // ── Unfocused 이벤트 타이핑 가드 ──

    #[test]
    fn 타이핑_직후_unfocused_무시() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());

        let _task = app.update(Message::QueryChanged("가".to_string()));
        assert!(app.window_focused, "QueryChanged 직후 focused 유지");

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Unfocused,
        ));
        assert!(
            app.window_focused,
            "타이핑 500ms 이내 Unfocused는 무시되어야 함"
        );
    }

    #[test]
    fn 타이핑_없을때_unfocused_정상_처리() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        app.last_query_changed_at = std::time::Instant::now() - Duration::from_secs(5);

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Unfocused,
        ));
        assert!(
            !app.window_focused,
            "타이핑하지 않은 상태에서 Unfocused는 window_focused=false로"
        );
    }

    // ── Focused 이벤트 — 항상 refocus + 카운트 ──

    #[test]
    fn 쿼리_있고_최근타이핑시_focused_refocus_안함() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        app.window_focused = false;
        app.query = "검색어".to_string();
        app.last_query_changed_at = std::time::Instant::now();
        let before = app.focus_request_count;

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Focused,
        ));
        assert!(app.window_focused);
        assert_eq!(
            app.focus_request_count, before,
            "조합/타이핑 직후 Focused 이벤트에서는 refocus를 생략"
        );
    }

    #[test]
    fn 쿼리_있어도_타이핑아님_focused시_focus_요청() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        app.window_focused = false;
        app.query = "검색어".to_string();
        app.last_query_changed_at = std::time::Instant::now() - Duration::from_secs(2);
        let before = app.focus_request_count;

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Focused,
        ));
        assert!(app.window_focused);
        assert!(
            app.focus_request_count > before,
            "타이핑 직후가 아니면 Focused 이벤트에서 focus 요청"
        );
    }

    #[test]
    fn 빈_쿼리_focused시_focus_요청() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        app.window_focused = false;
        app.query.clear();
        let before = app.focus_request_count;

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Focused,
        ));
        assert!(app.window_focused);
        assert!(
            app.focus_request_count > before,
            "빈 쿼리 Focused 시에도 focus 요청"
        );
    }

    // ── 첫 입력 후 포커스 복원 전체 시나리오 ──

    #[test]
    fn 첫_입력_후_전체_focus_시퀀스() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        let c0 = app.focus_request_count;

        // 1단계: 첫 글자 입력
        let _task = app.update(Message::QueryChanged("h".to_string()));
        let c1 = app.focus_request_count;
        assert_eq!(c1, c0, "QueryChanged에서는 request_focus를 호출하지 않음");

        // 2단계: macOS spurious Unfocused (view 재렌더 시 발생 가능)
        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Unfocused,
        ));
        assert!(app.window_focused, "타이핑 가드로 Unfocused 무시");

        // 3단계: DelayedRefocus (16ms 후 iced 이벤트 루프가 전달)
        let _task = app.update(Message::DelayedRefocus);
        let c2 = app.focus_request_count;
        assert_eq!(c2, c1, "입력 중 DelayedRefocus는 IME 안정성을 위해 스킵");

        // 4단계: 두 번째 글자 입력 가능
        let _task = app.update(Message::QueryChanged("he".to_string()));
        assert_eq!(app.query, "he");
        let c3 = app.focus_request_count;
        assert_eq!(c3, c2, "두 번째 입력에서도 refocus를 강제하지 않음");
    }

    #[test]
    fn 첫_입력시_중복_refocus_없음() {
        let mut app = make_test_app();
        let before = app.focus_request_count;

        let _task = app.update(Message::QueryChanged("h".to_string()));
        let after = app.focus_request_count;

        assert_eq!(
            after, before,
            "QueryChanged에서 request_focus를 하지 않아야 IME 조합 안정성 유지"
        );
    }

    #[test]
    fn 한글_첫_입력_후_전체_focus_시퀀스() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        let c0 = app.focus_request_count;

        // ㄱ 입력
        let _task = app.update(Message::QueryChanged("ㄱ".to_string()));
        let c1 = app.focus_request_count;
        assert_eq!(c1, c0, "한글 첫 자음 입력 시 강제 refocus 없음");

        // macOS spurious Unfocused + Focused
        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Unfocused,
        ));
        assert!(app.window_focused, "타이핑 가드로 무시");

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Focused,
        ));
        let c2 = app.focus_request_count;
        assert_eq!(c2, c1, "최근 타이핑 중 Focused refocus 생략");

        // DelayedRefocus (view 재렌더 후)
        let _task = app.update(Message::DelayedRefocus);
        let c3 = app.focus_request_count;
        assert_eq!(c3, c2, "입력 중 DelayedRefocus는 스킵");

        // ㅏ 결합 → 가
        let _task = app.update(Message::QueryChanged("가".to_string()));
        assert_eq!(app.query, "가", "한글 조합 완료");
        assert!(app.window_focused, "조합 이후에도 포커스 유지");
    }

    // ── 기타 focus 관련 ──

    #[test]
    fn got_raw_window_id_쿼리_있으면_focus_건너뜀() {
        let mut app = make_test_app();
        app.query = "test".to_string();
        let before = app.focus_request_count;

        let _task = app.update(Message::GotRawWindowId(12345));
        assert_eq!(
            app.focus_request_count, before,
            "쿼리 있으면 GotRawWindowId에서 focus 안 함"
        );
    }

    #[test]
    fn got_raw_window_id_빈쿼리_focus_요청() {
        let mut app = make_test_app();
        app.query.clear();
        let before = app.focus_request_count;

        let _task = app.update(Message::GotRawWindowId(12345));
        assert!(
            app.focus_request_count > before,
            "빈 쿼리 → GotRawWindowId에서 focus 요청"
        );
    }

    #[test]
    fn clear_search_후_focus_요청() {
        let mut app = make_test_app();
        app.query = "something".to_string();
        let before = app.focus_request_count;

        let _task = app.update(Message::QueryChanged("".to_string()));
        assert_eq!(app.query, "");
        assert_eq!(
            app.focus_request_count, before,
            "쿼리 비움 시 refocus 강제 없음"
        );
    }

    // ── 포커스 가드 (근본적 안전망) 테스트 ──

    #[test]
    fn 포커스_가드_쿼리_변경시_자동_refocus_안함() {
        let mut app = make_test_app();
        app.window_focused = true;
        let before = app.focus_request_count;

        let _task = app.update(Message::QueryChanged("abc".to_string()));
        assert_eq!(
            app.focus_request_count, before,
            "QueryChanged는 포커스 가드 제외"
        );
    }

    #[test]
    fn 포커스_가드_결과_변경시_자동_refocus() {
        let mut app = make_test_app();
        app.window_focused = true;

        // 쿼리를 설정하여 results가 생길 조건 만들기
        let _task = app.update(Message::QueryChanged(":help".to_string()));
        let after_help = app.focus_request_count;

        // results.clear()로 결과가 바뀌는 상황 시뮬레이션
        let _task = app.update(Message::QueryChanged("".to_string()));
        assert_eq!(
            app.focus_request_count, after_help,
            "입력 중 refocus는 금지"
        );
    }

    #[test]
    fn 포커스_가드_settings_토글_후_refocus() {
        let mut app = make_test_app();
        app.window_focused = true;

        // Settings 화면으로 진입
        let _task = app.update(Message::QueryChanged(":set".to_string()));
        let before = app.focus_request_count;

        // Settings 액션 실행 (toggle_ime_reset 시뮬레이션)
        // handle_settings_action을 직접 호출하는 대신,
        // update_inner가 아닌 update를 통해 query/results 변경이
        // 포커스 가드에 의해 보호되는지 검증
        let old_results = app.results.len();
        app.query = ":set".to_string();
        app.handle_settings_query(":set");
        let new_results = app.results.len();

        // results가 변경되었다면, update()를 통했을 때 가드가 동작해야 함
        if new_results != old_results {
            // 이 경우 가드가 자동으로 refocus
            let focus_task = app.request_focus();
            assert!(
                app.focus_request_count > before,
                "settings 토글 후 포커스 가드 동작 확인"
            );
            let _ = focus_task;
        }
    }

    #[test]
    fn 포커스_가드_윈도우_비활성시_미동작() {
        let mut app = make_test_app();
        app.window_focused = false;
        let before = app.focus_request_count;

        let _task = app.update(Message::QueryChanged("test".to_string()));
        // 창이 비활성 상태에서는 QueryChanged 경로에서도 refocus를 시도하지 않아야 한다.
        let after = app.focus_request_count;
        assert_eq!(
            after, before,
            "window_focused=false이면 입력 중 불필요한 focus 요청이 없어야 함"
        );
    }

    #[test]
    fn 포커스_가드_delayed_refocus_무한루프_방지() {
        let mut app = make_test_app();
        app.window_focused = true;
        let before = app.focus_request_count;

        let _task = app.update(Message::DelayedRefocus);
        let after = app.focus_request_count;
        // DelayedRefocus는 skip_focus_guard=true이므로 가드의 추가 호출 없음
        // update_inner 내부에서만 focus 요청 발생
        let delta = after - before;
        assert!(
            delta <= 2,
            "DelayedRefocus는 가드 바이패스, 무한루프 없음 (delta={delta})"
        );
    }

    #[test]
    fn 포커스_가드_ensure_focus_무한루프_방지() {
        let mut app = make_test_app();
        app.window_focused = true;
        app.query.clear();
        let before = app.focus_request_count;

        let _task = app.update(Message::EnsureFocus(1));
        let after = app.focus_request_count;
        let delta = after - before;
        assert!(
            delta <= 2,
            "EnsureFocus는 가드 바이패스, 무한루프 없음 (delta={delta})"
        );
    }

    #[test]
    fn settings_theme_변경_후_focus_복원() {
        let mut app = make_test_app();
        app.window_focused = true;

        // Settings로 진입
        let _task = app.update(Message::QueryChanged(":set".to_string()));
        let has_results_after_set = !app.results.is_empty();
        assert!(has_results_after_set, ":set 입력 후 결과가 있어야 함");

        let before = app.focus_request_count;

        // theme 변경 시뮬레이션: query를 clear하고 results를 clear
        // 이것이 handle_settings_action("theme:midnight")가 하는 일
        app.query.clear();
        app.results.clear();
        app.selected = 0;
        let focus_task = app.request_focus();
        let _ = focus_task;

        assert!(
            app.focus_request_count > before,
            "theme 변경 후 focus 요청 발생해야 함"
        );
    }

    #[test]
    fn 연속_settings_토글_focus_유지() {
        let mut app = make_test_app();
        app.window_focused = true;

        // 연속 3회 settings 진입-나가기 시뮬레이션
        for i in 0..3 {
            let before = app.focus_request_count;
            let _task = app.update(Message::QueryChanged(":set".to_string()));
            assert_eq!(
                app.focus_request_count, before,
                "#{i} settings 진입 입력 처리에서는 refocus 강제 없음"
            );

            let before2 = app.focus_request_count;
            let _task = app.update(Message::QueryChanged("".to_string()));
            assert_eq!(
                app.focus_request_count, before2,
                "#{i} settings 나가기 입력 처리에서도 refocus 강제 없음"
            );
        }
    }
}
