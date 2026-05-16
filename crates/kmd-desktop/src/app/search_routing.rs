use super::*;

use crate::query_prefix::{prefix_of, Prefix};

impl App {
    pub(super) fn perform_search(&mut self) -> Task<Message> {
        let query = self.query.clone();
        let trimmed = query.trim();

        if trimmed.is_empty() {
            self.clear_results_state(kmd_core::SearchMode::Fuzzy);
            return Task::none();
        }

        let prev_signature = self.last_results_signature;
        let mut post_task: Task<Message> = Task::none();

        match prefix_of(trimmed) {
            Prefix::Web => self.handle_web_query(trimmed),
            Prefix::Transform => self.handle_transform_query(trimmed),
            Prefix::Prompt => self.handle_prompt_query(trimmed),
            Prefix::Calc => self.handle_calc_query(trimmed),
            Prefix::Emoji => self.handle_emoji_query(trimmed),
            Prefix::Settings => {
                self.handle_settings_query(trimmed);
                post_task = self.schedule_autostart_status_refresh(false);
            }
            Prefix::Help => self.handle_help_query(),
            Prefix::Version => self.handle_version_query(),
            Prefix::Shell => self.handle_shell_query(trimmed),
            Prefix::Keymap => self.handle_keymap_query(trimmed),
            Prefix::Keys => self.handle_keys_query(),
            Prefix::FolderSearch => self.handle_folder_search(trimmed),
            Prefix::General => self.handle_main_search(trimmed),
        }

        // 결과가 실제로 바뀐 경우에만 아이콘 prefetch를 실행한다.
        let icon_task = if self.last_results_signature != prev_signature {
            self.spawn_icon_prefetch()
        } else {
            Task::none()
        };
        Task::batch([icon_task, post_task])
    }

    /// classify_web_query 통합 분류기 사용
    pub(super) fn handle_web_query(&mut self, query: &str) {
        let emoji = self.use_emoji;
        let cfg = web::WebQueryConfig {
            spell_prefixes: &self.spell_prefixes,
            translate_prefixes: &self.translate_prefixes,
            multi_llm_prefixes: &self.multi_llm_prefixes,
            multi_llm_ids: &self.selected_llm_providers,
            multi_web_prefixes: &self.multi_web_prefixes,
            multi_web_ids: &self.selected_multi_web_providers,
        };

        let results = match web::classify_web_query(query, &cfg) {
            web::WebQueryResult::Spell(q) => {
                items_to_results(web::spell_result_items(&q, &self.spell_providers, emoji))
            }
            web::WebQueryResult::Translate(dir, q) => items_to_results(
                web::translate_result_items(&q, dir, &self.translate_providers, emoji),
            ),
            web::WebQueryResult::MultiLlm(_svcs, q) => items_to_results(
                web::multi_llm_result_items(&q, &self.selected_llm_providers, emoji),
            ),
            web::WebQueryResult::MultiWeb(_svcs, q) => items_to_results(
                web::multi_web_result_items(&q, &self.selected_multi_web_providers, emoji),
            ),
            web::WebQueryResult::Single(service, q) => {
                if q.is_empty() {
                    let mut items = web::list_services_as_items("", emoji);
                    ensure_multi_llm_hint(&mut items, emoji);
                    ensure_multi_web_hint(&mut items, emoji);
                    items_to_results(items)
                } else {
                    let item = web::search_result_item(service, &q, emoji);
                    items_to_results(std::iter::once(item))
                }
            }
            web::WebQueryResult::Browse(filter) => {
                let mut items = web::list_services_as_items(&filter, emoji);
                ensure_multi_llm_hint(&mut items, emoji);
                ensure_multi_web_hint(&mut items, emoji);
                items_to_results(items)
            }
        };
        self.commit_results(results, kmd_core::SearchMode::Contains, true);
    }

    pub(super) fn handle_version_query(&mut self) {
        let emoji = self.use_emoji;
        let version_items = vec![
            IndexItem {
                name: format!("kmd-desktop {}", env!("CARGO_PKG_VERSION")),
                path: "Desktop launcher version".to_string(),
                icon: if emoji { "\u{1F4E6}" } else { "[VER]" }.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: "kmd:settings:noop".to_string(),
                icon_path: None,
            },
            IndexItem {
                name: format!("kmd-core {}", kmd_core::Index::current_version()),
                path: "Search index schema version".to_string(),
                icon: if emoji { "\u{1F9E0}" } else { "[CORE]" }.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: "kmd:settings:noop".to_string(),
                icon_path: None,
            },
            IndexItem {
                name: format!("target {}", std::env::consts::ARCH),
                path: format!("os {}", std::env::consts::OS),
                icon: if emoji { "\u{1F5A5}\u{FE0F}" } else { "[SYS]" }.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                keywords: "kmd:settings:noop".to_string(),
                icon_path: None,
            },
        ];
        self.apply_contains_items(version_items);
    }

    /// :t / :transform 쿼리 처리 (클립보드 변환)
    pub(super) fn handle_transform_query(&mut self, query: &str) {
        use kmd_core::transform;

        match transform::parse_transform_query(query) {
            Some(mut tq) => {
                if tq.text.is_empty() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            tq.text = text;
                        }
                    }
                }
                if tq.text.is_empty() {
                    self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
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
                        icon_path: None,
                    })));
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
                self.clear_results_state(kmd_core::SearchMode::Contains);
            }
            None => {
                let items = transform::help_items(self.use_emoji);
                self.apply_contains_items(items);
            }
        }
    }

    /// :prompt / :pt 쿼리 처리
    pub(super) fn handle_prompt_query(&mut self, query: &str) {
        let sub = query
            .strip_prefix(":prompt")
            .or_else(|| query.strip_prefix(":pt"))
            .unwrap_or("")
            .trim();

        let templates = self.runtime_config.launcher.prompt_templates.clone();

        if sub.starts_with("add ") {
            let rest = sub.strip_prefix("add ").unwrap_or("").trim();
            if let Some(pos) = rest.find(char::is_whitespace) {
                let name = &rest[..pos];
                let body = rest[pos..].trim();
                if !kmd_core::prompt::validate_template_name(name) {
                    self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
                        name: format!("❌ 잘못된 이름: '{name}'"),
                        path: "영문/숫자/하이픈/언더스코어만, 최대 32자".to_string(),
                        kind: ItemKind::SystemCommand,
                        source: Source::Plugin,
                        icon: if self.use_emoji { "\u{274C}" } else { "[!]" }.to_string(),
                        keywords: "kmd:settings:noop".to_string(),
                        icon_path: None,
                    })));
                } else if body.is_empty() {
                    self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
                        name: "❌ 본문이 비어 있습니다".to_string(),
                        path: ":prompt add <name> <body> 형태로 입력하세요".to_string(),
                        kind: ItemKind::SystemCommand,
                        source: Source::Plugin,
                        icon: if self.use_emoji { "\u{274C}" } else { "[!]" }.to_string(),
                        keywords: "kmd:settings:noop".to_string(),
                        icon_path: None,
                    })));
                } else {
                    self.runtime_config
                        .launcher
                        .prompt_templates
                        .retain(|t| !t.name.eq_ignore_ascii_case(name));
                    self.runtime_config.launcher.prompt_templates.push(
                        kmd_core::config::PromptTemplate {
                            name: name.to_string(),
                            body: body.to_string(),
                        },
                    );
                    let templates_to_save = self.runtime_config.launcher.prompt_templates.clone();
                    save_config(move |c| c.launcher.prompt_templates = templates_to_save);
                    self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
                        name: format!("✅ 템플릿 '{name}' 저장됨"),
                        path: format!("@ll :{name} <query> 형태로 사용"),
                        kind: ItemKind::SystemCommand,
                        source: Source::Plugin,
                        icon: if self.use_emoji { "\u{2705}" } else { "[OK]" }.to_string(),
                        keywords: "kmd:settings:noop".to_string(),
                        icon_path: None,
                    })));
                }
            } else {
                self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
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
                    icon_path: None,
                })));
            }
            return;
        }

        if sub.starts_with("remove ") || sub.starts_with("rm ") || sub.starts_with("del ") {
            let name = sub
                .strip_prefix("remove ")
                .or_else(|| sub.strip_prefix("rm "))
                .or_else(|| sub.strip_prefix("del "))
                .unwrap_or("")
                .trim();
            if name.is_empty() {
                self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
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
                    icon_path: None,
                })));
            } else if templates.iter().any(|t| t.name.eq_ignore_ascii_case(name)) {
                let name_owned = name.to_string();
                let display_name = name.to_string();
                self.runtime_config
                    .launcher
                    .prompt_templates
                    .retain(|t| !t.name.eq_ignore_ascii_case(&name_owned));
                let templates_to_save = self.runtime_config.launcher.prompt_templates.clone();
                save_config(move |cfg| cfg.launcher.prompt_templates = templates_to_save);
                self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
                    name: format!("✅ 템플릿 '{display_name}' 삭제됨"),
                    path: String::new(),
                    kind: ItemKind::SystemCommand,
                    source: Source::Plugin,
                    icon: if self.use_emoji { "\u{2705}" } else { "[OK]" }.to_string(),
                    keywords: "kmd:settings:noop".to_string(),
                    icon_path: None,
                })));
            } else {
                self.apply_contains_results(items_to_results(std::iter::once(IndexItem {
                    name: format!("❌ 템플릿 '{name}'을 찾을 수 없습니다"),
                    path: String::new(),
                    kind: ItemKind::SystemCommand,
                    source: Source::Plugin,
                    icon: if self.use_emoji { "\u{274C}" } else { "[!]" }.to_string(),
                    keywords: "kmd:settings:noop".to_string(),
                    icon_path: None,
                })));
            }
            return;
        }

        let filter = sub.strip_prefix("list").unwrap_or(sub).trim();
        let items = kmd_core::prompt::list_templates_as_items(&templates, filter, self.use_emoji);
        self.apply_contains_items(items);
    }

    pub(super) fn handle_calc_query(&mut self, query: &str) {
        let expr = query.strip_prefix(":calc").unwrap_or("").trim();
        let calc = builtin_calc::CalcExtension;
        self.apply_contains_items(calc.search_with_emoji(expr, self.use_emoji));
    }

    pub(super) fn handle_emoji_query(&mut self, query: &str) {
        let search_query = query
            .strip_prefix(":emoji")
            .or_else(|| query.strip_prefix(":e"))
            .unwrap_or("")
            .trim();
        let emoji_ext = builtin_emoji::EmojiExtension;
        self.apply_contains_items(emoji_ext.search_emoji(search_query));
    }

    pub(super) fn handle_shell_query(&mut self, query: &str) {
        let shell_query = query.strip_prefix('!').unwrap_or("").trim();
        let shell_ext = builtin_shell::ShellExtension;
        self.apply_contains_items(shell_ext.search(shell_query));
    }

    /// :keys / :k — 키 맵핑 치트시트
    pub(super) fn handle_keys_query(&mut self) {
        let items = kmd_core::keymap::keybinding_cheatsheet(&self.runtime_config, self.use_emoji);
        self.apply_contains_items(items);
    }

    /// :keymap / :km 쿼리 처리
    pub(super) fn handle_keymap_query(&mut self, query: &str) {
        let sub = query
            .strip_prefix(":keymap")
            .or_else(|| query.strip_prefix(":km"))
            .unwrap_or("")
            .trim();
        let items = kmd_core::keymap::keymap_items(&self.runtime_config, sub, self.use_emoji);
        self.apply_contains_items(items);
    }

    pub(super) fn handle_main_search(&mut self, query: &str) {
        let (mode, mut results) = self.engine.search(query, SEARCH_LIMIT);
        self.search_mode = mode;

        // 실행 이력 기반으로 자주 사용하는 항목의 점수를 높인다
        kmd_core::history::boost_results(&mut results, &self.db);

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

        self.commit_results(results, self.search_mode, true);
    }

    /// `:f /경로 쿼리` — 지정 폴더 안에서 파일/폴더를 즉석 검색.
    ///
    /// 형식: `:f /path/to/dir 검색어` 또는 `:f ~/dir 검색어`
    /// 경로만 있고 검색어가 없으면 최상위 목록을 보여준다.
    pub(super) fn handle_folder_search(&mut self, raw: &str) {
        // ":f " 또는 ":f" 이후 텍스트 파싱
        let after_prefix = raw.strip_prefix(":f").unwrap_or("").trim();

        if after_prefix.is_empty() {
            // 도움말 항목만 표시
            self.commit_results(
                vec![folder_search_help_item()],
                kmd_core::SearchMode::Fuzzy,
                true,
            );
            return;
        }

        // 첫 번째 토큰을 경로로, 나머지를 검색어로 사용
        let (dir_part, name_query) = match after_prefix.find(' ') {
            Some(pos) => (
                after_prefix[..pos].trim(),
                after_prefix[pos + 1..].trim(),
            ),
            None => (after_prefix, ""),
        };

        // ~ 확장
        let dir_str = if dir_part.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_default();
            dir_part.replacen('~', &home, 1)
        } else {
            dir_part.to_string()
        };

        let dir = std::path::Path::new(&dir_str);
        if !dir.is_dir() {
            self.commit_results(
                vec![folder_not_found_item(dir_part)],
                kmd_core::SearchMode::Fuzzy,
                true,
            );
            return;
        }

        // 폴더 내 항목 열거 (1단계 + 선택적 재귀)
        let query_lower = name_query.to_lowercase();
        let mut results: Vec<kmd_core::SearchResult> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // 숨김 파일 제외
                if file_name.starts_with('.') {
                    continue;
                }

                // 검색어 필터 — 쿼리가 있을 때만 lowercase 변환 (할당 최소화)
                if !query_lower.is_empty() {
                    let name_lower = file_name.to_ascii_lowercase();
                    if !name_lower.contains(query_lower.as_str()) {
                        continue;
                    }
                }

                let is_dir = path.is_dir();
                let kind = if is_dir {
                    kmd_core::index::ItemKind::Directory
                } else {
                    kmd_core::index::ItemKind::File
                };
                let icon = if is_dir { "D/" } else { "F " };

                results.push(kmd_core::SearchResult {
                    item: kmd_core::index::IndexItem {
                        name: file_name,
                        path: path.to_string_lossy().to_string(),
                        kind,
                        source: kmd_core::index::Source::FileProvider,
                        icon: icon.to_string(),
                        keywords: String::new(),
                        icon_path: None,
                    },
                    score: 0,
                });
            }
        }

        // 이름 오름차순 정렬 (폴더 우선)
        results.sort_by(|a, b| {
            let a_dir = a.item.kind == kmd_core::index::ItemKind::Directory;
            let b_dir = b.item.kind == kmd_core::index::ItemKind::Directory;
            b_dir.cmp(&a_dir).then(a.item.name.cmp(&b.item.name))
        });

        if results.is_empty() {
            results.push(no_match_in_folder_item(dir_part, name_query));
        }

        self.commit_results(results, kmd_core::SearchMode::Contains, true);
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

        // 폴더 지정 검색 힌트 — 인덱스에 없는 파일을 찾을 때 유용함을 알림
        items.push(kmd_core::SearchResult {
            item: kmd_core::index::IndexItem {
                name: format!(":f /폴더경로 {query}  — 폴더 직접 지정 검색"),
                path: format!(":f  {query}"),
                kind: kmd_core::index::ItemKind::SystemCommand,
                source: kmd_core::index::Source::Plugin,
                icon: "F>".to_string(),
                keywords: "kmd:folder_search:suggest".to_string(),
                icon_path: None,
            },
            score: 0,
        });

        items
    }
}

// ── 폴더 검색 헬퍼 항목 ──────────────────────────────────────────────────────

fn folder_search_help_item() -> kmd_core::SearchResult {
    kmd_core::SearchResult {
        item: kmd_core::index::IndexItem {
            name: ":f /경로 검색어  — 폴더 지정 검색".to_string(),
            path: String::new(),
            kind: kmd_core::index::ItemKind::SystemCommand,
            source: kmd_core::index::Source::Plugin,
            icon: "F?".to_string(),
            keywords: "kmd:folder_search:hint".to_string(),
            icon_path: None,
        },
        score: 0,
    }
}

fn folder_not_found_item(dir: &str) -> kmd_core::SearchResult {
    kmd_core::SearchResult {
        item: kmd_core::index::IndexItem {
            name: format!("폴더 없음: {dir}"),
            path: String::new(),
            kind: kmd_core::index::ItemKind::SystemCommand,
            source: kmd_core::index::Source::Plugin,
            icon: "F!".to_string(),
            keywords: "kmd:folder_search:error".to_string(),
            icon_path: None,
        },
        score: 0,
    }
}

fn no_match_in_folder_item(dir: &str, query: &str) -> kmd_core::SearchResult {
    kmd_core::SearchResult {
        item: kmd_core::index::IndexItem {
            name: format!("'{query}' — {dir} 안에 결과 없음"),
            path: String::new(),
            kind: kmd_core::index::ItemKind::SystemCommand,
            source: kmd_core::index::Source::Plugin,
            icon: "F0".to_string(),
            keywords: "kmd:folder_search:empty".to_string(),
            icon_path: None,
        },
        score: 0,
    }
}
