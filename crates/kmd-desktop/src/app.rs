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

use iced::keyboard;
use iced::widget::operation::scroll_to;
use iced::widget::scrollable as scrollable_mod;
use iced::widget::text::Wrapping;
use iced::widget::{
    column, container, image, mouse_area, row, scrollable, text, text_input, Column, Space,
};
use iced::{
    window, Background, Border, Color, Element, Fill, Length, Padding, Point, Shadow, Size,
    Subscription, Task, Vector,
};

use kmd_core::plugin::{builtin_calc, builtin_emoji, builtin_shell, Extension};
use kmd_core::single_instance::Guard;
use kmd_core::web;
use kmd_core::{IndexItem, ItemKind, Source};

use crate::theme::DesktopTheme;
use crate::window_state::WindowState;

// ─── Constants ────────────────────────────────────────────────────────────────

pub const DEFAULT_WIDTH: f32 = 680.0;
const PILL_HEIGHT: f32 = 42.0;
const DRAG_STRIP: f32 = 6.0;
pub const SEARCH_BAR_HEIGHT: f32 = PILL_HEIGHT + DRAG_STRIP;
const ROW_HEIGHT: f32 = 48.0;
const STATUS_BAR_HEIGHT: f32 = 28.0;
const MAX_VISIBLE_ROWS: usize = 8;
const SEARCH_LIMIT: usize = 50;
const SCORE_PLUGIN: u32 = u32::MAX;
/// 창은 처음부터 전체 높이로 열린다 — 런타임 resize 없이 콘텐츠만 전환.
/// 기본 = pill 검색바만 표시, 결과 시 같은 카드가 아래로 확장되어 보임.
pub const FULL_WINDOW_HEIGHT: f32 =
    SEARCH_BAR_HEIGHT + 1.0 + (MAX_VISIBLE_ROWS as f32 * ROW_HEIGHT) + 1.0 + STATUS_BAR_HEIGHT;

const QUIT_POLL_MS: u64 = 300;
const WARMUP_IDLE_MS: u64 = 400;
const FOCUS_RETRY_MS: u64 = 120;
const MAX_FOCUS_RETRIES: u8 = 3;
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
    loading: bool,
    engine_slot: EngineSlot,
    _guard: Guard,

    // ── Window geometry ───────────────────────────────────────────────
    window_width: f32,
    window_state: WindowState,
    state_dirty: bool,
    window_focused: bool,
    last_query_changed_at: std::time::Instant,

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
}

// ─── Context Actions ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ContextAction {
    Open,
    OpenAsAdmin,
    OpenFolder,
    CopyPath,
    CopyName,
    #[allow(dead_code)]
    Uninstall,
}

impl ContextAction {
    fn label(&self) -> &str {
        match self {
            Self::Open => "열기",
            Self::OpenAsAdmin => "관리자 권한으로 열기",
            Self::OpenFolder => "파일 위치 열기",
            Self::CopyPath => "경로 복사",
            Self::CopyName => "이름 복사",
            Self::Uninstall => "프로그램 제거",
        }
    }

    fn shortcut(&self) -> &str {
        match self {
            Self::Open => "Ctrl+1",
            Self::OpenAsAdmin => "Ctrl+Shift+Enter",
            Self::OpenFolder => "Ctrl+2",
            Self::CopyPath => "Ctrl+3",
            Self::CopyName => "Ctrl+4",
            Self::Uninstall => "",
        }
    }

    #[allow(dead_code)]
    fn icon_char(&self) -> &str {
        match self {
            Self::Open => "\u{2197}",        // ↗
            Self::OpenAsAdmin => "\u{26A1}", // ⚡
            Self::OpenFolder => "\u{1F4C2}", // 📂
            Self::CopyPath => "\u{1F4CB}",   // 📋
            Self::CopyName => "\u{270D}",    // ✍
            Self::Uninstall => "\u{1F5D1}",  // 🗑
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
    fn build_initial_engine(config: &kmd_core::Config) -> (kmd_core::SearchEngine, bool) {
        let cache_fresh = crate::engine::is_full_index_cache_fresh();
        let engine = if cache_fresh {
            tracing::info!("Full index cache is fresh — loading directly");
            crate::engine::create_search_engine(config)
        } else {
            tracing::info!("Full index cache stale/missing — using quick index");
            crate::engine::create_quick_search_engine(config)
        };
        (engine, cache_fresh)
    }

    fn initial_boot_tasks(input_id: iced::widget::Id, skip_warmup: bool) -> Task<Message> {
        let focus_task = iced::widget::operation::focus::<Message>(input_id);
        let id_task = window::oldest().map(Message::GotWindowId);
        if skip_warmup {
            Task::batch([focus_task, id_task])
        } else {
            let warmup_task = Self::schedule_warmup_tick(0);
            Task::batch([focus_task, id_task, warmup_task])
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
        self.results = items_to_results(items);
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    pub fn new(
        guard: Guard,
        config: kmd_core::Config,
        window_state: WindowState,
    ) -> (Self, Task<Message>) {
        // 캐시된 full index가 24시간 이내 → 직접 로드 (2-stage 불필요)
        // 캐시 없거나 오래됨 → quick index로 즉시 표시 후 full index를 비동기 빌드
        let (engine, cache_fresh) = Self::build_initial_engine(&config);
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
            loading: false,
            engine_slot,
            _guard: guard,
            window_width,
            window_state,
            state_dirty: false,
            window_focused: true,
            last_query_changed_at: std::time::Instant::now(),
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

    // ─── Update ───────────────────────────────────────────────────────────

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QueryChanged(query) => {
                self.query = query;
                self.selected = 0;
                self.last_query_changed_at = std::time::Instant::now();
                let search_task = self.perform_search();
                let refocus =
                    iced::widget::operation::focus::<Message>(self.input_id.clone());
                Task::batch([self.with_activity_warmup(search_task), refocus])
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
            Message::BrandClicked => {
                if self.query.starts_with(":help") {
                    self.clear_query_and_refocus()
                } else {
                    self.query = ":help".to_string();
                    self.perform_search()
                }
            }
            Message::BrandRightClicked => {
                if self.query.starts_with(":set") {
                    self.clear_query_and_refocus()
                } else {
                    self.query = ":set".to_string();
                    self.perform_search()
                }
            }
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
                        let ensure_visible = window::monitor_size(id).then(move |maybe_size| {
                            let (Some(x), Some(y), Some(mon)) = (saved_x, saved_y, maybe_size)
                            else {
                                return Task::none();
                            };

                            // If restored geometry is likely out of visible monitor bounds,
                            // recenter to keep the launcher discoverable.
                            let w = width.clamp(420.0, 1200.0);
                            let h = FULL_WINDOW_HEIGHT;
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
                    None => iced::widget::operation::focus::<Message>(self.input_id.clone()),
                }
            }
            Message::GotRawWindowId(raw_id) => {
                self.raw_window_id = Some(raw_id);
                crate::platform::force_square_corners(raw_id);
                crate::platform::force_foreground(raw_id);
                if self.reset_ime_on_launch {
                    crate::platform::force_english_ime(raw_id);
                }
                if self.query.is_empty() {
                    Task::batch([
                        iced::widget::operation::focus::<Message>(self.input_id.clone()),
                        Self::schedule_focus_retry(1),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::WarmupTick(token) => {
                if self.full_warmup_started || token != self.warmup_token {
                    return Task::none();
                }

                self.full_warmup_started = true;
                self.loading = true;
                tracing::info!(
                    "Starting full engine warmup after {}ms idle",
                    WARMUP_IDLE_MS
                );
                self.spawn_full_engine_load_task()
            }
            Message::EnsureFocus(attempt) => {
                if attempt > MAX_FOCUS_RETRIES || !self.query.is_empty() {
                    return Task::none();
                }

                if !self.window_focused {
                    #[cfg(target_os = "windows")]
                    {
                        if let Some(raw_id) = self.raw_window_id {
                            if !crate::platform::is_our_window_foreground(raw_id) {
                                return Task::none();
                            }
                        } else {
                            return Task::none();
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    return Task::none();
                }

                let focus_task = iced::widget::operation::focus::<Message>(self.input_id.clone());
                if attempt == MAX_FOCUS_RETRIES {
                    focus_task
                } else {
                    Task::batch([focus_task, Self::schedule_focus_retry(attempt + 1)])
                }
            }
            Message::EngineReady => {
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
                    self.loading = false;
                    tracing::info!("Search engine ready");
                    if !self.query.trim().is_empty() {
                        return self.perform_search();
                    }
                } else {
                    self.loading = false;
                }
                Task::none()
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
            Message::WindowEvent(_id, event) => {
                match event {
                    window::Event::Focused => {
                        self.window_focused = true;
                        let focus_now =
                            iced::widget::operation::focus::<Message>(self.input_id.clone());
                        if self.query.is_empty() {
                            let focus_retry = Self::schedule_focus_retry(1);
                            return Task::batch([focus_now, focus_retry]);
                        }
                        return focus_now;
                    }
                    window::Event::Moved(point) => {
                        self.window_state.x = Some(point.x);
                        self.window_state.y = Some(point.y);
                        self.state_dirty = true;
                    }
                    window::Event::Resized(size) => {
                        // 사용자 드래그 리사이즈 시 base width 저장
                        let w = size.width.max(420.0);
                        if (w - self.window_width).abs() > 1.0 {
                            self.window_width = w;
                            self.window_state.width = Some(w);
                            self.state_dirty = true;
                        }
                    }
                    window::Event::Unfocused => {
                        let typing_recently =
                            self.last_query_changed_at.elapsed() < Duration::from_millis(500);
                        if typing_recently {
                            return Task::none();
                        }
                        self.window_focused = false;
                        return Task::future(async {
                            tokio::time::sleep(Duration::from_millis(300)).await;
                            Message::CheckUnfocusedExit
                        });
                    }
                    _ => {}
                }
                Task::none()
            }
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
                return iced::exit();
            }
            Message::RunAction(action) => {
                return self.execute_context_action(action);
            }
            Message::BackgroundClicked => {
                if self.state_dirty {
                    self.window_state.save();
                }
                return iced::exit();
            }
            Message::CheckUnfocusedExit => {
                if self.window_focused {
                    return Task::none();
                }
                // Windows: GetForegroundWindow로 정확히 확인
                #[cfg(target_os = "windows")]
                if let Some(raw_id) = self.raw_window_id {
                    if crate::platform::is_our_window_foreground(raw_id) {
                        self.window_focused = true;
                        let focus_now =
                            iced::widget::operation::focus::<Message>(self.input_id.clone());
                        let focus_retry = Self::schedule_focus_retry(1);
                        return Task::batch([focus_now, focus_retry]);
                    }
                }
                // macOS/Linux: spurious unfocus는 이미 Unfocused 핸들러에서 차단했으므로
                // 여기까지 도달했다면 실제 포커스 유실이다.

                if self.state_dirty {
                    self.window_state.save();
                }
                return iced::exit();
            }
        }
    }

    fn execute_context_action(&mut self, action: ContextAction) -> Task<Message> {
        let Some(result) = self.results.get(self.selected) else {
            return Task::none();
        };
        let item = &result.item;

        match action {
            ContextAction::Open => {
                return self.launch_selected();
            }
            ContextAction::OpenAsAdmin => {
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                    let path = item.path.clone();
                    let _ = std::process::Command::new("powershell")
                        .args(["-Command", &format!("Start-Process '{}' -Verb RunAs", path)])
                        .creation_flags(CREATE_NO_WINDOW)
                        .spawn();
                    return iced::exit();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    return self.launch_selected();
                }
            }
            ContextAction::OpenFolder => {
                let path = std::path::Path::new(&item.path);
                let folder = if path.is_dir() {
                    item.path.clone()
                } else {
                    path.parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                if !folder.is_empty() {
                    if let kmd_core::action::ActionResult::Error(e) =
                        kmd_core::action::open_with_system(&folder)
                    {
                        tracing::warn!("폴더 열기 실패: {e}");
                    }
                }
                return iced::exit();
            }
            ContextAction::CopyPath => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Err(e) = clipboard.set_text(&item.path) {
                        tracing::warn!("클립보드 쓰기 실패: {e}");
                    }
                }
                return iced::exit();
            }
            ContextAction::CopyName => {
                let text = if item.kind == ItemKind::Calculator || item.kind == ItemKind::Emoji {
                    item.path.clone()
                } else {
                    item.name.clone()
                };
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Err(e) = clipboard.set_text(&text) {
                        tracing::warn!("클립보드 쓰기 실패: {e}");
                    }
                }
                return iced::exit();
            }
            ContextAction::Uninstall => {
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                    let _ = std::process::Command::new("control")
                        .arg("appwiz.cpl")
                        .creation_flags(CREATE_NO_WINDOW)
                        .spawn();
                    return iced::exit();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    return Task::none();
                }
            }
        }
    }

    // ─── Subscription ─────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => Message::KeyEvent(key, modifiers),
            keyboard::Event::KeyReleased { .. } | keyboard::Event::ModifiersChanged(_) => {
                Message::KeyEvent(keyboard::Key::Unidentified, keyboard::Modifiers::default())
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

/// All supported command prefixes for the search bar.
///
/// | Prefix    | Mode              | Example                        |
/// |-----------|-------------------|--------------------------------|
/// | `@`       | Web service       | `@g rust tutorial`, `@ai why`  |
/// | `:calc`   | Calculator        | `:calc (2+3)*4`                |
/// | `:emoji`  | Emoji search      | `:emoji fire`, `:e 하트`       |
/// | `:set`    | Settings          | `:set`, `:settings theme`      |
/// | `:keymap` | Keymap control    | `:keymap`, `:km on`, `:km off` |
/// | `:help`   | Help / commands   | `:help`, `:h`                  |
/// | `!`       | Shell command     | `!ip`, `!echo hello`           |
/// | (other)   | Fuzzy / glob / …  | `firefox`, `*.pdf`, `한글`     |
impl App {
    /// LLM 멀티 프롬프트를 클립보드에 복사 (템플릿 적용 포함)
    fn copy_multi_llm_prompt_to_clipboard(&self) {
        if let Some((_services, prompt)) = web::parse_multi_llm_query_with_prefixes(
            &self.query,
            &self.selected_llm_providers,
            &self.multi_llm_prefixes,
        ) {
            if !prompt.is_empty() {
                let config = crate::engine::load_config();
                let final_prompt =
                    kmd_core::prompt::apply_template(&config.launcher.prompt_templates, &prompt);
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Err(e) = clipboard.set_text(final_prompt) {
                        tracing::warn!("클립보드 쓰기 실패: {e}");
                    }
                }
            }
        }
    }

    fn perform_search(&mut self) -> Task<Message> {
        let query = self.query.clone();
        let trimmed = query.trim();

        if trimmed.is_empty() {
            self.results.clear();
            self.search_mode = kmd_core::SearchMode::Fuzzy;
            return Task::none();
        }

        match prefix_of(trimmed) {
            Prefix::Web => self.handle_web_query(trimmed),
            Prefix::Transform => self.handle_transform_query(trimmed),
            Prefix::Prompt => self.handle_prompt_query(trimmed),
            Prefix::Calc => self.handle_calc_query(trimmed),
            Prefix::Emoji => self.handle_emoji_query(trimmed),
            Prefix::Settings => self.handle_settings_query(trimmed),
            Prefix::Help => self.handle_help_query(),
            Prefix::Version => self.handle_version_query(),
            Prefix::Shell => self.handle_shell_query(trimmed),
            Prefix::Keymap => self.handle_keymap_query(trimmed),
            Prefix::General => self.handle_main_search(trimmed),
        }

        Task::none()
    }

    /// classify_web_query 통합 분류기 사용
    fn handle_web_query(&mut self, query: &str) {
        let emoji = self.use_emoji;
        let cfg = web::WebQueryConfig {
            spell_prefixes: &self.spell_prefixes,
            translate_prefixes: &self.translate_prefixes,
            multi_llm_prefixes: &self.multi_llm_prefixes,
            multi_llm_ids: &self.selected_llm_providers,
            multi_web_prefixes: &self.multi_web_prefixes,
            multi_web_ids: &self.selected_multi_web_providers,
        };

        match web::classify_web_query(query, &cfg) {
            web::WebQueryResult::Spell(q) => {
                self.results =
                    items_to_results(web::spell_result_items(&q, &self.spell_providers, emoji));
            }
            web::WebQueryResult::Translate(dir, q) => {
                self.results = items_to_results(web::translate_result_items(
                    &q,
                    dir,
                    &self.translate_providers,
                    emoji,
                ));
            }
            web::WebQueryResult::MultiLlm(_svcs, q) => {
                self.results = items_to_results(web::multi_llm_result_items(
                    &q,
                    &self.selected_llm_providers,
                    emoji,
                ));
            }
            web::WebQueryResult::MultiWeb(_svcs, q) => {
                self.results = items_to_results(web::multi_web_result_items(
                    &q,
                    &self.selected_multi_web_providers,
                    emoji,
                ));
            }
            web::WebQueryResult::Single(service, q) => {
                if q.is_empty() {
                    let mut items = web::list_services_as_items("", emoji);
                    ensure_multi_llm_hint(&mut items, emoji);
                    ensure_multi_web_hint(&mut items, emoji);
                    self.results = items_to_results(items);
                } else {
                    let item = web::search_result_item(service, &q, emoji);
                    self.results = items_to_results(std::iter::once(item));
                }
            }
            web::WebQueryResult::Browse(filter) => {
                let mut items = web::list_services_as_items(&filter, emoji);
                ensure_multi_llm_hint(&mut items, emoji);
                ensure_multi_web_hint(&mut items, emoji);
                self.results = items_to_results(items);
            }
        }
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_version_query(&mut self) {
        let emoji = self.use_emoji;
        let version_items = vec![
            IndexItem {
                name: format!("kmd-desktop {}", env!("CARGO_PKG_VERSION")),
                path: "Desktop launcher version".to_string(),
                icon: if emoji { "\u{1F4E6}" } else { "[VER]" }.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: "kmd:settings:noop".to_string(),
            },
            IndexItem {
                name: format!("kmd-core {}", kmd_core::Index::current_version()),
                path: "Search index schema version".to_string(),
                icon: if emoji { "\u{1F9E0}" } else { "[CORE]" }.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: "kmd:settings:noop".to_string(),
            },
            IndexItem {
                name: format!("target {}", std::env::consts::ARCH),
                path: format!("os {}", std::env::consts::OS),
                icon: if emoji { "\u{1F5A5}\u{FE0F}" } else { "[SYS]" }.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: "kmd:settings:noop".to_string(),
            },
        ];
        self.apply_contains_items(version_items);
    }

    /// :t / :transform 쿼리 처리 (클립보드 변환)
    fn handle_transform_query(&mut self, query: &str) {
        use kmd_core::transform;

        match transform::parse_transform_query(query) {
            Some(mut tq) => {
                // 텍스트가 비어있으면 클립보드에서 가져오기
                if tq.text.is_empty() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            tq.text = text;
                        }
                    }
                }
                if tq.text.is_empty() {
                    self.results = items_to_results(std::iter::once(IndexItem {
                        name: "❌ 클립보드가 비어 있습니다".to_string(),
                        path: "텍스트를 복사한 후 다시 시도하세요".to_string(),
                        kind: ItemKind::SystemCommand,
                        source: Source::Plugin,
                        icon: if self.use_emoji {
                            "\u{2139}\u{FE0F}"
                        } else {
                            "[!]"
                        }
                        .to_string(),
                        keywords: "kmd:settings:noop".to_string(),
                    }));
                    self.selected = 0;
                    return;
                }

                let urls = transform::build_transform_urls(
                    &tq,
                    &self.spell_providers,
                    &self.translate_providers,
                );
                for url in &urls {
                    if let kmd_core::action::ActionResult::Error(e) =
                        kmd_core::action::open_url(url)
                    {
                        tracing::warn!("URL 열기 실패: {url} — {e}");
                    }
                }
                self.results.clear();
                self.selected = 0;
            }
            None => {
                let items = transform::help_items(self.use_emoji);
                self.results = items_to_results(items);
                self.search_mode = kmd_core::SearchMode::Contains;
                self.selected = 0;
            }
        }
    }

    /// :prompt / :pt 쿼리 처리
    fn handle_prompt_query(&mut self, query: &str) {
        let sub = query
            .strip_prefix(":prompt")
            .or_else(|| query.strip_prefix(":pt"))
            .unwrap_or("")
            .trim();

        let config = crate::engine::load_config();
        let templates = &config.launcher.prompt_templates;

        // :prompt add <name> <body>
        if sub.starts_with("add ") {
            let rest = sub.strip_prefix("add ").unwrap_or("").trim();
            if let Some(pos) = rest.find(char::is_whitespace) {
                let name = &rest[..pos];
                let body = rest[pos..].trim();
                if !kmd_core::prompt::validate_template_name(name) {
                    self.results = items_to_results(std::iter::once(IndexItem {
                        name: format!("❌ 잘못된 이름: '{name}'"),
                        path: "영문/숫자/하이픈/언더스코어만, 최대 32자".to_string(),
                        kind: ItemKind::SystemCommand,
                        source: Source::Plugin,
                        icon: if self.use_emoji { "\u{274C}" } else { "[!]" }.to_string(),
                        keywords: "kmd:settings:noop".to_string(),
                    }));
                } else if body.is_empty() {
                    self.results = items_to_results(std::iter::once(IndexItem {
                        name: "❌ 본문이 비어 있습니다".to_string(),
                        path: ":prompt add <name> <body> 형태로 입력하세요".to_string(),
                        kind: ItemKind::SystemCommand,
                        source: Source::Plugin,
                        icon: if self.use_emoji { "\u{274C}" } else { "[!]" }.to_string(),
                        keywords: "kmd:settings:noop".to_string(),
                    }));
                } else {
                    let mut cfg = config;
                    cfg.launcher
                        .prompt_templates
                        .retain(|t| !t.name.eq_ignore_ascii_case(name));
                    cfg.launcher
                        .prompt_templates
                        .push(kmd_core::config::PromptTemplate {
                            name: name.to_string(),
                            body: body.to_string(),
                        });
                    save_config(move |c| {
                        c.launcher.prompt_templates = cfg.launcher.prompt_templates
                    });
                    self.results = items_to_results(std::iter::once(IndexItem {
                        name: format!("✅ 템플릿 '{name}' 저장됨"),
                        path: format!("@ll :{name} <query> 형태로 사용"),
                        kind: ItemKind::SystemCommand,
                        source: Source::Plugin,
                        icon: if self.use_emoji { "\u{2705}" } else { "[OK]" }.to_string(),
                        keywords: "kmd:settings:noop".to_string(),
                    }));
                }
            } else {
                self.results = items_to_results(std::iter::once(IndexItem {
                    name: "사용법: :prompt add <name> <body>".to_string(),
                    path: "예: :prompt add review 코드를 리뷰해주세요".to_string(),
                    kind: ItemKind::SystemCommand,
                    source: Source::Plugin,
                    icon: if self.use_emoji {
                        "\u{2139}\u{FE0F}"
                    } else {
                        "[?]"
                    }
                    .to_string(),
                    keywords: "kmd:settings:noop".to_string(),
                }));
            }
            self.selected = 0;
            return;
        }

        // :prompt remove/rm/del <name>
        if sub.starts_with("remove ") || sub.starts_with("rm ") || sub.starts_with("del ") {
            let name = sub
                .strip_prefix("remove ")
                .or_else(|| sub.strip_prefix("rm "))
                .or_else(|| sub.strip_prefix("del "))
                .unwrap_or("")
                .trim();
            if name.is_empty() {
                self.results = items_to_results(std::iter::once(IndexItem {
                    name: "사용법: :prompt remove <name>".to_string(),
                    path: "삭제할 템플릿 이름을 입력하세요".to_string(),
                    kind: ItemKind::SystemCommand,
                    source: Source::Plugin,
                    icon: if self.use_emoji {
                        "\u{2139}\u{FE0F}"
                    } else {
                        "[?]"
                    }
                    .to_string(),
                    keywords: "kmd:settings:noop".to_string(),
                }));
            } else if templates.iter().any(|t| t.name.eq_ignore_ascii_case(name)) {
                let name_owned = name.to_string();
                let display_name = name.to_string();
                save_config(move |cfg| {
                    cfg.launcher
                        .prompt_templates
                        .retain(|t| !t.name.eq_ignore_ascii_case(&name_owned));
                });
                self.results = items_to_results(std::iter::once(IndexItem {
                    name: format!("✅ 템플릿 '{display_name}' 삭제됨"),
                    path: String::new(),
                    kind: ItemKind::SystemCommand,
                    source: Source::Plugin,
                    icon: if self.use_emoji { "\u{2705}" } else { "[OK]" }.to_string(),
                    keywords: "kmd:settings:noop".to_string(),
                }));
            } else {
                self.results = items_to_results(std::iter::once(IndexItem {
                    name: format!("❌ 템플릿 '{name}'을 찾을 수 없습니다"),
                    path: String::new(),
                    kind: ItemKind::SystemCommand,
                    source: Source::Plugin,
                    icon: if self.use_emoji { "\u{274C}" } else { "[!]" }.to_string(),
                    keywords: "kmd:settings:noop".to_string(),
                }));
            }
            self.selected = 0;
            return;
        }

        let filter = sub.strip_prefix("list").unwrap_or(sub).trim();
        let items = kmd_core::prompt::list_templates_as_items(templates, filter, self.use_emoji);
        self.results = items_to_results(items);
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_calc_query(&mut self, query: &str) {
        let expr = query.strip_prefix(":calc").unwrap_or("").trim();
        let calc = builtin_calc::CalcExtension;
        self.apply_contains_items(calc.search_with_emoji(expr, self.use_emoji));
    }

    fn handle_emoji_query(&mut self, query: &str) {
        let search_query = query
            .strip_prefix(":emoji")
            .or_else(|| query.strip_prefix(":e"))
            .unwrap_or("")
            .trim();
        let emoji_ext = builtin_emoji::EmojiExtension;
        self.apply_contains_items(emoji_ext.search_emoji(search_query));
    }

    fn handle_shell_query(&mut self, query: &str) {
        let shell_query = query.strip_prefix('!').unwrap_or("").trim();
        let shell_ext = builtin_shell::ShellExtension;
        self.apply_contains_items(shell_ext.search(shell_query));
    }

    /// :keymap / :km 쿼리 처리
    fn handle_keymap_query(&mut self, query: &str) {
        let sub = query
            .strip_prefix(":keymap")
            .or_else(|| query.strip_prefix(":km"))
            .unwrap_or("")
            .trim();
        let config = crate::engine::load_config();
        let items = kmd_core::keymap::keymap_items(&config, sub, self.use_emoji);
        self.apply_contains_items(items);
    }

    fn handle_keymap_action(&mut self, result: &kmd_core::SearchResult) -> Task<Message> {
        let keywords = &result.item.keywords;
        if keywords.ends_with(":noop") || keywords.contains(":noop:") {
            return Task::none();
        }
        let mut config = crate::engine::load_config();
        if let Some(msg) = kmd_core::keymap::execute_keymap_action(&mut config, keywords) {
            tracing::info!("keymap action: {msg}");
        }
        let current_query = self.query.clone();
        self.handle_keymap_query(&current_query);
        Task::none()
    }

    fn handle_settings_query(&mut self, query: &str) {
        let filter = match query.find(' ') {
            Some(pos) => query[pos + 1..].trim().to_lowercase(),
            None => String::new(),
        };

        let emoji = self.use_emoji;
        let current_theme = self.theme.name;

        let ime_label = if self.reset_ime_on_launch {
            "IME: Reset to English on Launch [ON]"
        } else {
            "IME: Reset to English on Launch [OFF]"
        };

        let label = |base: &str, theme_name: &str| -> String {
            if current_theme.eq_ignore_ascii_case(theme_name) {
                format!("{base} [Current]")
            } else {
                base.to_string()
            }
        };

        let mut settings_entries: Vec<(String, String, String, String)> = vec![
            (
                "Edit Config File".to_string(),
                "kmd:settings:config".to_string(),
                if emoji { "\u{2699}\u{FE0F}" } else { "[CFG]" }.to_string(),
                "Open config.toml".to_string(),
            ),
            (
                "Open Config Directory".to_string(),
                "kmd:settings:dir".to_string(),
                if emoji { "\u{1F4C2}" } else { "[DIR]" }.to_string(),
                "Open configuration folder".to_string(),
            ),
            (
                format!(
                    "Version: desktop {} / core {}",
                    env!("CARGO_PKG_VERSION"),
                    kmd_core::Index::current_version()
                ),
                "kmd:settings:noop".to_string(),
                if emoji { "\u{1F4E6}" } else { "[VER]" }.to_string(),
                "Use :version or kmd-desktop --version".to_string(),
            ),
            (
                "Reset Window Position".to_string(),
                "kmd:settings:reset_position".to_string(),
                if emoji { "\u{1F4CD}" } else { "[POS]" }.to_string(),
                "Move window to default position".to_string(),
            ),
            (
                ime_label.to_string(),
                "kmd:settings:toggle_ime_reset".to_string(),
                if emoji { "\u{1F310}" } else { "[IME]" }.to_string(),
                "Toggle English input on launch".to_string(),
            ),
            (
                label("Theme: Midnight (default)", "Midnight"),
                "kmd:settings:theme:midnight".to_string(),
                if emoji { "\u{1F319}" } else { "[THM]" }.to_string(),
                "Switch desktop theme".to_string(),
            ),
            (
                label("Theme: Obsidian", "Obsidian"),
                "kmd:settings:theme:obsidian".to_string(),
                if emoji { "\u{2B1B}" } else { "[THM]" }.to_string(),
                "Switch desktop theme".to_string(),
            ),
            (
                label("Theme: Snow", "Snow"),
                "kmd:settings:theme:snow".to_string(),
                if emoji { "\u{2600}\u{FE0F}" } else { "[THM]" }.to_string(),
                "Switch desktop theme".to_string(),
            ),
            (
                label("Theme: Rose Pine", "Rose Pine"),
                "kmd:settings:theme:rose_pine".to_string(),
                if emoji { "\u{1F339}" } else { "[THM]" }.to_string(),
                "Switch desktop theme".to_string(),
            ),
            (
                label("Theme: Nord", "Nord"),
                "kmd:settings:theme:nord".to_string(),
                if emoji { "\u{2744}\u{FE0F}" } else { "[THM]" }.to_string(),
                "Switch desktop theme".to_string(),
            ),
        ];

        let llm_rows = [
            ("chatgpt", "ChatGPT"),
            ("gemini", "Gemini"),
            ("claude", "Claude"),
            ("grok", "Grok"),
            ("perplexity", "Perplexity"),
        ];
        for (id, provider_name) in llm_rows {
            let enabled = self
                .selected_llm_providers
                .iter()
                .any(|v| v.eq_ignore_ascii_case(id));
            settings_entries.push((
                format!(
                    "Multi LLM: {} [{}]",
                    provider_name,
                    if enabled { "ON" } else { "OFF" }
                ),
                format!("kmd:settings:llm:toggle:{id}"),
                if emoji { "\u{1F9E0}" } else { "[LLM]" }.to_string(),
                "Toggle provider for @llm compare".to_string(),
            ));
        }

        let multi_web_rows = [
            ("google", "Google"),
            ("naver_search", "Naver"),
            ("daum", "Daum"),
        ];
        for (id, provider_name) in multi_web_rows {
            let enabled = self
                .selected_multi_web_providers
                .iter()
                .any(|v| v.eq_ignore_ascii_case(id));
            settings_entries.push((
                format!(
                    "Multi Web: {} [{}]",
                    provider_name,
                    if enabled { "ON" } else { "OFF" }
                ),
                format!("kmd:settings:mweb:toggle:{id}"),
                if emoji { "\u{1F50E}" } else { "[WEB]" }.to_string(),
                "Toggle engine for @msearch multi search".to_string(),
            ));
        }

        let spell_rows = [
            ("naver_spell", "Naver Spell"),
            ("pusan_spell", "Pusan Spell"),
        ];
        for (id, provider_name) in spell_rows {
            let enabled = self
                .spell_providers
                .iter()
                .any(|v| v.eq_ignore_ascii_case(id));
            settings_entries.push((
                format!(
                    "Spell: {} [{}]",
                    provider_name,
                    if enabled { "ON" } else { "OFF" }
                ),
                format!("kmd:settings:spell:toggle:{id}"),
                if emoji { "\u{270D}\u{FE0F}" } else { "[SPL]" }.to_string(),
                "Toggle provider for @sp spelling check".to_string(),
            ));
        }

        let translate_rows = [
            ("google_translate", "Google Translate"),
            ("papago", "Papago"),
            ("deepl", "DeepL"),
        ];
        for (id, provider_name) in translate_rows {
            let enabled = self
                .translate_providers
                .iter()
                .any(|v| v.eq_ignore_ascii_case(id));
            settings_entries.push((
                format!(
                    "Translate: {} [{}]",
                    provider_name,
                    if enabled { "ON" } else { "OFF" }
                ),
                format!("kmd:settings:translate:toggle:{id}"),
                if emoji { "\u{1F5E3}\u{FE0F}" } else { "[TR]" }.to_string(),
                "Toggle provider for @tr translation".to_string(),
            ));
        }

        settings_entries.extend_from_slice(&[
            (
                "Rebuild Index".to_string(),
                "kmd:settings:rebuild".to_string(),
                if emoji { "\u{1F504}" } else { "[IDX]" }.to_string(),
                "Rebuild and reload index data".to_string(),
            ),
            // Non-actionable info entries are intentionally at the bottom.
            (
                "Info: Move Window (drag top strip)".to_string(),
                "kmd:settings:noop".to_string(),
                if emoji { "\u{2139}\u{FE0F}" } else { "[TIP]" }.to_string(),
                "Info only - not executable".to_string(),
            ),
            (
                "Info: Resize Window (drag left/right edges)".to_string(),
                "kmd:settings:noop".to_string(),
                if emoji { "\u{2139}\u{FE0F}" } else { "[TIP]" }.to_string(),
                "Info only - not executable".to_string(),
            ),
        ]);

        let items: Vec<IndexItem> = settings_entries
            .iter()
            .filter(|(name, _, _, _)| filter.is_empty() || name.to_lowercase().contains(&filter))
            .map(|(name, action, icon, desc)| IndexItem {
                name: name.clone(),
                path: desc.to_string(),
                icon: icon.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: action.clone(),
            })
            .collect();

        self.results = items_to_results(items);
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_help_query(&mut self) {
        let emoji = self.use_emoji;
        let entries: &[(&str, &str, &str)] = &[
            (
                "@  Web Search",
                "Type @prefix query  (e.g. @g rust, @ai why is the sky blue)",
                if emoji { "\u{1F310}" } else { "[WEB]" },
            ),
            (
                ":calc  Calculator",
                "Type :calc expression  (e.g. :calc (2+3)*4)",
                if emoji { "\u{1F522}" } else { "[CAL]" },
            ),
            (
                ":emoji  Emoji Search",
                "Type :emoji keyword  or  :e keyword  (e.g. :e fire)",
                if emoji { "\u{1F60A}" } else { "[EMO]" },
            ),
            (
                ":set  Settings",
                "Type :set or :settings to manage config, themes, index",
                if emoji { "\u{2699}\u{FE0F}" } else { "[SET]" },
            ),
            (
                ":t  Quick Transform",
                "Type :t spell/tr/trko/tren  (clipboard text → spell/translate)",
                if emoji { "\u{26A1}" } else { "[QT]" },
            ),
            (
                ":prompt  Prompt Templates",
                "Type :prompt  (manage reusable prompt templates for @ll)",
                if emoji { "\u{1F4DD}" } else { "[PT]" },
            ),
            (
                ":keymap  Keymap Control",
                "Type :keymap or :km  (kanata status, on/off, profile switch)",
                if emoji { "\u{2328}\u{FE0F}" } else { "[KEY]" },
            ),
            (
                ":version  Version Info",
                "Type :version  (show desktop/core/target/os versions)",
                if emoji { "\u{1F4E6}" } else { "[VER]" },
            ),
            (
                "@llm  Multi LLM Compare",
                "Type @ll prompt  (alias: @llm, open selected LLM providers)",
                if emoji { "\u{1F9E0}" } else { "[LLM]" },
            ),
            (
                "@msearch  Multi Web Search",
                "Type @m query  (alias: @msearch, open selected web engines)",
                if emoji { "\u{1F50E}" } else { "[MWEB]" },
            ),
            (
                "@sp  Spell Check",
                "Type @sp text  (Korean spelling check on selected providers)",
                if emoji { "\u{270D}\u{FE0F}" } else { "[SPL]" },
            ),
            (
                "@tr  Translate",
                "Type @tr/@trko/@tren text  (auto / en->ko / ko->en)",
                if emoji { "\u{1F5E3}\u{FE0F}" } else { "[TR]" },
            ),
            (
                "Version Info",
                "CLI also supports: kmd-desktop --version",
                if emoji { "\u{2139}\u{FE0F}" } else { "[VER]" },
            ),
            (
                "!  Shell Command",
                "Type !command  (e.g. !ip, !hostname, !echo hello)",
                if emoji { "\u{1F4BB}" } else { "[SHL]" },
            ),
            (
                "Fuzzy Search",
                "Just type to search files, apps, folders  (e.g. firefox)",
                if emoji { "\u{1F50D}" } else { "[FZF]" },
            ),
            (
                "*.ext  Glob Pattern",
                "Use * or ? for glob matching  (e.g. *.pdf, test?.rs)",
                if emoji { "\u{1F4C4}" } else { "[GLB]" },
            ),
            (
                "/regex/  Regular Expression",
                "Wrap in /slashes/ for regex  (e.g. /test\\d+/)",
                if emoji { "\u{1F9EA}" } else { "[RGX]" },
            ),
        ];

        let items: Vec<IndexItem> = entries
            .iter()
            .map(|(name, desc, icon)| IndexItem {
                name: name.to_string(),
                path: desc.to_string(),
                icon: icon.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: if name.starts_with("Fuzzy Search")
                    || name.starts_with("*.ext")
                    || name.starts_with("/regex/")
                {
                    "kmd:help:example".to_string()
                } else {
                    String::new()
                },
            })
            .collect();

        self.results = items_to_results(items);
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_settings_action(&mut self, result: &kmd_core::SearchResult) -> Task<Message> {
        let action_src = if result.item.keywords.starts_with("kmd:settings:") {
            result.item.keywords.as_str()
        } else {
            result.item.path.as_str()
        };
        let action = action_src.strip_prefix("kmd:settings:").unwrap_or("");

        match action {
            "noop" => {
                return Task::none();
            }
            "config" => {
                let config_dir = kmd_core::Config::default_config_dir();
                let config_path = config_dir.join(kmd_core::CONFIG_FILENAME);
                if !config_path.exists() {
                    let mut cfg = crate::engine::load_config();
                    cfg.config_path = Some(config_path.clone());
                    if let Err(e) = cfg.save() {
                        tracing::warn!("Failed to create config file: {e}");
                    }
                }
                if let Err(e) = open::that(&config_path) {
                    tracing::warn!("설정 파일 열기 실패: {e}");
                }
            }
            "dir" => {
                let config_dir = kmd_core::Config::default_config_dir();
                if let Err(e) = open::that(&config_dir) {
                    tracing::warn!("설정 디렉토리 열기 실패: {e}");
                }
            }
            "reset_position" => {
                WindowState::reset();
                self.window_state = WindowState::default();
                self.window_width = DEFAULT_WIDTH;
                self.state_dirty = false;
                let run_reset = |id: window::Id| {
                    let resize = window::resize(id, Size::new(DEFAULT_WIDTH, SEARCH_BAR_HEIGHT));
                    let move_task = window::monitor_size(id).then(move |maybe_size| {
                        if let Some(mon) = maybe_size {
                            let x = (mon.width - DEFAULT_WIDTH) / 2.0;
                            let y = (mon.height / 3.0).max(0.0);
                            window::move_to(id, Point::new(x, y))
                        } else {
                            Task::none()
                        }
                    });
                    Task::batch([resize, move_task])
                };

                self.query.clear();
                self.results.clear();
                self.selected = 0;

                return match self.window_id {
                    Some(id) => run_reset(id),
                    None => window::oldest().then(move |maybe_id| match maybe_id {
                        Some(id) => run_reset(id),
                        None => Task::none(),
                    }),
                };
            }
            "rebuild" => {
                self.full_warmup_started = true;
                self.loading = true;
                let task = self.spawn_full_engine_load_task();
                self.query.clear();
                self.results.clear();
                self.selected = 0;
                return task;
            }
            "toggle_ime_reset" => {
                self.reset_ime_on_launch = !self.reset_ime_on_launch;
                let new_val = self.reset_ime_on_launch;
                tracing::info!("reset_ime_on_launch = {new_val}");
                save_config(|cfg| cfg.general.reset_ime_on_launch = new_val);

                // Re-open settings so the user sees the updated [ON/OFF] label.
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return Task::none();
            }
            llm_toggle if llm_toggle.starts_with("llm:toggle:") => {
                let target = llm_toggle.strip_prefix("llm:toggle:").unwrap_or("");
                if !target.is_empty() {
                    if self
                        .selected_llm_providers
                        .iter()
                        .any(|v| v.eq_ignore_ascii_case(target))
                    {
                        self.selected_llm_providers
                            .retain(|v| !v.eq_ignore_ascii_case(target));
                    } else {
                        self.selected_llm_providers.push(target.to_string());
                    }

                    // Keep @llm usable even when users turn everything off.
                    if self.selected_llm_providers.is_empty() {
                        self.selected_llm_providers = vec![
                            "chatgpt".to_string(),
                            "gemini".to_string(),
                            "claude".to_string(),
                            "grok".to_string(),
                            "perplexity".to_string(),
                        ];
                    }

                    let selected = self.selected_llm_providers.clone();
                    save_config(move |cfg| cfg.launcher.multi_llm_providers = selected);
                }

                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return Task::none();
            }
            mweb_toggle if mweb_toggle.starts_with("mweb:toggle:") => {
                let target = mweb_toggle.strip_prefix("mweb:toggle:").unwrap_or("");
                if !target.is_empty() {
                    if self
                        .selected_multi_web_providers
                        .iter()
                        .any(|v| v.eq_ignore_ascii_case(target))
                    {
                        self.selected_multi_web_providers
                            .retain(|v| !v.eq_ignore_ascii_case(target));
                    } else {
                        self.selected_multi_web_providers.push(target.to_string());
                    }

                    if self.selected_multi_web_providers.is_empty() {
                        self.selected_multi_web_providers = vec![
                            "google".to_string(),
                            "naver_search".to_string(),
                            "daum".to_string(),
                        ];
                    }

                    let selected = self.selected_multi_web_providers.clone();
                    save_config(move |cfg| cfg.launcher.multi_web_providers = selected);
                }

                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return Task::none();
            }
            spell_toggle if spell_toggle.starts_with("spell:toggle:") => {
                let target = spell_toggle.strip_prefix("spell:toggle:").unwrap_or("");
                if !target.is_empty() {
                    if self
                        .spell_providers
                        .iter()
                        .any(|v| v.eq_ignore_ascii_case(target))
                    {
                        self.spell_providers
                            .retain(|v| !v.eq_ignore_ascii_case(target));
                    } else {
                        self.spell_providers.push(target.to_string());
                    }
                    if self.spell_providers.is_empty() {
                        self.spell_providers =
                            vec!["naver_spell".to_string(), "pusan_spell".to_string()];
                    }
                    let selected = self.spell_providers.clone();
                    save_config(move |cfg| cfg.launcher.spell_providers = selected);
                }
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return Task::none();
            }
            translate_toggle if translate_toggle.starts_with("translate:toggle:") => {
                let target = translate_toggle
                    .strip_prefix("translate:toggle:")
                    .unwrap_or("");
                if !target.is_empty() {
                    if self
                        .translate_providers
                        .iter()
                        .any(|v| v.eq_ignore_ascii_case(target))
                    {
                        self.translate_providers
                            .retain(|v| !v.eq_ignore_ascii_case(target));
                    } else {
                        self.translate_providers.push(target.to_string());
                    }
                    if self.translate_providers.is_empty() {
                        self.translate_providers = vec![
                            "google_translate".to_string(),
                            "papago".to_string(),
                            "deepl".to_string(),
                        ];
                    }
                    let selected = self.translate_providers.clone();
                    save_config(move |cfg| cfg.launcher.translate_providers = selected);
                }
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return Task::none();
            }
            theme_action if theme_action.starts_with("theme:") => {
                let theme_name = theme_action.strip_prefix("theme:").unwrap_or("midnight");
                self.theme = crate::theme::from_name(theme_name);
                tracing::info!("Theme changed to: {}", self.theme.name);

                // Persist theme selection to config file.
                let name_owned = theme_name.to_string();
                save_config(|cfg| cfg.general.theme = name_owned);
            }
            _ => {
                tracing::warn!("Unknown settings action: {action}");
            }
        }
        self.query.clear();
        self.results.clear();
        self.selected = 0;
        Task::none()
    }

    fn handle_main_search(&mut self, query: &str) {
        let (mode, mut results) = self.engine.search(query, SEARCH_LIMIT);
        self.search_mode = mode;

        if builtin_calc::looks_like_math(query) {
            let calc = builtin_calc::CalcExtension;
            let calc_items = calc.search_with_emoji(query, self.use_emoji);
            let calc_results: Vec<kmd_core::SearchResult> = calc_items
                .into_iter()
                .map(|item| kmd_core::SearchResult {
                    item,
                    score: SCORE_PLUGIN,
                })
                .collect();
            results.splice(0..0, calc_results);
        }

        if results.is_empty() && !query.is_empty() {
            if let Some(relaxed) = relaxed_hangul_query(query) {
                let relaxed_results = self.engine.search_with_mode(
                    kmd_core::SearchMode::Contains,
                    &relaxed,
                    SEARCH_LIMIT,
                );
                if !relaxed_results.is_empty() {
                    results = relaxed_results;
                    self.search_mode = kmd_core::SearchMode::Contains;
                }
            }
        }

        if results.is_empty() && !query.is_empty() {
            results = self.build_fallback_suggestions(query);
            self.search_mode = kmd_core::SearchMode::Contains;
        }

        self.results = results;
        self.selected = 0;
    }

    fn build_fallback_suggestions(&self, query: &str) -> Vec<kmd_core::SearchResult> {
        use kmd_core::web::{self, WEB_SERVICES};

        const FALLBACK_IDS: &[&str] = &[
            "google",
            "perplexity",
            "chatgpt",
            "claude",
            "gemini",
            "naver_search",
            "youtube",
            "github",
            "stackoverflow",
            "wikipedia",
        ];

        let emoji = self.use_emoji;
        let mut items: Vec<kmd_core::SearchResult> = FALLBACK_IDS
            .iter()
            .filter_map(|id| WEB_SERVICES.iter().find(|s| s.id == *id))
            .map(|service| {
                let item = web::search_result_item(service, query, emoji);
                kmd_core::SearchResult { item, score: 0 }
            })
            .collect();

        let multi_items = web::multi_llm_result_items(query, &self.selected_llm_providers, emoji);
        if let Some(multi_item) = multi_items.into_iter().next() {
            items.insert(
                5,
                kmd_core::SearchResult {
                    item: multi_item,
                    score: 0,
                },
            );
        }

        items
    }

    fn launch_selected(&mut self) -> Task<Message> {
        let Some(result) = self.results.get(self.selected).cloned() else {
            return Task::none();
        };

        // Help entries now act like quick templates.
        if self.query.starts_with(":help") && result.item.path.starts_with("Type ") {
            if let Some(seed) = help_query_seed(&result.item.name) {
                self.query = seed.to_string();
                self.selected = 0;
                return self.perform_search();
            }
            return Task::none();
        }

        if result.item.kind == ItemKind::SystemCommand
            && result.item.keywords.starts_with("kmd:settings:")
        {
            return self.handle_settings_action(&result);
        }

        if result.item.kind == ItemKind::SystemCommand
            && result.item.keywords.starts_with("kmd:keymap:")
        {
            return self.handle_keymap_action(&result);
        }

        if self.state_dirty {
            self.window_state.save();
        }

        // Shell 명령 처리
        if result.item.kind == ItemKind::Shell {
            if builtin_shell::ShellExtension::is_quick_action(&result.item.path) {
                // Quick Action → 백그라운드 실행, 결과를 클립보드에 복사
                let item = result.item.clone();
                return Task::future(async move {
                    let res = tokio::task::spawn_blocking(move || {
                        use kmd_core::plugin::Extension;
                        let shell_ext = builtin_shell::ShellExtension;
                        match shell_ext.execute(&item) {
                            kmd_core::plugin::ExtensionAction::CopyToClipboard(o) => Ok(o),
                            kmd_core::plugin::ExtensionAction::Display(msg) => Err(msg),
                            _ => Err("Unknown action".to_string()),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Task panicked: {e}")));
                    Message::ShellDone(res)
                });
            }
            // 사용자 명령 → 새 터미널 창에서 실행 (cmd /k 로 결과 유지)
            launch_in_terminal(&result.item.path);
            return iced::exit();
        }

        // 웹 검색 결과 — extract_batch_urls 통합 추출
        if result.item.kind == ItemKind::WebSearch {
            if let Some(urls) = web::extract_batch_urls(&result.item) {
                // LLM 멀티 프롬프트인 경우 클립보드에 프롬프트 복사
                if web::extract_multi_llm_urls(&result.item).is_some() {
                    self.copy_multi_llm_prompt_to_clipboard();
                }
                for url in urls {
                    if let kmd_core::action::ActionResult::Error(e) =
                        kmd_core::action::open_url(&url)
                    {
                        tracing::warn!("URL 열기 실패: {url} — {e}");
                    }
                }
                return iced::exit();
            }
        }

        let action_result = kmd_core::action::execute(&result);
        match action_result {
            kmd_core::action::ActionResult::Launched => {
                tracing::debug!("Launched: {}", result.item.name);
            }
            kmd_core::action::ActionResult::OpenedUrl(url) => {
                tracing::debug!("Opened URL: {url}");
            }
            kmd_core::action::ActionResult::NeedsConfirmation(msg) => {
                tracing::warn!("Needs confirmation: {msg}");
                return Task::none();
            }
            kmd_core::action::ActionResult::Error(err) => {
                tracing::error!("Failed to launch '{}': {err}", result.item.name);
                return Task::none();
            }
        }
        iced::exit()
    }

    /// Ctrl+숫자/Ctrl+Shift+Enter 단축키 → ContextAction 매핑
    fn match_shortcut(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<ContextAction> {
        if self.results.is_empty() {
            return None;
        }
        let item_kind = self.results.get(self.selected)?.item.kind;
        let available = context_actions_for(item_kind);

        if modifiers.control() && modifiers.shift() {
            if matches!(key, keyboard::Key::Named(keyboard::key::Named::Enter)) {
                if available
                    .iter()
                    .any(|a| matches!(a, ContextAction::OpenAsAdmin))
                {
                    return Some(ContextAction::OpenAsAdmin);
                }
            }
        }

        if modifiers.control() && !modifiers.shift() {
            let idx = match key {
                keyboard::Key::Character(c) => match c.as_str() {
                    "1" => Some(0usize),
                    "2" => Some(1),
                    "3" => Some(2),
                    "4" => Some(3),
                    _ => None,
                },
                _ => None,
            };
            if let Some(i) = idx {
                return available.into_iter().nth(i);
            }
        }

        None
    }

    fn handle_key(&mut self, key: keyboard::Key) -> Task<Message> {
        match key {
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                self.scroll_to_selected()
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                let max = self.results.len().saturating_sub(1);
                if self.selected < max {
                    self.selected += 1;
                }
                self.scroll_to_selected()
            }
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                if !self.query.is_empty() {
                    return self.clear_query_and_refocus();
                }
                if self.state_dirty {
                    self.window_state.save();
                }
                iced::exit()
            }
            _ => Task::none(),
        }
    }

    fn scroll_to_selected(&self) -> Task<Message> {
        let top_row = self.selected.saturating_sub(MAX_VISIBLE_ROWS - 1);
        let y_offset = top_row as f32 * ROW_HEIGHT;
        scroll_to(
            self.scrollable_id.clone(),
            scrollable_mod::AbsoluteOffset {
                x: 0.0,
                y: y_offset,
            },
        )
    }

    fn clear_query_and_refocus(&mut self) -> Task<Message> {
        self.query.clear();
        self.results.clear();
        self.selected = 0;
        iced::widget::operation::focus::<Message>(self.input_id.clone())
    }
}

// ─── Prefix Detection ─────────────────────────────────────────────────────────

enum Prefix {
    Web,
    Transform,
    Prompt,
    Calc,
    Emoji,
    Settings,
    Help,
    Version,
    Shell,
    Keymap,
    General,
}

fn prefix_of(query: &str) -> Prefix {
    if query.starts_with('@') {
        Prefix::Web
    } else if query.starts_with(":transform") || query.starts_with(":t ") || query == ":t" {
        Prefix::Transform
    } else if query.starts_with(":prompt") || query.starts_with(":pt") {
        Prefix::Prompt
    } else if query.starts_with(":calc") {
        Prefix::Calc
    } else if query.starts_with(":emoji") || query.starts_with(":e ") || query == ":e" {
        Prefix::Emoji
    } else if query.starts_with(":set") {
        Prefix::Settings
    } else if query.starts_with(":help") || query.starts_with(":h ") || query == ":h" {
        Prefix::Help
    } else if query.starts_with(":version")
        || query.starts_with(":ver")
        || query == ":v"
        || query.starts_with(":v ")
    {
        Prefix::Version
    } else if query.starts_with(":keymap") || query.starts_with(":km ") || query == ":km" {
        Prefix::Keymap
    } else if query.starts_with('!') {
        Prefix::Shell
    } else {
        Prefix::General
    }
}

// ─── View ─────────────────────────────────────────────────────────────────────

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let has_results = !self.results.is_empty();
        let text_color = t.text;
        let overlay_color = t.overlay;
        let accent_color = t.accent;
        let bg = t.background_with_opacity();
        let shadow_i = t.shadow_intensity;
        let peach = t.peach;
        let divider_c = t.border;

        // ── 드래그 스트립 (투명) ──
        let drag_strip = mouse_area(container(text("")).width(Fill).height(DRAG_STRIP))
            .on_press(Message::StartWindowDrag)
            .interaction(iced::mouse::Interaction::Grab);

        // ── 검색바 콘텐츠 ──
        let brand = mouse_area(container(text("\u{00BB}").size(24).color(peach)).padding(
            Padding {
                top: 0.0,
                right: 4.0,
                bottom: 0.0,
                left: 6.0,
            },
        ))
        .on_press(Message::BrandClicked)
        .on_right_press(Message::BrandRightClicked)
        .interaction(iced::mouse::Interaction::Pointer);

        let input = text_input("Search anything...", &self.query)
            .id(self.input_id.clone())
            .on_input(Message::QueryChanged)
            .on_submit(Message::Submit)
            .width(Fill)
            .size(20)
            .padding(Padding::from([2, 6]))
            .style(move |_theme, status| {
                let is_focused = matches!(status, text_input::Status::Focused { .. });
                let ph_color = if is_focused {
                    Color {
                        a: overlay_color.a * 0.5,
                        ..overlay_color
                    }
                } else {
                    overlay_color
                };
                text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border::default(),
                    icon: overlay_color,
                    placeholder: ph_color,
                    value: text_color,
                    selection: Color {
                        a: 0.3,
                        ..accent_color
                    },
                }
            });

        let search_row = row![brand, input]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([0, 12]));

        let search_bar = container(search_row).width(Fill).center_y(PILL_HEIGHT);

        // ── 카드 본체 (항상 동일한 트리 구조 — 포커스/IME 안정성 보장) ──
        let sep_h = if has_results { 1.0 } else { 0.0 };
        let h_sep = container(text(""))
            .width(Fill)
            .height(sep_h)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(divider_c)),
                ..Default::default()
            });

        let h_sep2 = container(text(""))
            .width(Fill)
            .height(sep_h)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(divider_c)),
                ..Default::default()
            });

        let vert_divider = container(text(""))
            .width(if has_results { 1 } else { 0 })
            .height(Fill)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(divider_c)),
                ..Default::default()
            });

        let left_col = Column::new()
            .push(self.view_results_list())
            .push(h_sep2)
            .push(self.view_status_bar());

        let results_body = row![
            container(left_col).width(Length::FillPortion(2)),
            vert_divider,
            container(self.view_detail_panel()).width(Length::FillPortion(1)),
        ]
        .width(Fill)
        .height(if has_results {
            Length::Fill
        } else {
            Length::Fixed(0.0)
        });

        let card_col = Column::new()
            .push(search_bar)
            .push(h_sep)
            .push(results_body);

        // ── 카드 스타일 (pill ↔ 라운드 사각 자연 전환) ──
        let border_color = Color {
            a: 0.30,
            ..accent_color
        };
        let radius: f32 = if has_results {
            t.corner_radius
        } else {
            PILL_HEIGHT / 2.0
        };

        let card = container(card_col)
            .width(Fill)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: radius.into(),
                    width: 2.0,
                    color: border_color,
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.15 * shadow_i),
                    offset: Vector::new(0.0, 2.0),
                    blur_radius: 8.0,
                },
                text_color: None,
                snap: false,
            });

        // ── 안정적 트리: 항상 동일한 4개 자식 (drag, card_row, hint, bg_dismiss) ──
        let edge_w: f32 = if has_results { 4.0 } else { 0.0 };
        let left_edge = mouse_area(container(text("")).width(edge_w).height(Fill))
            .on_press(Message::StartWindowResize(window::Direction::West))
            .interaction(iced::mouse::Interaction::ResizingHorizontally);
        let right_edge = mouse_area(container(text("")).width(edge_w).height(Fill))
            .on_press(Message::StartWindowResize(window::Direction::East))
            .interaction(iced::mouse::Interaction::ResizingHorizontally);

        let card_row = row![left_edge, card, right_edge]
            .width(Fill)
            .height(if has_results {
                Length::Fill
            } else {
                Length::Shrink
            });

        let hint_area: Element<'_, Message> = if !has_results && !self.query.trim().is_empty() {
            container(text("No results found").size(13).color(t.overlay).center())
                .width(Fill)
                .padding(Padding::from([8, 0]))
                .center_x(Fill)
                .into()
        } else {
            container(text("")).width(0).height(0).into()
        };

        let bg_dismiss = mouse_area(container(text("")).width(Fill).height(Fill))
            .on_press(Message::BackgroundClicked);

        let content = Column::new()
            .push(drag_strip)
            .push(card_row)
            .push(hint_area)
            .push(bg_dismiss);

        container(content).width(Fill).height(Fill).into()
    }

    fn view_results_list(&self) -> Element<'_, Message> {
        let mut list = Column::new().spacing(0);
        for (i, result) in self.results.iter().enumerate() {
            list = list.push(self.view_result_row(i, result));
        }

        scrollable(container(list).width(Fill))
            .id(self.scrollable_id.clone())
            .height(Fill)
            .into()
    }

    fn view_result_row<'a>(
        &'a self,
        index: usize,
        result: &'a kmd_core::SearchResult,
    ) -> Element<'a, Message> {
        let t = &self.theme;
        let is_selected = index == self.selected;
        let item = &result.item;

        let sel_color = if is_selected {
            t.accent
        } else {
            Color::TRANSPARENT
        };
        let left_bar = container(text(""))
            .width(3)
            .height(ROW_HEIGHT - 8.0)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(sel_color)),
                border: Border {
                    radius: 1.5.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        let icon_size: f32 = 32.0;
        let icon_element: Element<'_, Message> = if let Some(handle) =
            crate::brand_icons::brand_icon_for_item(item.kind, &item.keywords, &item.path)
                .or_else(|| crate::brand_icons::brand_icon_for_settings(&item.keywords))
                .or_else(|| crate::app_icons::app_icon_for_item(item.kind, &item.path))
        {
            image(handle)
                .content_fit(iced::ContentFit::Fill)
                .width(icon_size)
                .height(icon_size)
                .into()
        } else {
            container(text(&item.icon).size(icon_size - 6.0))
                .center_x(icon_size)
                .center_y(icon_size)
                .into()
        };
        let title = text(&item.name)
            .size(16)
            .color(t.text)
            .wrapping(Wrapping::None);
        let subtitle = text(&item.path)
            .size(13)
            .color(t.subtext)
            .wrapping(Wrapping::None);
        let info = column![title, subtitle].spacing(2).width(Fill).clip(true);

        let kind_color = t.kind_color(item.kind);
        let kind_label = item.kind.to_string();
        let badge_bg = Color {
            a: 0.10,
            ..kind_color
        };
        let badge_border = Color {
            a: 0.20,
            ..kind_color
        };
        let badge = container(text(kind_label).size(10).color(kind_color))
            .padding(Padding::from([2, 6]))
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(badge_bg)),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: badge_border,
                },
                ..Default::default()
            });

        let bg = if is_selected {
            t.surface2
        } else {
            Color::TRANSPARENT
        };

        let row_content = row![left_bar, icon_element, info, badge]
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([4, 12]));

        mouse_area(
            container(row_content)
                .width(Fill)
                .height(ROW_HEIGHT)
                .center_y(Fill)
                .style(move |_: &_| container::Style {
                    background: Some(Background::Color(bg)),
                    ..Default::default()
                }),
        )
        .on_press(Message::ResultClicked(index))
        .into()
    }

    fn view_status_bar(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let status_text = format!(
            "{}  \u{00B7}  {} results",
            self.search_mode.label(),
            self.results.len()
        );

        let left = text(status_text).size(11).color(t.overlay);
        let right = text("Esc to close").size(11).color(t.overlay);

        let bar = row![left, Space::new().width(Fill), right]
            .padding(Padding::from([4, 14]))
            .align_y(iced::Alignment::Center);

        container(bar).width(Fill).height(STATUS_BAR_HEIGHT).into()
    }

    fn view_detail_panel(&self) -> Element<'_, Message> {
        let t = &self.theme;

        let Some(result) = self.results.get(self.selected) else {
            let hint_color = t.overlay;
            let shortcuts_col = column![
                text("Shortcuts").size(12).color(hint_color),
                container(text("")).height(8),
                text("Enter    Open").size(11).color(hint_color),
                text("\u{2191}\u{2193}      Navigate")
                    .size(11)
                    .color(hint_color),
                text("Esc       Close").size(11).color(hint_color),
                text("Tab       Detail").size(11).color(hint_color),
            ]
            .spacing(4);
            return container(
                container(shortcuts_col)
                    .width(Fill)
                    .padding(Padding::from([20, 16])),
            )
            .width(Fill)
            .height(Fill)
            .into();
        };
        let item = &result.item;

        // ── 아이콘 영역 (그라데이션 배경 위 큰 아이콘) ──
        let big_icon_size: f32 = 52.0;
        let big_icon: Element<'_, Message> = if let Some(handle) =
            crate::brand_icons::brand_icon_for_item(item.kind, &item.keywords, &item.path)
                .or_else(|| crate::brand_icons::brand_icon_for_settings(&item.keywords))
                .or_else(|| crate::app_icons::app_icon_for_item(item.kind, &item.path))
        {
            image(handle)
                .content_fit(iced::ContentFit::Fill)
                .width(big_icon_size)
                .height(big_icon_size)
                .into()
        } else {
            container(text(&item.icon).size(big_icon_size - 10.0))
                .center_x(big_icon_size)
                .center_y(big_icon_size)
                .into()
        };

        let kind_color = t.kind_color(item.kind);
        let icon_glow = Color {
            a: 0.06,
            ..kind_color
        };
        let icon_area = container(
            container(big_icon)
                .width(Fill)
                .center_x(Fill)
                .padding(Padding::from([20, 0])),
        )
        .width(Fill)
        .style(move |_: &_| container::Style {
            background: Some(Background::Color(icon_glow)),
            ..Default::default()
        });

        // ── 이름 (Bold, 중앙) ──
        let name_str = truncate_str(&item.name, 25);
        let name_label = container(
            text(name_str)
                .size(15)
                .color(t.text)
                .wrapping(Wrapping::None)
                .center(),
        )
        .width(Fill)
        .center_x(Fill)
        .padding(Padding::from([8, 12]));

        // ── "Path" 라벨 + 경로 (왼쪽 정렬, 말줄임) ──
        let path_text = if item.path.chars().count() > 35 {
            let end: String = item
                .path
                .chars()
                .rev()
                .take(32)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("...{end}")
        } else {
            item.path.clone()
        };
        let path_section = container(
            column![
                text("Path").size(10).color(t.overlay),
                text(path_text)
                    .size(11)
                    .color(t.subtext)
                    .wrapping(Wrapping::None),
            ]
            .spacing(2),
        )
        .width(Fill)
        .padding(Padding::from([4, 16]));

        // ── 뱃지 행 ──
        let kind_label = item.kind.to_string();
        let badge_bg = Color {
            a: 0.12,
            ..kind_color
        };
        let badge_border_c = Color {
            a: 0.22,
            ..kind_color
        };
        let badge = container(text(kind_label).size(10).color(kind_color))
            .padding(Padding::from([2, 8]))
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(badge_bg)),
                border: Border {
                    radius: 10.0.into(),
                    width: 1.0,
                    color: badge_border_c,
                },
                ..Default::default()
            });
        let badge_row = container(badge).width(Fill).padding(Padding::from([2, 16]));

        // ── 구분선 (패딩 있는 정돈된 선) ──
        let sep_color = t.border;
        let divider = container(
            container(text(""))
                .width(Fill)
                .height(1)
                .style(move |_: &_| container::Style {
                    background: Some(Background::Color(sep_color)),
                    ..Default::default()
                }),
        )
        .padding(Padding::from([4, 14]));

        // ── 액션 목록 ──
        let actions = context_actions_for(item.kind);
        let mut action_list = Column::new().spacing(1);
        for (i, action) in actions.iter().enumerate() {
            let action_clone = action.clone();
            let label_text = action.label().to_string();
            let shortcut_text = action.shortcut().to_string();
            let is_primary = i == 0;

            let text_color = if is_primary { t.accent } else { t.text };
            let accent_c = t.accent;
            let surface_c = t.surface2;

            let icon_text = match action {
                ContextAction::Open => "\u{2197}",
                ContextAction::OpenAsAdmin => "\u{26A1}",
                ContextAction::OpenFolder => "\u{1F4C1}",
                ContextAction::CopyPath => "\u{1F4CB}",
                ContextAction::CopyName => "\u{270D}",
                ContextAction::Uninstall => "\u{2715}",
            };

            let icon_label = text(icon_text).size(13);
            let label = text(label_text).size(12).color(text_color);
            let shortcut = text(shortcut_text).size(10).color(t.overlay);

            let action_row = row![
                container(icon_label).width(22).center_x(22),
                label,
                Space::new().width(Fill),
                shortcut
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([6, 14]));

            let action_btn =
                container(action_row)
                    .width(Fill)
                    .style(move |_: &_| container::Style {
                        background: if is_primary {
                            Some(Background::Color(Color {
                                a: 0.10,
                                ..accent_c
                            }))
                        } else {
                            Some(Background::Color(Color {
                                a: 0.0,
                                ..surface_c
                            }))
                        },
                        border: if is_primary {
                            Border {
                                radius: 6.0.into(),
                                width: 1.0,
                                color: Color {
                                    a: 0.18,
                                    ..accent_c
                                },
                            }
                        } else {
                            Border::default()
                        },
                        ..Default::default()
                    });

            let wrapped = if is_primary {
                container(
                    mouse_area(action_btn)
                        .on_press(Message::RunAction(action_clone))
                        .interaction(iced::mouse::Interaction::Pointer),
                )
                .padding(Padding::from([2, 10]))
            } else {
                container(
                    mouse_area(action_btn)
                        .on_press(Message::RunAction(action_clone))
                        .interaction(iced::mouse::Interaction::Pointer),
                )
                .padding(Padding::from([0, 10]))
            };

            action_list = action_list.push(wrapped);
        }

        let panel_content = column![
            icon_area,
            name_label,
            path_section,
            badge_row,
            divider,
            action_list,
        ]
        .spacing(0);

        container(panel_content).width(Fill).height(Fill).into()
    }
}

// ─── Shell Terminal Launch ────────────────────────────────────────────────────

/// 새 터미널 창에서 셸 명령을 실행 (결과가 화면에 유지됨)
fn launch_in_terminal(cmd: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let _ = std::process::Command::new("cmd")
            .args(["/k", cmd])
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let escaped = cmd.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("tell application \"Terminal\" to do script \"{}\"", escaped);
        if let Err(e) = std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn()
        {
            tracing::warn!("터미널 실행 실패: {e}");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let escaped = cmd.replace('\'', "'\\''");
        for term in &["x-terminal-emulator", "gnome-terminal", "xterm"] {
            if std::process::Command::new(term)
                .args([
                    "-e",
                    &format!("sh -c '{escaped} ; read -p \"Press Enter...\"'"),
                ])
                .spawn()
                .is_ok()
            {
                return;
            }
        }
    }
}

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
        keywords: "@ll @llm @multi @cmp multi llm compare".to_string(),
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
        keywords: "@m @mw @msearch @multisearch @searchall @krsearch multi web".to_string(),
    });
}

fn help_query_seed(name: &str) -> Option<&'static str> {
    if name.starts_with("@ll") || name.starts_with("@llm") {
        Some("@ll ")
    } else if name.starts_with("@m") {
        Some("@m ")
    } else if name.starts_with("@") {
        Some("@")
    } else if name.starts_with(":calc") {
        Some(":calc ")
    } else if name.starts_with(":emoji") {
        Some(":emoji ")
    } else if name.starts_with(":set") {
        Some(":set")
    } else if name.starts_with(":keymap") {
        Some(":keymap")
    } else if name.starts_with(":version") || name.starts_with("Version Info") {
        Some(":version")
    } else if name.starts_with("!") {
        Some("!")
    } else if name.starts_with("Fuzzy Search") {
        Some("firefox")
    } else if name.starts_with("*.ext") {
        Some("*.pdf")
    } else if name.starts_with("/regex/") {
        Some("/test\\d+/")
    } else {
        None
    }
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
        let tmp = std::env::temp_dir().join(format!(
            "kmd_test_app_{}_{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::create_dir_all(&tmp);

        let guard = match kmd_core::single_instance::acquire_or_toggle(&tmp) {
            kmd_core::single_instance::InstanceAction::Acquired(g) => g,
            _ => panic!("Guard 획득 실패"),
        };
        let config = kmd_core::Config::default();
        let window_state = WindowState::default();
        let (app, _task) = App::new(guard, config, window_state);
        app
    }

    // ── 레이아웃 상수 일관성 ──

    #[test]
    fn 전체_창_높이_계산_일관성() {
        let expected =
            SEARCH_BAR_HEIGHT + 1.0 + (MAX_VISIBLE_ROWS as f32 * ROW_HEIGHT) + 1.0 + STATUS_BAR_HEIGHT;
        assert!(
            (FULL_WINDOW_HEIGHT - expected).abs() < f32::EPSILON,
            "FULL_WINDOW_HEIGHT({FULL_WINDOW_HEIGHT}) != 계산값({expected})"
        );
    }

    #[test]
    fn pill_높이가_폰트보다_충분히_큼() {
        let input_font_size: f32 = 20.0;
        assert!(
            PILL_HEIGHT > input_font_size + 10.0,
            "PILL_HEIGHT({PILL_HEIGHT})가 폰트({input_font_size})보다 상하 마진 포함해 충분히 커야 함"
        );
    }

    #[test]
    fn row_높이가_아이콘보다_큼() {
        let icon_size: f32 = 32.0;
        assert!(
            ROW_HEIGHT > icon_size + 8.0,
            "ROW_HEIGHT({ROW_HEIGHT})가 아이콘({icon_size})보다 충분히 커야 함"
        );
    }

    // ── 포커스 상태 관리 ──

    #[test]
    fn 쿼리_변경시_window_focused_유지() {
        let mut app = make_test_app();
        assert!(app.window_focused, "초기 상태에서 window_focused = true");

        let _task = app.update(Message::QueryChanged("test".to_string()));
        assert!(
            app.window_focused,
            "QueryChanged 후에도 window_focused 유지되어야 함"
        );
        assert_eq!(app.query, "test");
    }

    #[test]
    fn 쿼리_비어있을때_ensure_focus_실행_가능() {
        let mut app = make_test_app();
        app.window_focused = true;
        app.query.clear();
        let _task = app.update(Message::EnsureFocus(1));
        // EnsureFocus 가드: attempt <= MAX 이고 query가 비어있으면 실행
        // (Task 내부는 검증 불가하지만, 상태 변경 없음을 확인)
        assert!(app.window_focused);
    }

    #[test]
    fn 쿼리_있을때_ensure_focus_건너뜀() {
        let mut app = make_test_app();
        app.window_focused = true;
        app.query = "hello".to_string();
        let _task = app.update(Message::EnsureFocus(1));
        // query가 비어있지 않으므로 EnsureFocus는 Task::none() 반환
        assert!(app.window_focused);
    }

    #[test]
    fn 최대_재시도_초과시_ensure_focus_중단() {
        let mut app = make_test_app();
        app.window_focused = true;
        app.query.clear();
        let _task = app.update(Message::EnsureFocus(MAX_FOCUS_RETRIES + 1));
        assert!(app.window_focused);
    }

    // ── Unfocused 이벤트 타이핑 가드 ──

    #[test]
    fn 타이핑_직후_unfocused_무시() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());

        // 타이핑 시뮬레이션
        let _task = app.update(Message::QueryChanged("가".to_string()));
        assert!(app.window_focused, "QueryChanged 직후 focused 유지");

        // 타이핑 직후(500ms 이내) Unfocused 이벤트
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
        // last_query_changed_at을 충분히 과거로 설정
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

    // ── Focused 이벤트 — 항상 refocus ──

    #[test]
    fn 쿼리_있어도_focused시_window_focused_복원() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        app.window_focused = false;
        app.query = "검색어".to_string();

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Focused,
        ));
        assert!(
            app.window_focused,
            "Focused 이벤트 시 query 상태와 무관하게 window_focused=true"
        );
    }

    #[test]
    fn 빈_쿼리_focused시_window_focused_복원() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());
        app.window_focused = false;
        app.query.clear();

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Focused,
        ));
        assert!(app.window_focused, "Focused 이벤트 시 window_focused=true");
    }

    // ── 한글 입력 IME 보호 ──

    #[test]
    fn 한글_자모_연속_입력시_상태_일관성() {
        let mut app = make_test_app();

        // ㄱ 입력 (자음)
        let _task = app.update(Message::QueryChanged("ㄱ".to_string()));
        assert_eq!(app.query, "ㄱ");
        assert!(app.window_focused);
        let ts1 = app.last_query_changed_at;

        // ㅏ 결합 → 가
        let _task = app.update(Message::QueryChanged("가".to_string()));
        assert_eq!(app.query, "가");
        assert!(app.window_focused);
        assert!(
            app.last_query_changed_at >= ts1,
            "last_query_changed_at은 갱신되어야 함"
        );
    }

    #[test]
    fn 한글_입력중_spurious_unfocused_무시() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());

        // 한글 입력
        let _task = app.update(Message::QueryChanged("ㄱ".to_string()));

        // 바로 spurious Unfocused 발생
        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Unfocused,
        ));
        assert!(
            app.window_focused,
            "한글 조합 중 spurious Unfocused는 무시"
        );

        // 이어서 Focused 발생
        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Focused,
        ));
        assert!(app.window_focused, "Focused 후 복원");

        // ㅏ 결합 가능
        let _task = app.update(Message::QueryChanged("가".to_string()));
        assert_eq!(app.query, "가", "한글 조합이 정상 완료되어야 함");
    }

    #[test]
    fn 영문_입력중_spurious_unfocused_무시() {
        let mut app = make_test_app();
        app.window_id = Some(window::Id::unique());

        let _task = app.update(Message::QueryChanged("h".to_string()));
        assert!(app.window_focused);

        let _task = app.update(Message::WindowEvent(
            app.window_id.unwrap(),
            window::Event::Unfocused,
        ));
        assert!(
            app.window_focused,
            "영문 입력 직후 spurious Unfocused 무시"
        );

        let _task = app.update(Message::QueryChanged("he".to_string()));
        assert_eq!(app.query, "he", "연속 입력 가능해야 함");
    }

    #[test]
    fn got_raw_window_id_쿼리_있으면_focus_건너뜀() {
        let mut app = make_test_app();
        app.query = "test".to_string();
        let _task = app.update(Message::GotRawWindowId(12345));
        assert!(app.window_focused);
    }

    #[test]
    fn clear_search_후_focus_복원() {
        let mut app = make_test_app();
        app.query = "something".to_string();
        let _task = app.update(Message::QueryChanged("".to_string()));
        assert_eq!(app.query, "");
        assert!(app.window_focused);
    }
}
