//! TUI application state and main event loop

use std::path::PathBuf;

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
use kmd_core::index::{ItemKind, Source, files::icon_for_path};
use kmd_core::search::{SearchEngine, SearchMode, SearchResult};
use kmd_core::web;

use super::event::{AppEvent, EventHandler};
use super::theme::Theme;
use super::ui;

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
    /// Folder drill-down stack: previous (query, results, selected_index) states
    drill_stack: Vec<DrillState>,
    /// Current drill-down directory path (None = normal search mode)
    pub drill_path: Option<PathBuf>,
}

/// Saved state for returning from a folder drill-down
struct DrillState {
    query: String,
    results: Vec<SearchResult>,
    selected_index: usize,
    search_mode: SearchMode,
    /// The drill path before entering this level (None for the first drill)
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

/// Run the TUI application
pub async fn run_app() -> color_eyre::Result<()> {
    // Load config and build index
    let config = crate::cmd::load_config()?;
    let index = crate::cmd::load_or_build_index(&config.launcher);
    let db = crate::cmd::open_db().ok();

    // Initialize search engine
    let mut engine = SearchEngine::new();
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
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let theme = Theme::default_theme();
    let events = EventHandler::new(config.general.render_fps);

    // Initial empty results: show history
    if let Some(ref db) = db {
        load_history_into_results(&mut state, db);
    }

    // Main loop
    loop {
        // Render
        terminal.draw(|frame| {
            ui::render(frame, &state, &theme);
        })?;

        if state.should_quit {
            break;
        }

        // Handle events
        match events.next()? {
            AppEvent::Key(key) => {
                handle_key(&mut state, key, &mut engine, db.as_ref());
            }
            AppEvent::Paste(text) => {
                handle_paste(&mut state, &text, &mut engine, db.as_ref());
            }
            AppEvent::Resize(_, _) => {
                // Terminal will re-render automatically
            }
            AppEvent::Tick => {
                // Nothing to do on tick
            }
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

/// Handle a key event
fn handle_key(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    match (key.code, key.modifiers) {
        // ── Toggle Korean mode: Ctrl+Space or Right Alt ──
        (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
            // Flush any composing character before toggling
            flush_composer(state);
            state.hangul_mode = !state.hangul_mode;
            update_search(state, engine, db);
        }

        // ── Quit ──
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            state.should_quit = true;
        }

        // ── Escape: exit drill-down → clear query → quit ──
        (KeyCode::Esc, _) => {
            flush_composer(state);
            if state.drill_path.is_some() {
                // Exit drill-down mode first
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

        // ── Navigate up ──
        (KeyCode::Up, _) => {
            flush_composer(state);
            if state.selected_index > 0 {
                state.selected_index -= 1;
            }
        }

        // ── Navigate down ──
        (KeyCode::Down, _) => {
            flush_composer(state);
            if state.selected_index + 1 < state.results.len() {
                state.selected_index += 1;
            }
        }

        // ── Drill into folder: Tab or Right arrow ──
        (KeyCode::Tab, _) | (KeyCode::Right, _) => {
            flush_composer(state);
            drill_into_folder(state);
        }

        // ── Drill back: Left arrow ──
        (KeyCode::Left, _) => {
            flush_composer(state);
            if state.drill_path.is_some() {
                drill_back(state);
            }
        }

        // ── Toggle preview ──
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            flush_composer(state);
            state.show_preview = !state.show_preview;
        }

        // ── Execute selected item ──
        (KeyCode::Enter, _) => {
            flush_composer(state);
            update_search(state, engine, db);
            execute_selected(state, db);
        }

        // ── Backspace ──
        (KeyCode::Backspace, _) => {
            if state.hangul_mode && state.composer.is_composing() {
                // Decompose within the composing character
                state.composer.backspace();
                state.composing = state.composer.composing();
                update_search(state, engine, db);
            } else {
                // Normal backspace: remove last committed character
                state.query.pop();
                update_search(state, engine, db);
            }
        }

        // ── Character input ──
        (KeyCode::Char(c), mods) => {
            // Skip Ctrl+key and Alt+key (except for toggle handled above)
            if mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT) {
                return;
            }

            if state.hangul_mode {
                handle_hangul_char(state, c, engine, db);
            } else {
                // English mode: if the system IME sent a Korean char, accept it
                if hangul::is_korean_char(c) {
                    state.query.push(c);
                } else {
                    state.query.push(c);
                }
                update_search(state, engine, db);
            }
        }

        _ => {}
    }
}

/// Handle a character in Korean input mode
fn handle_hangul_char(
    state: &mut AppState,
    c: char,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    // If the character is already Korean (system IME composed it), pass through
    if hangul::is_korean_char(c) {
        flush_composer(state);
        state.query.push(c);
        update_search(state, engine, db);
        return;
    }

    // Try mapping the key to a jamo
    if let Some(jamo) = hangul::key_to_jamo(c) {
        let result = state.composer.process(jamo);
        if let Some(committed) = result.committed {
            state.query.push(committed);
        }
        state.composing = result.composing;
        update_search(state, engine, db);
    } else {
        // Not a jamo key (number, symbol, space, etc.)
        // Flush the composer and add the character as-is
        flush_composer(state);
        state.query.push(c);
        update_search(state, engine, db);
    }
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
    // Flush any composing character first
    flush_composer(state);

    // Filter out control characters but keep all Unicode (including Korean, CJK, emoji)
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    if clean.is_empty() {
        return;
    }
    state.query.push_str(&clean);
    update_search(state, engine, db);
}

/// Execute the currently selected item
fn execute_selected(state: &mut AppState, db: Option<&kmd_core::Database>) {
    if let Some(result) = state.results.get(state.selected_index) {
        // Check for web query
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

        // Check for URL mode
        let (mode, normalized) = SearchMode::detect(&state.query);
        if mode == SearchMode::Url {
            let _ = action::open_url(&normalized);
            if state.quit_on_launch {
                state.should_quit = true;
            }
            return;
        }

        // Execute the item
        match action::execute(result) {
            action::ActionResult::Launched => {
                if let Some(db) = db {
                    kmd_core::history::record_launch(
                        db,
                        &format!("{}", result.item.kind),
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
            action::ActionResult::NeedsConfirmation(_name) => {
                // TODO: show confirmation dialog
            }
            action::ActionResult::Error(_e) => {
                // TODO: show error toast
            }
        }
    }
}

/// Update search results based on current query (including composing char)
fn update_search(
    state: &mut AppState,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    let search_query = state.effective_query();

    if search_query.is_empty() {
        state.results.clear();
        state.selected_index = 0;

        // Show history when query is empty
        if let Some(db) = db {
            load_history_into_results(state, db);
        }
        return;
    }

    // Check for @ web service prefix
    if search_query.starts_with('@') {
        if let Some((service, query)) = web::parse_web_query(&search_query) {
            if query.is_empty() {
                let items = web::list_services_as_items("");
                state.results = items
                    .into_iter()
                    .map(|item| SearchResult { item, score: 0 })
                    .collect();
            } else {
                let item = web::search_result_item(service, &query);
                state.results = vec![SearchResult { item, score: 100 }];
            }
            state.search_mode = SearchMode::Contains;
        } else {
            let filter = search_query.trim_start_matches('@');
            let items = web::list_services_as_items(filter);
            state.results = items
                .into_iter()
                .map(|item| SearchResult { item, score: 0 })
                .collect();
            state.search_mode = SearchMode::Contains;
        }
        state.selected_index = 0;
        return;
    }

    let (mode, mut results) = engine.search(&search_query, 50);
    state.search_mode = mode;

    // Apply history boost
    if let Some(db) = db {
        kmd_core::history::boost_results(&mut results, db);
    }

    state.results = results;
    state.selected_index = 0;
}

/// Drill into the selected folder — list its contents
fn drill_into_folder(state: &mut AppState) {
    let selected = match state.results.get(state.selected_index) {
        Some(r) => r,
        None => return,
    };

    // Only drill into directories
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

    // List directory contents
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
        state.selected_index = prev.selected_index;
        state.search_mode = prev.search_mode;
        state.drill_path = prev.parent_drill_path;
    }
}

/// List contents of a directory as SearchResults, sorted: directories first, then files
fn list_directory_contents(dir: &PathBuf) -> Vec<SearchResult> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry
            .file_name()
            .to_string_lossy()
            .to_string();

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

        let result = SearchResult { item, score: 0 };

        if is_dir {
            dirs.push(result);
        } else {
            files.push(result);
        }
    }

    // Sort each group by name
    dirs.sort_by(|a, b| a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase()));
    files.sort_by(|a, b| a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase()));

    // Directories first, then files
    dirs.extend(files);
    dirs
}

/// Load recent history into results (for empty query display)
fn load_history_into_results(state: &mut AppState, db: &kmd_core::Database) {
    let history = db.query_history(20);
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
                ItemKind::Directory => "\u{1F4C1}".to_string(), // 📁
                _ => icon_for_path(&path_buf),
            };
            // Prepend clock emoji to indicate history
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
                score: h.frequency * 100,
            }
        })
        .collect();
}
