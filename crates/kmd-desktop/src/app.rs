//! Application state, messages, update/view/subscription — the Elm core.
//!
//! Renders a Spotlight-like floating launcher: search bar always visible,
//! results list + status bar appear only when there are results.
//! Supports singleton toggle via `kmd_core::single_instance::Guard`.
//!
//! **Async engine loading**: the window appears instantly; the search engine
//! is loaded on a background thread and swapped in when ready.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::keyboard;
use iced::widget::{
    column, container, mouse_area, row, scrollable, text, text_input, Column, Space,
};
use iced::{
    window, Background, Border, Color, Element, Fill, Padding, Shadow, Size, Subscription, Task,
    Vector,
};

use kmd_core::plugin::{builtin_calc, builtin_emoji, builtin_shell, Extension};
use kmd_core::single_instance::Guard;
use kmd_core::web;
use kmd_core::{IndexItem, ItemKind, Source};

use crate::theme::DesktopTheme;

// ─── Constants ────────────────────────────────────────────────────────────────

const WINDOW_WIDTH: f32 = 680.0;
const SEARCH_BAR_HEIGHT: f32 = 56.0;
const ROW_HEIGHT: f32 = 52.0;
const STATUS_BAR_HEIGHT: f32 = 28.0;
const MAX_VISIBLE_ROWS: usize = 8;
const SEARCH_LIMIT: usize = 50;
const SCORE_PLUGIN: u32 = u32::MAX;

/// Interval between quit-signal polls (ms).
const QUIT_POLL_MS: u64 = 300;

// ─── Shared slot for async engine hand-off ────────────────────────────────────

/// Engine + emoji-icons flag, loaded on a background thread.
type EngineSlot = Arc<Mutex<Option<(kmd_core::SearchEngine, bool)>>>;

// ─── App State ────────────────────────────────────────────────────────────────

pub struct App {
    query: String,
    results: Vec<kmd_core::SearchResult>,
    search_mode: kmd_core::SearchMode,
    selected: usize,
    engine: kmd_core::SearchEngine,
    theme: DesktopTheme,
    input_id: iced::widget::Id,
    window_id: Option<window::Id>,
    use_emoji: bool,
    /// `true` while the background engine load is in progress.
    loading: bool,
    /// Shared slot — background thread deposits the engine here.
    engine_slot: EngineSlot,
    /// Singleton guard — dropping it removes the lock file.
    _guard: Guard,
}

// ─── Messages ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    QueryChanged(String),
    Submit,
    ResultClicked(usize),
    KeyEvent(keyboard::Key, keyboard::Modifiers),
    GotWindowId(Option<window::Id>),
    /// Background engine finished loading — swap it in.
    EngineReady,
    /// Periodic tick — check if another instance told us to quit.
    CheckQuitSignal,
}

// ─── Boot ─────────────────────────────────────────────────────────────────────

impl App {
    pub fn new(guard: Guard) -> (Self, Task<Message>) {
        // Create an *empty* engine so the window can appear instantly.
        let engine = kmd_core::SearchEngine::new();
        let theme = crate::theme::midnight();
        let input_id = iced::widget::Id::unique();
        let engine_slot: EngineSlot = Arc::new(Mutex::new(None));

        // ── Background engine loading ──────────────────────────────────────
        let slot_for_task = engine_slot.clone();
        let load_task = Task::future(async move {
            // spawn_blocking so we don't stall the async executor.
            let _ = tokio::task::spawn_blocking(move || {
                let config = crate::engine::load_config();
                let eng = crate::engine::create_search_engine(&config);
                let emoji = config.general.emoji_icons;
                *slot_for_task.lock().expect("engine_slot poisoned") = Some((eng, emoji));
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
            window_id: None,
            use_emoji: true, // default until config loads
            loading: true,
            engine_slot,
            _guard: guard,
        };

        // Focus input + fetch window ID + background engine load.
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
            Message::GotWindowId(id) => {
                self.window_id = id;
                iced::widget::operation::focus::<Message>(self.input_id.clone())
            }
            Message::EngineReady => {
                // Take engine out of the shared slot (drop lock before re-search).
                let loaded = self
                    .engine_slot
                    .lock()
                    .expect("engine_slot poisoned")
                    .take();

                if let Some((engine, emoji)) = loaded {
                    self.engine = engine;
                    self.use_emoji = emoji;
                    self.loading = false;
                    tracing::info!("Search engine ready");

                    // If the user already typed something, re-search now.
                    if !self.query.trim().is_empty() {
                        return self.perform_search();
                    }
                }
                Task::none()
            }
            Message::CheckQuitSignal => {
                if self._guard.should_quit() {
                    self._guard.consume_quit_signal();
                    tracing::info!("Received quit signal from another instance — exiting");
                    return iced::exit();
                }
                Task::none()
            }
        }
    }

    // ─── Subscription ─────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed {
                key, modifiers, ..
            } => Message::KeyEvent(key, modifiers),
            _ => Message::KeyEvent(
                keyboard::Key::Named(keyboard::key::Named::Shift),
                keyboard::Modifiers::default(),
            ),
        });

        let quit_sub = iced::time::every(Duration::from_millis(QUIT_POLL_MS))
            .map(|_| Message::CheckQuitSignal);

        Subscription::batch([keyboard_sub, quit_sub])
    }

    pub fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

// ─── Search Logic (with plugin integration) ───────────────────────────────────

impl App {
    fn perform_search(&mut self) -> Task<Message> {
        let query = self.query.clone();

        if query.trim().is_empty() {
            self.results.clear();
            self.search_mode = kmd_core::SearchMode::Fuzzy;
        } else if query.starts_with('@') {
            self.handle_web_query(&query);
        } else if query.starts_with(":calc") {
            self.handle_calc_query(&query);
        } else if query.starts_with(":emoji") || query.starts_with(":e ") || query == ":e" {
            self.handle_emoji_query(&query);
        } else if query.starts_with(":set") {
            self.handle_settings_query(&query);
        } else if query.starts_with('!') {
            self.handle_shell_query(&query);
        } else {
            self.handle_main_search(&query);
        }

        self.resize_window()
    }

    fn handle_web_query(&mut self, query: &str) {
        let emoji = self.use_emoji;
        if let Some((service, q)) = web::parse_web_query(query) {
            if q.is_empty() {
                self.results = items_to_results(web::list_services_as_items("", emoji));
            } else {
                let item = web::search_result_item(service, &q, emoji);
                self.results = items_to_results(std::iter::once(item));
            }
        } else {
            let filter = query.trim_start_matches('@');
            self.results = items_to_results(web::list_services_as_items(filter, emoji));
        }
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
        // Split into command part and filter argument.
        // The canonical command is ":settings", but partial prefixes like
        // ":set", ":sett", ":setti", ":settin", ":setting" are also accepted.
        // The filter is everything after the first space (if any).
        let filter = match query.find(' ') {
            Some(pos) => query[pos + 1..].trim().to_lowercase(),
            None => String::new(),
        };

        let emoji = self.use_emoji;
        let mut items: Vec<IndexItem> = Vec::new();

        let settings_entries = [
            ("Edit Config File", "kmd:settings:config", if emoji { "\u{2699}\u{FE0F}" } else { "[CFG]" }),
            ("Open Config Directory", "kmd:settings:dir", if emoji { "\u{1F4C2}" } else { "[DIR]" }),
            ("Theme: Midnight (default)", "kmd:settings:theme:midnight", if emoji { "\u{1F319}" } else { "[THM]" }),
            ("Theme: Obsidian", "kmd:settings:theme:obsidian", if emoji { "\u{2B1B}" } else { "[THM]" }),
            ("Theme: Snow", "kmd:settings:theme:snow", if emoji { "\u{2600}\u{FE0F}" } else { "[THM]" }),
            ("Theme: Rose Pine", "kmd:settings:theme:rose_pine", if emoji { "\u{1F339}" } else { "[THM]" }),
            ("Theme: Nord", "kmd:settings:theme:nord", if emoji { "\u{2744}\u{FE0F}" } else { "[THM]" }),
            ("Rebuild Index", "kmd:settings:rebuild", if emoji { "\u{1F504}" } else { "[IDX]" }),
        ];

        for (name, path, icon) in settings_entries {
            if filter.is_empty() || name.to_lowercase().contains(&filter) {
                items.push(IndexItem {
                    name: name.to_string(),
                    path: path.to_string(),
                    icon: icon.to_string(),
                    kind: ItemKind::SystemCommand,
                    source: Source::Plugin,
                    keywords: String::new(),
                });
            }
        }

        self.results = items_to_results(items);
        self.search_mode = kmd_core::SearchMode::Contains;
        self.selected = 0;
    }

    fn handle_settings_action(&mut self, result: &kmd_core::SearchResult) -> Task<Message> {
        let action = result.item.path.strip_prefix("kmd:settings:").unwrap_or("");

        match action {
            "config" => {
                let config_dir = kmd_core::Config::default_config_dir();
                let config_path = config_dir.join(kmd_core::CONFIG_FILENAME);
                if config_path.exists() {
                    let _ = open::that(&config_path);
                    tracing::info!("Opened config file: {}", config_path.display());
                } else {
                    tracing::warn!("Config file not found: {}", config_path.display());
                }
            }
            "dir" => {
                let config_dir = kmd_core::Config::default_config_dir();
                let _ = open::that(&config_dir);
                tracing::info!("Opened config directory: {}", config_dir.display());
            }
            "rebuild" => {
                // Rebuild engine asynchronously
                self.loading = true;
                let slot = self.engine_slot.clone();
                let task = Task::future(async move {
                    let _ = tokio::task::spawn_blocking(move || {
                        let config = crate::engine::load_config();
                        let eng = crate::engine::create_search_engine(&config);
                        let emoji = config.general.emoji_icons;
                        *slot.lock().expect("engine_slot poisoned") = Some((eng, emoji));
                    })
                    .await;
                    Message::EngineReady
                });
                self.query.clear();
                self.results.clear();
                self.selected = 0;
                return Task::batch([self.resize_window(), task]);
            }
            theme_action if theme_action.starts_with("theme:") => {
                let theme_name = theme_action.strip_prefix("theme:").unwrap_or("midnight");
                self.theme = crate::theme::from_name(theme_name);
                tracing::info!("Theme changed to: {}", self.theme.name);
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

        if result.item.kind == ItemKind::SystemCommand
            && result.item.path.starts_with("kmd:settings:")
        {
            return self.handle_settings_action(&result);
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
                tracing::warn!("Action needs confirmation: {msg}");
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
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                let max = self.results.len().saturating_sub(1);
                if self.selected < max {
                    self.selected += 1;
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.results.clear();
                    self.selected = 0;
                    return self.resize_window();
                }
                return iced::exit();
            }
            _ => {}
        }
        Task::none()
    }

    fn resize_window(&self) -> Task<Message> {
        let height = if self.results.is_empty() {
            SEARCH_BAR_HEIGHT
        } else {
            let rows = self.results.len().min(MAX_VISIBLE_ROWS) as f32;
            SEARCH_BAR_HEIGHT + (rows * ROW_HEIGHT) + STATUS_BAR_HEIGHT
        };
        let size = Size::new(WINDOW_WIDTH, height);

        match self.window_id {
            Some(id) => window::resize(id, size),
            None => window::oldest().then(move |maybe_id| match maybe_id {
                Some(id) => window::resize(id, size),
                None => Task::none(),
            }),
        }
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
            content = content.push(
                container(text(""))
                    .width(Fill)
                    .height(1)
                    .style(move |_: &_| container::Style {
                        background: Some(Background::Color(border_color)),
                        ..Default::default()
                    }),
            );
            content = content.push(self.view_results_list());
            content = content.push(self.view_status_bar());
            content = content.push(self.view_accent_bar());
        }

        let bg = t.background_with_opacity();
        let radius = t.corner_radius;
        let shadow_i = t.shadow_intensity;
        let border_color = Color {
            a: 0.15,
            ..t.accent
        };

        container(content)
            .width(WINDOW_WIDTH)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: radius.into(),
                    width: 1.0,
                    color: border_color,
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.6 * shadow_i),
                    offset: Vector::new(0.0, 8.0),
                    blur_radius: 32.0,
                },
                text_color: None,
                snap: false,
            })
            .into()
    }

    fn view_search_bar(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let text_color = t.text;
        let overlay_color = t.overlay;
        let accent_color = t.accent;
        let surface = t.surface;
        let has_results = !self.results.is_empty();
        let radius: f32 = if has_results { 0.0 } else { t.corner_radius };

        let brand = text("\u{00BB}").size(24).color(t.peach);

        // Change placeholder while loading to give user feedback.
        let placeholder = if self.loading {
            "Loading..."
        } else {
            "Search anything..."
        };

        let input = text_input(placeholder, &self.query)
            .id(self.input_id.clone())
            .on_input(Message::QueryChanged)
            .on_submit(Message::Submit)
            .size(18)
            .padding(0)
            .style(move |_theme, _status| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
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

        let bar_content = row![brand, input, Space::new().width(Fill), badge]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([0, 16]));

        // ── Depth layering ────────────────────────────────────────────────
        //
        // 1. Top highlight — 1px semi-transparent white line (light reflection)
        // 2. Main surface — slightly brighter than results area
        // 3. Bottom shadow — 1px darker line (cast shadow from raised surface)
        // 4. Subtle border glow — thin accent-tinted border
        //
        // Together these create a "raised card" 3D effect.

        let highlight_color = Color::from_rgba(1.0, 1.0, 1.0, 0.06);
        let shadow_line_color = Color::from_rgba(0.0, 0.0, 0.0, 0.3);
        let border_glow = Color {
            a: 0.12,
            ..accent_color
        };

        // Brighter surface for the search bar (elevated feel)
        let bar_surface = Color {
            r: (surface.r + 0.03).min(1.0),
            g: (surface.g + 0.03).min(1.0),
            b: (surface.b + 0.03).min(1.0),
            a: surface.a,
        };

        // Top highlight (1px)
        let top_highlight = container(text(""))
            .width(Fill)
            .height(1)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(highlight_color)),
                ..Default::default()
            });

        // Main content area
        let main_bar = container(bar_content)
            .width(Fill)
            .height(SEARCH_BAR_HEIGHT - 2.0) // account for top highlight + bottom shadow
            .center_y(Fill);

        // Bottom shadow (1px, only when expanded)
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

        let layered = column![top_highlight, main_bar, bottom_shadow];

        container(layered)
            .width(Fill)
            .height(SEARCH_BAR_HEIGHT)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(bar_surface)),
                border: Border {
                    radius: radius.into(),
                    width: 1.0,
                    color: border_glow,
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                    offset: Vector::new(0.0, 2.0),
                    blur_radius: 8.0,
                },
                text_color: None,
                snap: false,
            })
            .into()
    }

    fn view_results_list(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let mut list = Column::new().spacing(0);
        for (i, result) in self.results.iter().take(MAX_VISIBLE_ROWS).enumerate() {
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
        let kind_label = format!("{}", item.kind);
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
                border: Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
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
