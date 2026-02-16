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
use iced::widget::{
    column, container, mouse_area, row, scrollable, text, text_input, Column, Space,
};
use iced::{
    window, Background, Border, Color, Element, Fill, Padding, Point, Shadow, Size, Subscription,
    Task, Vector,
};

use kmd_core::plugin::{builtin_calc, builtin_emoji, builtin_shell, Extension};
use kmd_core::single_instance::Guard;
use kmd_core::web;
use kmd_core::{IndexItem, ItemKind, Source};

use crate::theme::DesktopTheme;
use crate::window_state::WindowState;

// ─── Constants ────────────────────────────────────────────────────────────────

pub const DEFAULT_WIDTH: f32 = 680.0;
pub const SEARCH_BAR_HEIGHT: f32 = 56.0;
const ROW_HEIGHT: f32 = 52.0;
const STATUS_BAR_HEIGHT: f32 = 28.0;
const MAX_VISIBLE_ROWS: usize = 8;
const SEARCH_LIMIT: usize = 50;
const SCORE_PLUGIN: u32 = u32::MAX;

/// Interval between quit-signal polls (ms). Also used to flush state to disk.
const QUIT_POLL_MS: u64 = 300;

// ─── Shared slot for async engine hand-off ────────────────────────────────────

type EngineSlot = Arc<Mutex<Option<(kmd_core::SearchEngine, bool, Vec<String>)>>>;

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
    use_emoji: bool,
    selected_llm_providers: Vec<String>,
    loading: bool,
    engine_slot: EngineSlot,
    _guard: Guard,

    // ── Window geometry ───────────────────────────────────────────────
    window_width: f32,
    window_state: WindowState,
    state_dirty: bool,

    // ── IME ───────────────────────────────────────────────────────────
    reset_ime_on_launch: bool,
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
    EngineReady,
    CheckQuitSignal,
    WindowEvent(window::Id, window::Event),
}

// ─── Boot ─────────────────────────────────────────────────────────────────────

impl App {
    pub fn new(
        guard: Guard,
        config: kmd_core::Config,
        window_state: WindowState,
    ) -> (Self, Task<Message>) {
        let engine = kmd_core::SearchEngine::new();
        let theme = crate::theme::from_name(&config.general.theme);
        let use_emoji = config.general.emoji_icons;
        let selected_llm_providers = config.launcher.multi_llm_providers.clone();
        let reset_ime = config.general.reset_ime_on_launch;
        let window_width = window_state.width.unwrap_or(DEFAULT_WIDTH);

        let input_id = iced::widget::Id::unique();
        let scrollable_id = iced::widget::Id::unique();
        let engine_slot: EngineSlot = Arc::new(Mutex::new(None));

        let slot_for_task = engine_slot.clone();
        let load_task = Task::future(async move {
            let _ = tokio::task::spawn_blocking(move || {
                let eng = crate::engine::create_search_engine(&config);
                let emoji = config.general.emoji_icons;
                let llm = config.launcher.multi_llm_providers.clone();
                *slot_for_task.lock().expect("engine_slot poisoned") = Some((eng, emoji, llm));
            })
            .await;
            Message::EngineReady
        });

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
            use_emoji,
            selected_llm_providers,
            loading: true,
            engine_slot,
            _guard: guard,
            window_width,
            window_state,
            state_dirty: false,
            reset_ime_on_launch: reset_ime,
        };

        let focus_task = iced::widget::operation::focus::<Message>(input_id);
        let id_task = window::oldest().map(Message::GotWindowId);
        (app, Task::batch([focus_task, id_task, load_task]))
    }

    // ─── Update ───────────────────────────────────────────────────────────

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QueryChanged(query) => {
                self.query = query;
                self.selected = 0;
                self.perform_search()
            }
            Message::Submit => self.launch_selected(),
            Message::ResultClicked(index) => {
                self.selected = index;
                self.launch_selected()
            }
            Message::KeyEvent(key, _modifiers) => self.handle_key(key),
            Message::BrandClicked => {
                // Toggle quick help.
                if self.query.starts_with(":help") {
                    self.clear_query_and_refocus()
                } else {
                    self.query = ":help".to_string();
                    self.perform_search()
                }
            }
            Message::BrandRightClicked => {
                // Toggle settings.
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
                    Some(id) => Task::batch([
                        iced::widget::operation::focus::<Message>(self.input_id.clone()),
                        window::raw_id::<Message>(id).map(Message::GotRawWindowId),
                    ]),
                    None => iced::widget::operation::focus::<Message>(self.input_id.clone()),
                }
            }
            Message::GotRawWindowId(raw_id) => {
                crate::platform::force_square_corners(raw_id);
                if self.reset_ime_on_launch {
                    crate::platform::force_english_ime(raw_id);
                }
                Task::none()
            }
            Message::EngineReady => {
                let loaded = self
                    .engine_slot
                    .lock()
                    .expect("engine_slot poisoned")
                    .take();

                if let Some((engine, emoji, llm)) = loaded {
                    self.engine = engine;
                    self.use_emoji = emoji;
                    self.selected_llm_providers = llm;
                    self.loading = false;
                    tracing::info!("Search engine ready");
                    if !self.query.trim().is_empty() {
                        return self.perform_search();
                    }
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
                    window::Event::Moved(point) => {
                        self.window_state.x = Some(point.x);
                        self.window_state.y = Some(point.y);
                        self.state_dirty = true;
                    }
                    window::Event::Resized(size) => {
                        if (size.width - self.window_width).abs() > 1.0 {
                            self.window_width = size.width;
                            self.window_state.width = Some(size.width);
                            self.state_dirty = true;
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
        }
    }

    // ─── Subscription ─────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                Some(Message::KeyEvent(key, modifiers))
            }
            _ => None,
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
/// | `:help`   | Help / commands   | `:help`, `:h`                  |
/// | `!`       | Shell command     | `!ip`, `!echo hello`           |
/// | (other)   | Fuzzy / glob / …  | `firefox`, `*.pdf`, `한글`     |
impl App {
    fn perform_search(&mut self) -> Task<Message> {
        let query = self.query.clone();
        let trimmed = query.trim();

        if trimmed.is_empty() {
            self.results.clear();
            self.search_mode = kmd_core::SearchMode::Fuzzy;
        } else {
            match prefix_of(trimmed) {
                Prefix::Web => self.handle_web_query(trimmed),
                Prefix::Calc => self.handle_calc_query(trimmed),
                Prefix::Emoji => self.handle_emoji_query(trimmed),
                Prefix::Settings => self.handle_settings_query(trimmed),
                Prefix::Help => self.handle_help_query(),
                Prefix::Version => self.handle_version_query(),
                Prefix::Shell => self.handle_shell_query(trimmed),
                Prefix::General => self.handle_main_search(trimmed),
            }
        }

        self.resize_window()
    }

    fn handle_web_query(&mut self, query: &str) {
        let emoji = self.use_emoji;
        if let Some((_services, q)) =
            web::parse_multi_llm_query(query, &self.selected_llm_providers)
        {
            self.results = items_to_results(web::multi_llm_result_items(
                &q,
                &self.selected_llm_providers,
                emoji,
            ));
            self.search_mode = kmd_core::SearchMode::Contains;
            self.selected = 0;
            return;
        }

        if let Some((service, q)) = web::parse_web_query(query) {
            if q.is_empty() {
                let mut items = web::list_services_as_items("", emoji);
                ensure_multi_llm_hint(&mut items, emoji);
                self.results = items_to_results(items);
            } else {
                let item = web::search_result_item(service, &q, emoji);
                self.results = items_to_results(std::iter::once(item));
            }
        } else {
            let filter = query.trim_start_matches('@');
            let mut items = web::list_services_as_items(filter, emoji);
            ensure_multi_llm_hint(&mut items, emoji);
            self.results = items_to_results(items);
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
        self.results = items_to_results(version_items);
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_calc_query(&mut self, query: &str) {
        let expr = query.strip_prefix(":calc").unwrap_or("").trim();
        let calc = builtin_calc::CalcExtension;
        self.results = items_to_results(calc.search_with_emoji(expr, self.use_emoji));
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_emoji_query(&mut self, query: &str) {
        let search_query = query
            .strip_prefix(":emoji")
            .or_else(|| query.strip_prefix(":e"))
            .unwrap_or("")
            .trim();
        let emoji_ext = builtin_emoji::EmojiExtension;
        self.results = items_to_results(emoji_ext.search_emoji(search_query));
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_shell_query(&mut self, query: &str) {
        let shell_query = query.strip_prefix('!').unwrap_or("").trim();
        let shell_ext = builtin_shell::ShellExtension;
        self.results = items_to_results(shell_ext.search(shell_query));
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
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
                ":version  Version Info",
                "Type :version  (show desktop/core/target/os versions)",
                if emoji { "\u{1F4E6}" } else { "[VER]" },
            ),
            (
                "@llm  Multi LLM Compare",
                "Type @llm prompt  (open selected LLM providers in parallel tabs)",
                if emoji { "\u{1F9E0}" } else { "[LLM]" },
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
                let _ = open::that(&config_path);
            }
            "dir" => {
                let config_dir = kmd_core::Config::default_config_dir();
                let _ = open::that(&config_dir);
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
                self.loading = true;
                let slot = self.engine_slot.clone();
                let task = Task::future(async move {
                    let _ = tokio::task::spawn_blocking(move || {
                        let config = crate::engine::load_config();
                        let eng = crate::engine::create_search_engine(&config);
                        let emoji = config.general.emoji_icons;
                        let llm = config.launcher.multi_llm_providers.clone();
                        *slot.lock().expect("engine_slot poisoned") = Some((eng, emoji, llm));
                    })
                    .await;
                    Message::EngineReady
                });
                self.query.clear();
                self.results.clear();
                self.selected = 0;
                return Task::batch([self.resize_window(), task]);
            }
            "toggle_ime_reset" => {
                self.reset_ime_on_launch = !self.reset_ime_on_launch;
                let new_val = self.reset_ime_on_launch;
                tracing::info!("reset_ime_on_launch = {new_val}");
                save_config(|cfg| cfg.general.reset_ime_on_launch = new_val);

                // Re-open settings so the user sees the updated [ON/OFF] label.
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return self.resize_window();
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
                return self.resize_window();
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
        self.resize_window()
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

        self.results = results;
        self.selected = 0;
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

        if self.state_dirty {
            self.window_state.save();
        }

        if result.item.kind == ItemKind::WebSearch {
            if let Some(urls) = web::extract_multi_llm_urls(&result.item) {
                for url in urls {
                    let _ = kmd_core::action::open_url(&url);
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
        .into()
    }

    fn clear_query_and_refocus(&mut self) -> Task<Message> {
        self.query.clear();
        self.results.clear();
        self.selected = 0;
        Task::batch([
            self.resize_window(),
            iced::widget::operation::focus::<Message>(self.input_id.clone()),
        ])
    }

    fn resize_window(&self) -> Task<Message> {
        let height = if self.results.is_empty() {
            SEARCH_BAR_HEIGHT
        } else {
            let rows = self.results.len().min(MAX_VISIBLE_ROWS) as f32;
            SEARCH_BAR_HEIGHT + (rows * ROW_HEIGHT) + STATUS_BAR_HEIGHT
        };
        let size = Size::new(self.window_width, height);

        match self.window_id {
            Some(id) => window::resize(id, size),
            None => window::oldest().then(move |maybe_id| match maybe_id {
                Some(id) => window::resize(id, size),
                None => Task::none(),
            }),
        }
    }
}

// ─── Prefix Detection ─────────────────────────────────────────────────────────

enum Prefix {
    Web,
    Calc,
    Emoji,
    Settings,
    Help,
    Version,
    Shell,
    General,
}

fn prefix_of(query: &str) -> Prefix {
    if query.starts_with('@') {
        Prefix::Web
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

        let search_bar = self.view_search_bar();
        let mut content = Column::new().push(search_bar);

        if has_results {
            let border_color = t.border;
            content = content.push(container(text("")).width(Fill).height(1).style(
                move |_: &_| container::Style {
                    background: Some(Background::Color(border_color)),
                    ..Default::default()
                },
            ));
            content = content.push(self.view_results_list());
            content = content.push(self.view_status_bar());
            content = content.push(self.view_accent_bar());
        }

        let bg = t.background_with_opacity();
        let radius = t.corner_radius;
        let shadow_i = t.shadow_intensity;
        let border_color = Color {
            a: 0.35,
            ..t.accent
        };

        let body = container(content)
            .width(Fill)
            .height(Fill)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: radius.into(),
                    width: 1.5,
                    color: border_color,
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.25 * shadow_i),
                    offset: Vector::new(0.0, 2.0),
                    blur_radius: 6.0,
                },
                text_color: None,
                snap: false,
            });

        // Invisible edge grips for natural resize on borderless window.
        let left_edge_resize = mouse_area(container(text("")).width(8).height(Fill))
            .on_press(Message::StartWindowResize(window::Direction::West))
            .interaction(iced::mouse::Interaction::ResizingHorizontally);
        let right_edge_resize = mouse_area(container(text("")).width(8).height(Fill))
            .on_press(Message::StartWindowResize(window::Direction::East))
            .interaction(iced::mouse::Interaction::ResizingHorizontally);

        row![left_edge_resize, body, right_edge_resize]
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn view_search_bar(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let text_color = t.text;
        let overlay_color = t.overlay;
        let accent_color = t.accent;
        let surface = t.surface;
        let has_results = !self.results.is_empty();

        let bar_surface = Color {
            r: (surface.r + 0.03).min(1.0),
            g: (surface.g + 0.03).min(1.0),
            b: (surface.b + 0.03).min(1.0),
            a: surface.a,
        };

        let radius = t.corner_radius;
        let bar_border_width: f32 = if has_results { 0.0 } else { 1.5 };
        let bar_shadow_blur: f32 = if has_results { 0.0 } else { 8.0 };

        let brand = mouse_area(
            container(text("\u{00BB}").size(24).color(t.peach)).padding(Padding::from([0, 4])),
        )
        .on_press(Message::BrandClicked)
        .on_right_press(Message::BrandRightClicked)
        .interaction(iced::mouse::Interaction::Pointer);

        let placeholder = if self.loading {
            "Loading..."
        } else {
            "Search anything...  (:help for commands)"
        };

        let input = text_input(placeholder, &self.query)
            .id(self.input_id.clone())
            .on_input(Message::QueryChanged)
            .on_submit(Message::Submit)
            .width(Fill)
            .size(18)
            .padding(0)
            .style(move |_theme, _status| text_input::Style {
                background: Background::Color(bar_surface),
                border: Border::default(),
                icon: overlay_color,
                placeholder: overlay_color,
                value: text_color,
                selection: Color {
                    a: 0.3,
                    ..accent_color
                },
            });

        let mode_text = if self.query.is_empty() {
            ""
        } else {
            self.search_mode.label()
        };
        let badge = text(mode_text).size(11).color(t.overlay);

        let bar_content = row![brand, input, badge]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([0, 16]));

        // Depth layering (raised card 3D effect)
        let highlight_color = Color::from_rgba(1.0, 1.0, 1.0, 0.06);
        let shadow_line_color = Color::from_rgba(0.0, 0.0, 0.0, 0.3);
        let border_glow = Color {
            a: 0.30,
            ..accent_color
        };

        // Full-width drag strip so users can move window naturally.
        let top_drag_strip = mouse_area(container(text("")).width(Fill).height(12).style(
            move |_: &_| container::Style {
                background: Some(Background::Color(highlight_color)),
                ..Default::default()
            },
        ))
        .on_press(Message::StartWindowDrag)
        .interaction(iced::mouse::Interaction::Grab);

        let main_bar = container(bar_content)
            .width(Fill)
            .height(SEARCH_BAR_HEIGHT - 13.0)
            .center_y(Fill);

        let bottom_shadow = container(text(""))
            .width(Fill)
            .height(1)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(if has_results {
                    shadow_line_color
                } else {
                    Color::TRANSPARENT
                })),
                ..Default::default()
            });

        let layered = column![top_drag_strip, main_bar, bottom_shadow];

        container(layered)
            .width(Fill)
            .height(SEARCH_BAR_HEIGHT)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(bar_surface)),
                border: Border {
                    radius: radius.into(),
                    width: bar_border_width,
                    color: border_glow,
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                    offset: Vector::new(0.0, 2.0),
                    blur_radius: bar_shadow_blur,
                },
                text_color: None,
                snap: false,
            })
            .into()
    }

    fn view_results_list(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let mut list = Column::new().spacing(0);
        for (i, result) in self.results.iter().enumerate() {
            list = list.push(self.view_result_row(i, result));
        }

        let rows_count = self.results.len().min(MAX_VISIBLE_ROWS);
        let list_height = rows_count as f32 * ROW_HEIGHT;
        let bg = t.background_with_opacity();

        scrollable(
            container(list)
                .width(Fill)
                .style(move |_: &_| container::Style {
                    background: Some(Background::Color(bg)),
                    ..Default::default()
                }),
        )
        .id(self.scrollable_id.clone())
        .height(list_height)
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

        let icon = text(&item.icon).size(22);
        let title = text(&item.name).size(14).color(t.text);
        let subtitle = text(&item.path).size(11).color(t.subtext);
        let info = column![title, subtitle].spacing(2);

        let kind_color = t.kind_color(item.kind);
        let kind_label = item.kind.to_string();
        let badge_bg = Color {
            a: 0.12,
            ..kind_color
        };
        let badge_border = Color {
            a: 0.25,
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

        let row_content = row![left_bar, icon, info, Space::new().width(Fill), badge]
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
            .padding(Padding::from([4, 16]))
            .align_y(iced::Alignment::Center);

        container(bar)
            .width(Fill)
            .height(STATUS_BAR_HEIGHT - 2.0)
            .into()
    }

    fn view_accent_bar(&self) -> Element<'_, Message> {
        let accent = self.theme.accent;
        container(text(""))
            .width(Fill)
            .height(2)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(accent)),
                ..Default::default()
            })
            .into()
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

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
    if items.iter().any(|item| item.name.starts_with("@llm")) {
        return;
    }
    items.push(IndexItem {
        name: "@llm        Compare multiple LLMs with one prompt".to_string(),
        path: "Open selected LLM providers in parallel tabs".to_string(),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: if use_emoji { "\u{1F9E0}" } else { "Ml" }.to_string(),
        keywords: "@llm @multi @cmp multi llm compare".to_string(),
    });
}

fn help_query_seed(name: &str) -> Option<&'static str> {
    if name.starts_with("@llm") {
        Some("@llm ")
    } else if name.starts_with("@") {
        Some("@")
    } else if name.starts_with(":calc") {
        Some(":calc ")
    } else if name.starts_with(":emoji") {
        Some(":emoji ")
    } else if name.starts_with(":set") {
        Some(":set")
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
