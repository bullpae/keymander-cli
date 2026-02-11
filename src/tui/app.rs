//! TUI application state and main event loop

use crossterm::event::{KeyCode, KeyModifiers, EnableBracketedPaste, DisableBracketedPaste};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use kmd_core::action;
use kmd_core::search::{SearchEngine, SearchMode, SearchResult};
use kmd_core::web;

use super::event::{AppEvent, EventHandler};
use super::theme::Theme;
use super::ui;

/// Application state
pub struct AppState {
    /// Current search query
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
}

impl AppState {
    pub fn search_mode_label(&self) -> &str {
        self.search_mode.label()
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
        let history = db.query_history(20);
        state.results = history
            .into_iter()
            .map(|h| SearchResult {
                item: kmd_core::index::IndexItem {
                    name: h.display,
                    path: h.value,
                    kind: kmd_core::index::ItemKind::App,
                    source: kmd_core::index::Source::Path,
                    icon: "\u{1F552}".to_string(), // 🕒
                    keywords: String::new(),
                },
                score: h.frequency * 100,
            })
            .collect();
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
                // Handle pasted text and IME-composed input.
                // On some terminals/OS combos, Korean IME commits arrive as Paste events
                // when BracketedPaste is enabled.
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
        // Quit
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            state.should_quit = true;
        }
        // Escape: clear query or quit
        (KeyCode::Esc, _) => {
            if state.query.is_empty() {
                state.should_quit = true;
            } else {
                state.query.clear();
                state.results.clear();
                state.selected_index = 0;

                // Show history when query is empty
                if let Some(db) = db {
                    let history = db.query_history(20);
                    state.results = history
                        .into_iter()
                        .map(|h| SearchResult {
                            item: kmd_core::index::IndexItem {
                                name: h.display,
                                path: h.value,
                                kind: kmd_core::index::ItemKind::App,
                                source: kmd_core::index::Source::Path,
                                icon: "\u{1F552}".to_string(),
                                keywords: String::new(),
                            },
                            score: h.frequency * 100,
                        })
                        .collect();
                }
            }
        }
        // Navigate up
        (KeyCode::Up, _) => {
            if state.selected_index > 0 {
                state.selected_index -= 1;
            }
        }
        // Navigate down
        (KeyCode::Down, _) => {
            if state.selected_index + 1 < state.results.len() {
                state.selected_index += 1;
            }
        }
        // Toggle preview
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            state.show_preview = !state.show_preview;
        }
        // Execute selected item
        (KeyCode::Enter, _) => {
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
        // Backspace
        (KeyCode::Backspace, _) => {
            state.query.pop();
            update_search(state, engine, db);
        }
        // Character input — accept with any modifier combination.
        // Korean IME-committed chars may arrive with unexpected modifier flags
        // depending on terminal emulator.
        (KeyCode::Char(c), mods) => {
            // Skip if Ctrl is held (except Shift+Ctrl for some edge cases)
            // to avoid capturing Ctrl+A, Ctrl+E etc. as text input.
            if mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT) {
                return;
            }
            state.query.push(c);
            update_search(state, engine, db);
        }
        _ => {}
    }
}

/// Handle pasted text (clipboard paste or IME-composed text)
fn handle_paste(
    state: &mut AppState,
    text: &str,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    // Filter out control characters but keep all Unicode (including Korean, CJK, emoji)
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    if clean.is_empty() {
        return;
    }
    state.query.push_str(&clean);
    update_search(state, engine, db);
}

/// Update search results based on current query
fn update_search(
    state: &mut AppState,
    engine: &mut SearchEngine,
    db: Option<&kmd_core::Database>,
) {
    if state.query.is_empty() {
        state.results.clear();
        state.selected_index = 0;

        // Show history when query is empty
        if let Some(db) = db {
            let history = db.query_history(20);
            state.results = history
                .into_iter()
                .map(|h| SearchResult {
                    item: kmd_core::index::IndexItem {
                        name: h.display,
                        path: h.value,
                        kind: kmd_core::index::ItemKind::App,
                        source: kmd_core::index::Source::Path,
                        icon: "\u{1F552}".to_string(),
                        keywords: String::new(),
                    },
                    score: h.frequency * 100,
                })
                .collect();
        }
        return;
    }

    // Check for @ web service prefix
    if state.query.starts_with('@') {
        if let Some((service, query)) = web::parse_web_query(&state.query) {
            if query.is_empty() {
                // Show available services
                let items = web::list_services_as_items("");
                state.results = items
                    .into_iter()
                    .map(|item| SearchResult { item, score: 0 })
                    .collect();
            } else {
                // Show search result for this service
                let item = web::search_result_item(service, &query);
                state.results = vec![SearchResult { item, score: 100 }];
            }
            state.search_mode = SearchMode::Contains;
        } else {
            // Show all services filtered
            let filter = state.query.trim_start_matches('@');
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

    let (mode, mut results) = engine.search(&state.query, 50);
    state.search_mode = mode;

    // Apply history boost
    if let Some(db) = db {
        kmd_core::history::boost_results(&mut results, db);
    }

    state.results = results;
    state.selected_index = 0;
}
