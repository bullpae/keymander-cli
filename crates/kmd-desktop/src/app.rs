//! Application state, messages, update/view/subscription — the Elm core.
//!
//! Renders a Spotlight-like floating launcher: search bar always visible,
//! results list + status bar appear only when there are results.

use iced::keyboard;
use iced::widget::{
    column, container, mouse_area, row, scrollable, text, text_input, Column, Space,
};
use iced::{
    event, window, Background, Border, Color, Element, Fill, Padding, Shadow, Size, Subscription,
    Task, Vector,
};

use crate::theme::DesktopTheme;

// ─── Constants ────────────────────────────────────────────────────────────────

const WINDOW_WIDTH: f32 = 680.0;
const SEARCH_BAR_HEIGHT: f32 = 56.0;
const ROW_HEIGHT: f32 = 52.0;
const STATUS_BAR_HEIGHT: f32 = 28.0;
const MAX_VISIBLE_ROWS: usize = 8;
const SEARCH_LIMIT: usize = 50;

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
}

// ─── Messages ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    QueryChanged(String),
    Submit,
    ResultClicked(usize),
    KeyEvent(keyboard::Key, keyboard::Modifiers),
    GotWindowId(Option<window::Id>),
    Noop,
}

// ─── Boot ─────────────────────────────────────────────────────────────────────

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let config = load_config();
        let engine = create_search_engine(&config);
        let theme = crate::theme::midnight();
        let input_id = iced::widget::Id::unique();

        let app = Self {
            query: String::new(),
            results: Vec::new(),
            search_mode: kmd_core::SearchMode::Fuzzy,
            selected: 0,
            engine,
            theme,
            input_id: input_id.clone(),
            window_id: None,
        };

        // Fetch the main window ID on startup.
        let id_task = window::oldest().map(Message::GotWindowId);
        (app, id_task)
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
                Task::none()
            }

            Message::Noop => Task::none(),
        }
    }

    // ─── View ─────────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let has_results = !self.results.is_empty();

        let search_bar = self.view_search_bar();

        let mut content = Column::new().push(search_bar);

        if has_results {
            // Separator line (1px container)
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

        // Outer container — rounded, semi-transparent, shadowed
        let bg = t.background_with_opacity();
        let radius = t.corner_radius;
        let shadow_i = t.shadow_intensity;

        container(content)
            .width(WINDOW_WIDTH)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: radius.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5 * shadow_i),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 20.0,
                },
                text_color: None,
                snap: false,
            })
            .into()
    }

    // ─── Subscription ─────────────────────────────────────────────────────

    pub fn subscription(&self) -> Subscription<Message> {
        event::listen().map(|ev| match ev {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) => match &key {
                keyboard::Key::Named(n) => match n {
                    keyboard::key::Named::ArrowUp
                    | keyboard::key::Named::ArrowDown
                    | keyboard::key::Named::Escape
                    | keyboard::key::Named::Tab => Message::KeyEvent(key, modifiers),
                    _ => Message::Noop,
                },
                _ => Message::Noop,
            },
            _ => Message::Noop,
        })
    }

    pub fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

// ─── Sub-views ────────────────────────────────────────────────────────────────

impl App {
    fn view_search_bar(&self) -> Element<'_, Message> {
        let t = &self.theme;

        // Brand mark "»"
        let brand = text("»").size(24).color(t.peach);

        // Search input
        let text_color = t.text;
        let overlay_color = t.overlay;
        let accent_color = t.accent;
        let input = text_input("Search anything...", &self.query)
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

        // Mode indicator badge
        let mode_text = if self.query.is_empty() {
            ""
        } else {
            self.search_mode.label()
        };
        let badge = text(mode_text).size(11).color(t.overlay);

        let surface = t.surface;
        let has_results = !self.results.is_empty();
        let radius: f32 = if has_results { 0.0 } else { 12.0 };

        let bar_content = row![
            brand,
            input,
            Space::new().width(Fill),
            badge
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding(Padding::from([0, 16]));

        container(bar_content)
            .width(Fill)
            .height(SEARCH_BAR_HEIGHT)
            .center_y(Fill)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(surface)),
                border: Border {
                    radius: radius.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow::default(),
                text_color: None,
                snap: false,
            })
            .into()
    }

    fn view_results_list(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let visible = self.results.iter().take(MAX_VISIBLE_ROWS).enumerate();

        let mut list = Column::new().spacing(0);
        for (i, result) in visible {
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

    fn view_result_row<'a>(&'a self, index: usize, result: &'a kmd_core::SearchResult) -> Element<'a, Message> {
        let t = &self.theme;
        let is_selected = index == self.selected;
        let item = &result.item;

        // Left accent bar (selection indicator)
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

        // Icon
        let icon = text(&item.icon).size(22);

        // Title + Subtitle
        let title = text(&item.name).size(14).color(t.text);
        let subtitle = text(&item.path).size(11).color(t.subtext);
        let info = column![title, subtitle].spacing(2);

        // Kind badge (pill)
        let kind_color = t.kind_color(item.kind);
        let kind_label = format!("{}", item.kind);
        let badge_bg = Color {
            a: 0.12,
            ..kind_color
        };
        let badge_border_color = Color {
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
                    color: badge_border_color,
                },
                ..Default::default()
            });

        // Row background
        let bg = if is_selected {
            t.surface2
        } else {
            Color::TRANSPARENT
        };

        let row_content = row![
            left_bar,
            icon,
            info,
            Space::new().width(Fill),
            badge
        ]
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
        let count = self.results.len();
        let mode_label = self.search_mode.label();
        let status_text = format!("{}  ·  {} results", mode_label, count);

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

// ─── Logic ────────────────────────────────────────────────────────────────────

impl App {
    fn perform_search(&mut self) -> Task<Message> {
        if self.query.trim().is_empty() {
            self.results.clear();
            self.search_mode = kmd_core::SearchMode::Fuzzy;
        } else {
            let (mode, results) = self.engine.search(&self.query, SEARCH_LIMIT);
            self.search_mode = mode;
            self.results = results;
        }
        self.resize_window()
    }

    fn launch_selected(&mut self) -> Task<Message> {
        if let Some(result) = self.results.get(self.selected) {
            kmd_core::action::execute(result);
            self.query.clear();
            self.results.clear();
            self.selected = 0;
            return self.resize_window();
        }
        Task::none()
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

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn load_config() -> kmd_core::Config {
    let config_dir = kmd_core::Config::default_config_dir();
    kmd_core::Config::load(&config_dir).unwrap_or_default()
}

fn create_search_engine(config: &kmd_core::Config) -> kmd_core::SearchEngine {
    let data_dir = kmd_core::Config::default_data_dir();
    let cache_path = data_dir.join(kmd_core::INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    let index = if cache_path.exists() {
        match kmd_core::index::store::load_index(&cache_path) {
            Ok(cached) if cached.version == expected_version => cached,
            _ => {
                let idx = kmd_core::Index::build(&config.launcher, config.general.emoji_icons);
                let _ = kmd_core::index::store::save_index(&idx, &cache_path);
                idx
            }
        }
    } else {
        let idx = kmd_core::Index::build(&config.launcher, config.general.emoji_icons);
        let _ = kmd_core::index::store::save_index(&idx, &cache_path);
        idx
    };

    tracing::info!("Loaded {} items into search engine", index.items.len());

    let mut engine = kmd_core::SearchEngine::new();
    engine.set_kind_weights(config.launcher.kind_weights.clone());
    engine.load(index.items);
    engine
}
