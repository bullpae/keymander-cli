//! TUI application state and main event loop

use std::path::{Path, PathBuf};

use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, KeyCode, KeyModifiers,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use kmd_core::action;
use kmd_core::hangul::{self, HangulComposer};
use kmd_core::index::{files::icon_for_path, ItemKind, Source};
use kmd_core::plugin::builtin_calc;
use kmd_core::search::{SearchEngine, SearchMode, SearchResult};
use kmd_core::web;

use super::event::{AppEvent, EventHandler};
use super::settings::{self, SettingsAction, SettingsState};
use super::theme::Theme;
use super::ui;

// ── Constants ────────────────────────────────────────────────────────────────

/// Max results returned by fuzzy search
const SEARCH_RESULT_LIMIT: usize = 50;

/// Max history entries shown when query is empty
const HISTORY_DISPLAY_LIMIT: usize = 20;

/// Score for web service list items (@ prefix browsing)
const SCORE_WEB_LIST: u32 = 0;

/// Score for a specific web search result
const SCORE_WEB_SEARCH: u32 = 100;

/// Score for explicit calculator results (:calc prefix)
const SCORE_CALC: u32 = 1000;

/// Score for inline calculator results (always on top)
const SCORE_CALC_INLINE: u32 = u32::MAX;

/// Score for directory listing items
const SCORE_DIR_LISTING: u32 = 0;

/// Multiplier for history frequency → score
const HISTORY_SCORE_MULTIPLIER: u32 = 100;

// ── Application State ────────────────────────────────────────────────────────

/// Application state
pub struct AppState {
    /// Current search query (committed text only)
    pub query: String,
    /// Search results
    pub results: Vec<SearchResult>,
    /// Currently selected index
    pub selected_index: usize,
    /// Total indexed items
    pub total_items: usize,
    /// Show preview panel
    pub show_preview: bool,
    /// Current search mode
    search_mode: SearchMode,
    /// Whether to quit
    should_quit: bool,
    /// Whether to quit after launch
    quit_on_launch: bool,
    /// Korean (Hangul) input mode
    pub hangul_mode: bool,
    /// Currently composing character (during Korean input)
    pub composing: Option<char>,
    /// Hangul composition engine
    composer: HangulComposer,
    /// Folder drill-down stack
    drill_stack: Vec<DrillState>,
    /// Current drill-down directory path (None = normal search mode)
    pub drill_path: Option<PathBuf>,
    /// Temporary status message (e.g. "Copied to clipboard")
    pub status_message: Option<String>,
    /// Settings modal (None = hidden, Some = active)
    pub settings: Option<SettingsState>,
}

/// Saved state for returning from a folder drill-down
struct DrillState {
    query: String,
    results: Vec<SearchResult>,
    selected_index: usize,
    search_mode: SearchMode,
    parent_drill_path: Option<PathBuf>,
}

impl AppState {
    pub fn search_mode_label(&self) -> &str {
        self.search_mode.label()
    }

    /// Get the effective query for display and search (query + composing char)
    pub fn effective_query(&self) -> String {
        match self.composing {
            Some(c) => format!("{}{}", self.query, c),
            None => self.query.clone(),
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Wrap items into SearchResults with a uniform score
fn items_to_results(
    items: impl IntoIterator<Item = kmd_core::IndexItem>,
    score: u32,
) -> Vec<SearchResult> {
    items
        .into_iter()
        .map(|item| SearchResult { item, score })
        .collect()
}

// ── Main Loop ────────────────────────────────────────────────────────────────

/// Run the TUI application
pub fn run_app() -> color_eyre::Result<()> {
    // Load config and build index
    let mut config = crate::cmd::load_config()?;
    let index = crate::cmd::load_or_build_index(&config.launcher);
    let db = crate::cmd::open_db().ok();

    // Initialize search engine with kind weights
    let mut engine = SearchEngine::new();
    engine.set_kind_weights(config.launcher.kind_weights.clone());
    let total_items = index.items.len();
    engine.load(index.items);

    // Initialize state
    let mut state = AppState {
        query: String::new(),
        results: Vec::new(),
        selected_index: 0,
        total_items,
        show_preview: config.general.show_preview,
        search_mode: SearchMode::Fuzzy,
        should_quit: false,
        quit_on_launch: config.launcher.quit_on_launch,
        hangul_mode: false,
        composing: None,
        composer: HangulComposer::new(),
        drill_stack: Vec::new(),
        drill_path: None,
        status_message: None,
        settings: None,
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let theme = Theme::default();
    let events = EventHandler::new(config.general.render_fps);

    // Initial empty results: show history
    if let Some(ref db) = db {
        load_history_into_results(&mut state, db);
    }

    // Main loop
    loop {
        terminal.draw(|frame| {
            ui::render(frame, &state, &theme);
            // Render settings modal overlay on top
            if let Some(ref settings_state) = state.settings {
                settings::render::render_modal(frame, frame.area(), settings_state, &theme);
            }
        })?;

        if state.should_quit {
            break;
        }

        match events.next()? {
            AppEvent::Key(key) => {
                // Route keys to settings if modal is open
                if state.settings.is_some() {
                    handle_settings_key_event(
                        &mut state,
                        key,
                        &mut config,
                        &mut engine,
                    );
                } else {
                    handle_key(&mut state, key, &mut engine, db.as_ref());
                }
            }
            AppEvent::Paste(text) => {
                if state.settings.is_none() {
                    handle_paste(&mut state, &text, &mut engine, db.as_ref());
                }
            }
            AppEvent::Resize | AppEvent::Tick => {}
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    Ok(())
}

// ── Settings Integration ─────────────────────────────────────────────────────

/// Handle a key event when the settings modal is open
fn handle_settings_key_event(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    config: &mut kmd_core::Config,
    engine: &mut SearchEngine,
) {
    // Take ownership of settings state temporarily
    let Some(mut settings_state) = state.settings.take() else {
        return;
    };

    let action = settings::handle_settings_key(&mut settings_state, key);

    match action {
        SettingsAction::None => {
            // Put it back
            state.settings = Some(settings_state);
        }
        SettingsAction::Close => {
            // Discard unsaved changes, close modal
            state.settings = None;
        }
        SettingsAction::Save { needs_rebuild } => {
            // Apply the edited config
            *config = settings_state.config.clone();
            settings_state.dirty = false;

            // Save to file
            if let Err(e) = config.save() {
                state.status_message =
                    Some(format!("\u{274C} Save failed: {}", e));
                state.settings = Some(settings_state);
                return;
            }

            // Apply immediate settings
            state.show_preview = config.general.show_preview;
            state.quit_on_launch = config.launcher.quit_on_launch;
            engine.set_kind_weights(config.launcher.kind_weights.clone());

            if needs_rebuild {
                // Rebuild index with new config
                let index = crate::cmd::load_or_build_index(&config.launcher);
                state.total_items = index.items.len();
                engine.load(index.items);

                // Delete old cache so it's rebuilt next time
                let cache_path = crate::cmd::index_cache_path();
                let _ = std::fs::remove_file(&cache_path);
                let index = kmd_core::Index::build(&config.launcher);
                let _ = kmd_core::index::store::save_index(&index, &cache_path);
                state.total_items = index.items.len();
                engine.load(index.items);

                state.status_message = Some(format!(
                    "\u{2705} Settings saved. Index rebuilt ({} items)",
                    state.total_items
                ));
            } else {
                state.status_message =
                    Some("\u{2705} Settings saved".to_string());
            }

            // Close modal after save
            state.settings = None;
        }
        SettingsAction::Reset => {
            // Reset to defaults
            let default_config = kmd_core::Config::default();
            settings_state.config.launcher.kind_weights =
                default_config.launcher.kind_weights;
            settings_state.config.launcher.search_depth =
                default_config.launcher.search_depth;
            settings_state.config.launcher.max_results =
                default_config.launcher.max_results;
            settings_state.config.launcher.ignore_patterns =
                default_config.launcher.ignore_patterns;
            settings_state.config.launcher.index_directories =
                default_config.launcher.index_directories;
            settings_state.config.launcher.file_search_provider =
                default_config.launcher.file_search_provider;
            settings_state.config.general = default_config.general;
            settings_state.config.keybindings = default_config.keybindings;
            settings_state.dirty = true;
            state.settings = Some(settings_state);
        }
    }
}

// ── Key Handling ─────────────────────────────────────────────────────────────

/// Handle a key event
fn handle_key(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    state.status_message = None;

    match (key.code, key.modifiers) {
        // Open settings modal
        (KeyCode::F(2), _) => {
            flush_composer(state);
            let config = crate::cmd::load_config().unwrap_or_default();
            state.settings = Some(SettingsState::new(config));
        }

        (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
            flush_composer(state);
            state.hangul_mode = !state.hangul_mode;
            update_search(state, engine, db);
        }
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            state.should_quit = true;
        }
        (KeyCode::Esc, _) => handle_escape(state, db),
        (KeyCode::Up, _) => {
            flush_composer(state);
            if state.selected_index > 0 {
                state.selected_index -= 1;
            }
        }
        (KeyCode::Down, _) => {
            flush_composer(state);
            if state.selected_index + 1 < state.results.len() {
                state.selected_index += 1;
            }
        }
        (KeyCode::Tab, _) | (KeyCode::Right, _) => {
            flush_composer(state);
            drill_into_folder(state);
        }
        (KeyCode::Left, _) => {
            flush_composer(state);
            if state.drill_path.is_some() {
                drill_back(state);
            }
        }
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            flush_composer(state);
            state.show_preview = !state.show_preview;
        }
        (KeyCode::Enter, _) => {
            flush_composer(state);
            update_search(state, engine, db);
            execute_selected(state, db);
        }
        (KeyCode::Backspace, _) => {
            if state.hangul_mode && state.composer.is_composing() {
                state.composer.backspace();
                state.composing = state.composer.composing();
            } else {
                state.query.pop();
            }
            update_search(state, engine, db);
        }
        (KeyCode::Char(c), mods) => {
            if mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT) {
                return;
            }
            if state.hangul_mode {
                handle_hangul_char(state, c, engine, db);
            } else {
                state.query.push(c);
                update_search(state, engine, db);
            }
        }
        _ => {}
    }
}

/// Handle Escape: exit drill-down → clear query → quit
fn handle_escape(state: &mut AppState, db: Option<&kmd_core::Database>) {
    flush_composer(state);
    if state.drill_path.is_some() {
        drill_back(state);
    } else if state.query.is_empty() {
        state.should_quit = true;
    } else {
        state.query.clear();
        state.results.clear();
        state.selected_index = 0;
        if let Some(db) = db {
            load_history_into_results(state, db);
        }
    }
}

// ── Korean Input ─────────────────────────────────────────────────────────────

/// Handle a character in Korean input mode
fn handle_hangul_char(
    state: &mut AppState,
    c: char,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    if hangul::is_korean_char(c) {
        flush_composer(state);
        state.query.push(c);
    } else if let Some(jamo) = hangul::key_to_jamo(c) {
        let result = state.composer.process(jamo);
        if let Some(committed) = result.committed {
            state.query.push(committed);
        }
        state.composing = result.composing;
    } else {
        flush_composer(state);
        state.query.push(c);
    }
    update_search(state, engine, db);
}

/// Flush the Hangul composer — commit any composing character to the query
fn flush_composer(state: &mut AppState) {
    if let Some(committed) = state.composer.flush() {
        state.query.push(committed);
    }
    state.composing = None;
}

/// Handle pasted text (clipboard paste or IME-composed text)
fn handle_paste(
    state: &mut AppState,
    text: &str,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    flush_composer(state);
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    if clean.is_empty() {
        return;
    }
    state.query.push_str(&clean);
    update_search(state, engine, db);
}

// ── Execute ──────────────────────────────────────────────────────────────────

/// Execute the currently selected item
fn execute_selected(state: &mut AppState, db: Option<&kmd_core::Database>) {
    let Some(result) = state.results.get(state.selected_index) else {
        return;
    };

    // Calculator result → copy to clipboard
    if result.item.kind == ItemKind::Calculator && !result.item.path.is_empty() {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&result.item.path);
            state.status_message = Some(format!("\u{1F4CB} Copied: {}", result.item.path));
        }
        return;
    }

    // Web query
    if let Some((service, web_query)) = web::parse_web_query(&state.query) {
        if !web_query.is_empty() {
            let url = web::build_search_url(service, &web_query);
            let _ = action::open_url(&url);
            if state.quit_on_launch {
                state.should_quit = true;
            }
            return;
        }
    }

    // URL mode
    let (mode, normalized) = SearchMode::detect(&state.query);
    if mode == SearchMode::Url {
        let _ = action::open_url(&normalized);
        if state.quit_on_launch {
            state.should_quit = true;
        }
        return;
    }

    // Normal execution
    match action::execute(result) {
        action::ActionResult::Launched => {
            if let Some(db) = db {
                kmd_core::history::record_launch(
                    db,
                    &result.item.kind.to_string(),
                    &result.item.path,
                    Some(&result.item.name),
                );
            }
            if state.quit_on_launch {
                state.should_quit = true;
            }
        }
        action::ActionResult::OpenedUrl(_) => {
            if state.quit_on_launch {
                state.should_quit = true;
            }
        }
        action::ActionResult::NeedsConfirmation(name) => {
            state.status_message =
                Some(format!("\u{26A0}\u{FE0F} Confirmation needed: {}", name));
        }
        action::ActionResult::Error(e) => {
            state.status_message = Some(format!("\u{274C} Error: {}", e));
        }
    }
}

// ── Search ───────────────────────────────────────────────────────────────────

/// Update search results based on current query (including composing char)
fn update_search(
    state: &mut AppState,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    let query = state.effective_query();

    if query.is_empty() {
        return handle_empty_query(state, db);
    }

    // Typing while in drill mode exits drill and does a full search
    if state.drill_path.is_some() {
        state.drill_path = None;
        state.drill_stack.clear();
    }

    if query.starts_with('@') {
        return handle_web_query(&query, state);
    }

    if query.starts_with(":calc") {
        return handle_calc_query(&query, state);
    }

    handle_main_search(&query, state, engine, db);
}

/// Empty query: show drill directory contents or recent history
fn handle_empty_query(state: &mut AppState, db: Option<&kmd_core::Database>) {
    state.results.clear();
    state.selected_index = 0;

    if let Some(ref path) = state.drill_path {
        state.results = list_directory_contents(path);
    } else if let Some(db) = db {
        load_history_into_results(state, db);
    }
}

/// Handle @prefix web service queries
fn handle_web_query(query: &str, state: &mut AppState) {
    if let Some((service, q)) = web::parse_web_query(query) {
        if q.is_empty() {
            state.results = items_to_results(web::list_services_as_items(""), SCORE_WEB_LIST);
        } else {
            let item = web::search_result_item(service, &q);
            state.results = items_to_results(std::iter::once(item), SCORE_WEB_SEARCH);
        }
    } else {
        let filter = query.trim_start_matches('@');
        state.results = items_to_results(web::list_services_as_items(filter), SCORE_WEB_LIST);
    }
    state.search_mode = SearchMode::Contains;
    state.selected_index = 0;
}

/// Handle :calc prefix (explicit calculator mode)
fn handle_calc_query(query: &str, state: &mut AppState) {
    let expr = query.strip_prefix(":calc").unwrap_or("").trim();
    let calc = builtin_calc::CalcExtension;
    let items =
        <builtin_calc::CalcExtension as kmd_core::plugin::Extension>::search(&calc, expr);
    state.results = items_to_results(items, SCORE_CALC);
    state.selected_index = 0;
}

/// Main fuzzy search with optional inline calculator and history boost
fn handle_main_search(
    query: &str,
    state: &mut AppState,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    let (mode, mut results) = engine.search(query, SEARCH_RESULT_LIMIT);
    state.search_mode = mode;

    // Inline calculator: prepend result if query looks like math
    if builtin_calc::looks_like_math(query) {
        let calc = builtin_calc::CalcExtension;
        let calc_items =
            <builtin_calc::CalcExtension as kmd_core::plugin::Extension>::search(&calc, query);
        let calc_results = items_to_results(calc_items, SCORE_CALC_INLINE);
        results.splice(0..0, calc_results);
    }

    // Apply history boost
    if let Some(db) = db {
        kmd_core::history::boost_results(&mut results, db);
    }

    state.results = results;
    state.selected_index = 0;
}

// ── Drill-Down ───────────────────────────────────────────────────────────────

/// Drill into the selected folder — list its contents
fn drill_into_folder(state: &mut AppState) {
    let Some(selected) = state.results.get(state.selected_index) else {
        return;
    };

    if selected.item.kind != ItemKind::Directory {
        return;
    }

    let dir_path = PathBuf::from(&selected.item.path);
    if !dir_path.is_dir() {
        return;
    }

    // Save current state to the drill stack
    state.drill_stack.push(DrillState {
        query: state.query.clone(),
        results: state.results.clone(),
        selected_index: state.selected_index,
        search_mode: state.search_mode,
        parent_drill_path: state.drill_path.clone(),
    });

    state.results = list_directory_contents(&dir_path);
    state.selected_index = 0;
    state.drill_path = Some(dir_path);
    state.search_mode = SearchMode::Contains;
    state.query.clear();
}

/// Go back from drill-down to the previous state
fn drill_back(state: &mut AppState) {
    if let Some(prev) = state.drill_stack.pop() {
        state.query = prev.query;
        state.results = prev.results;
        state.selected_index = prev.selected_index.min(state.results.len().saturating_sub(1));
        state.search_mode = prev.search_mode;
        state.drill_path = prev.parent_drill_path;
    }
}

/// List contents of a directory as SearchResults, sorted: directories first, then files
fn list_directory_contents(dir: &Path) -> Vec<SearchResult> {
    let mut directories = Vec::new();
    let mut files = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/dirs
        if name.starts_with('.') {
            continue;
        }

        let is_dir = path.is_dir();
        let path_str = path.to_string_lossy().to_string();

        let item = kmd_core::IndexItem {
            name,
            path: path_str,
            kind: if is_dir {
                ItemKind::Directory
            } else {
                ItemKind::File
            },
            source: Source::FileProvider,
            icon: if is_dir {
                "\u{1F4C1}".to_string() // 📁
            } else {
                icon_for_path(&path)
            },
            keywords: String::new(),
        };

        let result = SearchResult {
            item,
            score: SCORE_DIR_LISTING,
        };

        if is_dir {
            directories.push(result);
        } else {
            files.push(result);
        }
    }

    let sort_by_name = |a: &SearchResult, b: &SearchResult| {
        a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase())
    };
    directories.sort_by(sort_by_name);
    files.sort_by(sort_by_name);

    directories.extend(files);
    directories
}

// ── History ──────────────────────────────────────────────────────────────────

/// Load recent history into results (for empty query display)
fn load_history_into_results(state: &mut AppState, db: &kmd_core::Database) {
    let history = db.query_history(HISTORY_DISPLAY_LIMIT);
    state.results = history
        .into_iter()
        .map(|h| {
            let kind = match h.item_type.as_str() {
                "App" => ItemKind::App,
                "File" => ItemKind::File,
                "Dir" => ItemKind::Directory,
                "Exe" => ItemKind::Executable,
                "System" => ItemKind::SystemCommand,
                "Web" => ItemKind::WebSearch,
                _ => ItemKind::App,
            };
            let path_buf = PathBuf::from(&h.value);
            let icon = match kind {
                ItemKind::Directory => "\u{1F4C1}".to_string(),
                _ => icon_for_path(&path_buf),
            };
            let icon = format!("\u{1F552}{}", icon); // 🕒 + original icon
            SearchResult {
                item: kmd_core::IndexItem {
                    name: h.display,
                    path: h.value,
                    kind,
                    source: Source::FileProvider,
                    icon,
                    keywords: String::new(),
                },
                score: h.frequency * HISTORY_SCORE_MULTIPLIER,
            }
        })
        .collect();
}
