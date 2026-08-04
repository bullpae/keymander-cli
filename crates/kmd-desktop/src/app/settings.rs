//! 설정 쿼리/액션 핸들러 — :set, :help, autostart, provider 토글

use super::*;
#[allow(unused_imports)]
use super::{items_to_results, save_config};

impl App {
    fn set_clipboard(text: &str) {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Err(e) = clipboard.set_text(text.to_string()) {
                tracing::warn!("클립보드 쓰기 실패: {e}");
            }
        }
    }

    /// LLM 실행(@gpt/@llm) 라우팅. LLM 쿼리가 아니면 None을 반환해 일반 경로에 맡긴다.
    ///
    /// - 오토파일럿 켜짐 + 데몬에 잡 전송 성공: 자동화 서비스는 데몬이 키 주입,
    ///   나머지(perplexity/grok)는 여기서 URL로 연다.
    /// - 아니면 폴백: 전 서비스 URL을 열고, 붙여넣기형(gemini)이 있으면 프롬프트를
    ///   클립보드에 담아 수동 붙여넣기를 돕는다 (현행 동작).
    pub(super) fn try_llm_launch(&self) -> Option<Task<Message>> {
        let (services, prompt) = web::parse_any_llm_query(
            &self.query,
            &self.selected_llm_providers,
            &self.multi_llm_prefixes,
        )?;
        if services.is_empty() {
            return None;
        }

        let final_prompt = kmd_core::prompt::apply_template(
            &self.runtime_config.launcher.prompt_templates,
            &prompt,
        );
        let plan = web::build_llm_launch_plan(&services, &final_prompt);

        let has_paste = plan
            .jobs
            .iter()
            .any(|j| matches!(j.method, kmd_core::ipc::LlmInject::PasteEnter));

        // 오토파일럿 시도 (opt-in + 데몬 실행 필요)
        if self.runtime_config.launcher.llm_autopilot && !plan.jobs.is_empty() {
            let req = kmd_core::ipc::Request::LlmAutopilot {
                jobs: plan.jobs.clone(),
            };
            match kmd_core::ipc::send_request_result(&req) {
                Ok(_) => {
                    // 자동화 불필요 서비스만 여기서 직접 연다
                    for url in &plan.plain_urls {
                        let _ = kmd_core::action::open_url(url);
                    }
                    tracing::info!("LLM 오토파일럿 위임: {}개 잡", plan.jobs.len());
                    return Some(iced::exit());
                }
                Err(e) => {
                    tracing::warn!("오토파일럿 IPC 실패 — URL 폴백: {e}");
                }
            }
        }

        // 폴백: 전 서비스 URL 열기 + (붙여넣기형 있으면) 클립보드
        if has_paste && !final_prompt.is_empty() {
            Self::set_clipboard(&final_prompt);
        }
        for url in web::llm_plan_all_urls(&plan) {
            let _ = kmd_core::action::open_url(&url);
        }
        Some(iced::exit())
    }

    /// `@@ <프롬프트>` 이어서 질문 — 데몬에 위임. 열 URL이 없으므로 데몬
    /// 미실행/세션 없음 시엔 안내 로그만 남기고 종료(폴백 불가).
    pub(super) fn send_llm_followup(&self, prompt: &str) -> Task<Message> {
        let final_prompt = kmd_core::prompt::apply_template(
            &self.runtime_config.launcher.prompt_templates,
            prompt,
        );
        let req = kmd_core::ipc::Request::LlmFollowup {
            prompt: final_prompt,
        };
        match kmd_core::ipc::send_request_result(&req) {
            Ok(kmd_core::ipc::Response::Ok { message }) => tracing::info!("{message}"),
            Ok(kmd_core::ipc::Response::Error { message }) => tracing::warn!("{message}"),
            Ok(_) => {}
            Err(e) => tracing::warn!("이어서 질문 실패(데몬 미실행?): {e}"),
        }
        iced::exit()
    }

    pub(super) fn handle_keymap_action(
        &mut self,
        result: &kmd_core::SearchResult,
    ) -> Task<Message> {
        let keywords = &result.item.keywords;
        if keywords.ends_with(":noop") || keywords.contains(":noop:") {
            return Task::none();
        }
        if let Some(msg) =
            kmd_core::keymap::execute_keymap_action(&mut self.runtime_config, keywords)
        {
            tracing::info!("keymap action: {msg}");
        }
        let current_query = kmd_core::query_prefix::normalize_slash_command(self.query.trim())
            .unwrap_or_else(|| self.query.clone());
        self.handle_keymap_query(&current_query);
        Task::none()
    }

    pub(super) fn handle_settings_query(&mut self, query: &str) {
        let filter = match query.find(' ') {
            Some(pos) => query[pos + 1..].trim().to_lowercase(),
            None => String::new(),
        };

        let emoji = self.use_emoji;
        let current_theme = self.theme.name;
        let autostart_enabled = self.daemon_autostart_enabled;

        let ime_label = if self.reset_ime_on_launch {
            "IME: Reset to English on Launch [ON]"
        } else {
            "IME: Reset to English on Launch [OFF]"
        };
        let daemon_autostart_label = match autostart_enabled {
            Some(true) => "Daemon Auto Start [ON]",
            Some(false) => "Daemon Auto Start [OFF]",
            None => "Daemon Auto Start [UNKNOWN]",
        };
        let brand_icons_label = if self
            .runtime_config
            .general
            .brand_icons
            .eq_ignore_ascii_case("mono")
        {
            "Brand Icons: Mono (theme tint) [ON]"
        } else {
            "Brand Icons: Mono (theme tint) [OFF]"
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
                daemon_autostart_label.to_string(),
                "kmd:settings:toggle_autostart".to_string(),
                if emoji { "\u{23FB}\u{FE0F}" } else { "[BOOT]" }.to_string(),
                "Toggle daemon start at login".to_string(),
            ),
            (
                brand_icons_label.to_string(),
                "kmd:settings:toggle_brand_icons".to_string(),
                if emoji { "\u{1F5BC}" } else { "[ICO]" }.to_string(),
                "Mono glyphs vs full-color logos".to_string(),
            ),
            (
                label("Theme: Keymander (default)", "Keymander"),
                "kmd:settings:theme:keymander".to_string(),
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
                icon_path: None,
            })
            .collect();

        self.apply_contains_items(items);
    }

    pub(super) fn handle_help_query(&mut self) {
        let items = kmd_core::query_prefix::help_items(self.use_emoji);
        self.apply_contains_items(items);
    }

    pub(super) fn handle_settings_action(
        &mut self,
        result: &kmd_core::SearchResult,
    ) -> Task<Message> {
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
                // 높이 고정 플랫폼에서는 높이를 건드리지 않는다 — 여기서 접으면
                // 리사이즈 잔상이 그대로 드러난다 (app.rs FIXED_WINDOW_HEIGHT).
                // (쿼리/결과는 이 아래에서 비우므로 목표 높이는 명시적으로 정한다)
                let reset_height = if self.fixed_window_height {
                    self.ui.full_window_height
                } else {
                    self.ui.collapsed_window_height
                };
                self.window_height = reset_height;
                let run_reset = move |id: window::Id| {
                    let resize = window::resize(id, Size::new(DEFAULT_WIDTH, reset_height));
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
                self.clear_results_state(kmd_core::SearchMode::Fuzzy);

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
                self.clear_results_state(kmd_core::SearchMode::Fuzzy);
                return task;
            }
            "toggle_brand_icons" => {
                let mono = self
                    .runtime_config
                    .general
                    .brand_icons
                    .eq_ignore_ascii_case("mono");
                let new_val = if mono { "color" } else { "mono" };
                self.runtime_config.general.brand_icons = new_val.to_string();
                tracing::info!("brand_icons = {new_val}");
                save_config(|cfg| cfg.general.brand_icons = new_val.to_string());

                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return self.request_focus();
            }
            "toggle_ime_reset" => {
                self.reset_ime_on_launch = !self.reset_ime_on_launch;
                let new_val = self.reset_ime_on_launch;
                self.runtime_config.general.reset_ime_on_launch = new_val;
                tracing::info!("reset_ime_on_launch = {new_val}");
                save_config(|cfg| cfg.general.reset_ime_on_launch = new_val);

                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return self.request_focus();
            }
            "toggle_autostart" => {
                if self.daemon_autostart_toggle_in_flight {
                    return Task::none();
                }
                self.daemon_autostart_toggle_in_flight = true;
                let request = if self.daemon_autostart_enabled.unwrap_or(false) {
                    kmd_core::ipc::Request::AutostartDisable
                } else {
                    kmd_core::ipc::Request::AutostartEnable
                };
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return Task::future(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        kmd_core::ipc::send_request_result(&request)
                    })
                    .await;
                    let mapped = match result {
                        Ok(Ok(kmd_core::ipc::Response::Ok { message })) => Ok(message),
                        Ok(Ok(kmd_core::ipc::Response::Error { message })) => Err(message),
                        Ok(Ok(other)) => Err(format!("예기치 않은 응답: {other:?}")),
                        Ok(Err(e)) => Err(format!("IPC 실패: {e}")),
                        Err(e) => Err(format!("작업 실패: {e}")),
                    };
                    Message::AutostartToggleFinished(mapped)
                });
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
                    self.runtime_config.launcher.multi_llm_providers = selected.clone();
                    save_config(move |cfg| cfg.launcher.multi_llm_providers = selected);
                }

                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return self.request_focus();
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
                    self.runtime_config.launcher.multi_web_providers = selected.clone();
                    save_config(move |cfg| cfg.launcher.multi_web_providers = selected);
                }

                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return self.request_focus();
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
                    self.runtime_config.launcher.spell_providers = selected.clone();
                    save_config(move |cfg| cfg.launcher.spell_providers = selected);
                }
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return self.request_focus();
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
                    self.runtime_config.launcher.translate_providers = selected.clone();
                    save_config(move |cfg| cfg.launcher.translate_providers = selected);
                }
                self.query = ":set".to_string();
                self.handle_settings_query(":set");
                return self.request_focus();
            }
            theme_action if theme_action.starts_with("theme:") => {
                let theme_name = theme_action.strip_prefix("theme:").unwrap_or("midnight");
                self.theme = crate::theme::from_name(theme_name);
                self.runtime_config.general.theme = theme_name.to_string();
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
        self.clear_results_state(kmd_core::SearchMode::Fuzzy);
        self.request_focus()
    }

    pub(super) fn schedule_autostart_status_refresh(&mut self, force: bool) -> Task<Message> {
        if self.daemon_autostart_check_in_flight {
            return Task::none();
        }
        if !force
            && self
                .daemon_autostart_last_checked_at
                .is_some_and(|ts| ts.elapsed() < Duration::from_millis(AUTOSTART_STATUS_REFRESH_MS))
        {
            return Task::none();
        }
        self.daemon_autostart_check_in_flight = true;
        Task::future(async move {
            let result = tokio::task::spawn_blocking(|| {
                kmd_core::ipc::send_request_result(&kmd_core::ipc::Request::AutostartStatus)
            })
            .await;
            let mapped = match result {
                Ok(Ok(kmd_core::ipc::Response::AutostartStatus { installed })) => Ok(installed),
                Ok(Ok(other)) => Err(format!("예기치 않은 응답: {other:?}")),
                Ok(Err(e)) => Err(format!("IPC 실패: {e}")),
                Err(e) => Err(format!("작업 실패: {e}")),
            };
            Message::AutostartStatusLoaded(mapped)
        })
    }
}
