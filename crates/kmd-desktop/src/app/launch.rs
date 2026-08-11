//! 항목 실행, 컨텍스트 액션, 단축키 처리

use super::context_actions_for;
use super::*;

impl App {
    pub(super) fn execute_context_action(&mut self, action: ContextAction) -> Task<Message> {
        let Some(result) = self.results.get(self.selected) else {
            return Task::none();
        };
        let item = &result.item;

        match action {
            ContextAction::Open => self.launch_selected(),
            ContextAction::OpenAsAdmin => {
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                    let path = item.path.clone();
                    let escaped = path.replace('\'', "''");
                    let script = format!("Start-Process -FilePath '{}' -Verb RunAs", escaped);
                    if let Err(e) = std::process::Command::new("powershell")
                        .args(["-NoProfile", "-Command", &script])
                        .creation_flags(CREATE_NO_WINDOW)
                        .spawn()
                    {
                        tracing::error!("관리자 실행 실패: {e}");
                    }
                    return iced::exit();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    self.launch_selected()
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
                iced::exit()
            }
            ContextAction::CopyPath => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Err(e) = clipboard.set_text(&item.path) {
                        tracing::warn!("클립보드 쓰기 실패: {e}");
                    }
                }
                iced::exit()
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
                iced::exit()
            }
        }
    }

    pub(super) fn launch_selected(&mut self) -> Task<Message> {
        // 이어서 질문: `@@ <프롬프트>` → 데몬이 기억한 LLM 창들에 전달 (열 URL 없음)
        if let Some(followup) = web::parse_llm_followup(&self.query) {
            return self.send_llm_followup(&followup);
        }

        let Some(result) = self.results.get(self.selected).cloned() else {
            return Task::none();
        };

        // 클립보드 히스토리 (docs/12 흐름 B): 데몬에 "이 항목을 런처 열기 전 앱에
        // 붙여넣어라"고 요청하고 창을 닫는다. 데몬이 이전 앱을 활성화한 뒤 주입한다.
        // 마커는 `kmd:clip:<id>:<slot>` (clip_item이 생성) — 안정 ID로 붙여넣어
        // 검색 뒤 새 복사로 슬롯이 밀려도 선택한 항목을 정확히 집는다.
        // id=0은 구버전 데몬 응답(안정 ID 없음) → 슬롯 요청으로 폴백한다.
        if let Some(marker) = result.item.keywords.strip_prefix("kmd:clip:") {
            if marker == "notice" {
                return Task::none(); // 안내 항목 — 실행 대상 아님
            }
            let mut parts = marker.split(':');
            let id = parts.next().and_then(|s| s.parse::<u64>().ok());
            let slot = parts.next().and_then(|s| s.parse::<usize>().ok());
            let request = match (id, slot) {
                (Some(id), _) if id > 0 => Some(kmd_core::ipc::Request::ClipPasteItem {
                    id,
                    to_previous: true,
                }),
                (_, Some(slot)) if slot > 0 => Some(kmd_core::ipc::Request::ClipPaste {
                    slot,
                    to_previous: true,
                }),
                _ => None,
            };
            let Some(request) = request else {
                return iced::exit(); // 마커 손상 — 요청할 대상이 없다
            };
            // 성공(Ok)에만 창을 닫는다. 실패는 결과 리스트에 표시해 사용자가
            // 원인(항목 만료·스냅샷 실패·데몬 중지)을 보고 다시 시도할 수 있게.
            return match kmd_core::ipc::send_request_result(&request) {
                Ok(kmd_core::ipc::Response::Error { message }) => {
                    tracing::warn!("클립보드 붙여넣기 실패: {message}");
                    self.apply_contains_items(vec![super::search_routing::clip_notice(
                        "클립보드 붙여넣기 실패",
                        &message,
                        self.use_emoji,
                    )]);
                    Task::none()
                }
                Err(e) => {
                    tracing::warn!("클립보드 붙여넣기 요청 실패: {e}");
                    self.apply_contains_items(vec![super::search_routing::clip_notice(
                        "클립보드 붙여넣기 실패",
                        "데몬에 연결할 수 없습니다 — kmd daemon status로 확인하세요",
                        self.use_emoji,
                    )]);
                    Task::none()
                }
                Ok(_) => iced::exit(),
            };
        }

        // Help entries act like quick templates — 선택 시 시작 쿼리로 전환.
        if result.item.keywords.starts_with("kmd:help:") {
            if let Some(seed) = kmd_core::query_prefix::help_query_seed(&result.item.name) {
                self.query = seed.to_string();
                self.selected = 0;
                return self.perform_search();
            }
            return Task::none();
        }

        // 셸 모드의 웹 검색 전환 힌트 (!g → @g)
        if result.item.keywords.starts_with("kmd:bang_hint:") {
            self.query = result.item.path.clone();
            self.selected = 0;
            return self.perform_search();
        }

        // 미지 명령 안내 → :help 로 이동
        if result.item.keywords == "kmd:unknown_cmd" {
            self.query = ":help".to_string();
            self.selected = 0;
            return self.perform_search();
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
            // 사용자 명령 → 새 터미널 창에서 실행 (결과가 화면에 유지됨)
            if let Err(e) = builtin_shell::launch_in_terminal(&result.item.path) {
                tracing::warn!("터미널 실행 실패: {e}");
            }
            return iced::exit();
        }

        // 폴더 검색 제안 항목 — path가 ":f  <query>" 형태이면 해당 쿼리로 전환
        if result.item.keywords.starts_with("kmd:folder_search:") {
            let seed = result.item.path.trim().to_string();
            if seed.starts_with(":f") {
                self.query = seed;
                self.selected = 0;
                return self.perform_search();
            }
            return Task::none();
        }

        // `@` 브라우징 가상 항목 — 설명 문구를 explorer로 열지 않고 해당 모드로 전환
        if result.item.kind == ItemKind::WebSearch {
            if let Some(seed) = web::web_browse_hint_seed(&result.item.keywords) {
                self.query = seed.to_string();
                self.selected = 0;
                return self.perform_search();
            }
        }

        // LLM 실행(@gpt/@llm) — 오토파일럿 또는 URL 폴백으로 라우팅
        if result.item.kind == ItemKind::WebSearch {
            if let Some(task) = self.try_llm_launch() {
                return task;
            }
        }

        // 웹 검색 결과 — extract_batch_urls 통합 추출 (LLM 외 msearch/spell/translate)
        if result.item.kind == ItemKind::WebSearch {
            if let Some(urls) = web::extract_batch_urls(&result.item) {
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

        // 계산기 결과: 값을 클립보드에 복사 후 종료
        if result.item.kind == ItemKind::Calculator {
            if !result.item.path.is_empty() {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&result.item.path);
                }
            }
            return iced::exit();
        }

        let action_result = kmd_core::action::execute(&result);
        match action_result {
            kmd_core::action::ActionResult::Launched => {
                // 앱/파일 실행 기록 → 다음 검색 시 frecency 부스트에 사용
                let kind_str = match result.item.kind {
                    ItemKind::App => "app",
                    ItemKind::Executable => "app",
                    ItemKind::File => "file",
                    ItemKind::Directory => "dir",
                    _ => "misc",
                };
                kmd_core::history::record_launch(
                    &self.db,
                    kind_str,
                    &result.item.path,
                    Some(&result.item.name),
                );
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
    pub(super) fn match_shortcut(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<ContextAction> {
        if self.results.is_empty() {
            return None;
        }
        let item_kind = self.results.get(self.selected)?.item.kind;
        let available = context_actions_for(item_kind);

        if modifiers.control()
            && modifiers.shift()
            && matches!(key, keyboard::Key::Named(keyboard::key::Named::Enter))
            && available
                .iter()
                .any(|a| matches!(a, ContextAction::OpenAsAdmin))
        {
            return Some(ContextAction::OpenAsAdmin);
        }

        #[cfg(target_os = "macos")]
        let ctrl_like = modifiers.control() || modifiers.logo();
        #[cfg(not(target_os = "macos"))]
        let ctrl_like = modifiers.control();

        if ctrl_like {
            let idx = match key {
                keyboard::Key::Character(c) => match c.as_str() {
                    // Shift가 눌린 숫자 기호(!,@,#,$)나 레이아웃 변형에서도
                    // 같은 단축키 인덱스로 해석한다.
                    "1" | "!" => Some(0usize),
                    "2" | "@" => Some(1),
                    "3" | "#" => Some(2),
                    "4" | "$" => Some(3),
                    _ => None,
                },
                _ => None,
            };
            if let Some(i) = idx {
                // 먼저 위치(position) 기반으로 시도
                if let Some(action) = available.get(i).copied() {
                    return Some(action);
                }
                // 위치 기반으로 해당 액션이 없으면 shortcut_index 기반으로 매칭.
                // Calculator(CopyName 1개)처럼 액션이 적은 항목에서도
                // 해당 액션의 고유 단축키 번호(Ctrl+4 등)가 동작하도록 한다.
                return available.into_iter().find(|a| a.shortcut_index() == i);
            }
        }

        None
    }

    pub(super) fn handle_key(&mut self, key: keyboard::Key) -> Task<Message> {
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
                if self.state_dirty {
                    self.window_state.save();
                }
                iced::exit()
            }
            keyboard::Key::Named(keyboard::key::Named::F1) => {
                self.query = ":keys".to_string();
                self.perform_search()
            }
            _ => Task::none(),
        }
    }

    pub(super) fn scroll_to_selected(&self) -> Task<Message> {
        if self.results.is_empty() {
            return Task::none();
        }
        let top_row = self.selected.saturating_sub(self.ui.visible_rows - 1);
        let y_offset = top_row as f32 * self.ui.row_height;
        scroll_to(
            self.scrollable_id.clone(),
            scrollable_mod::AbsoluteOffset {
                x: 0.0,
                y: y_offset,
            },
        )
    }

    pub(super) fn clear_query_and_refocus(&mut self) -> Task<Message> {
        self.query.clear();
        self.clear_results_state(kmd_core::SearchMode::Fuzzy);
        self.request_focus()
    }

    /// 브랜드 클릭 등으로 특정 모드(`:help`, `:set` 등)를 토글한다.
    /// 이미 해당 모드면 쿼리를 지우고, 아니면 해당 쿼리로 전환한다.
    pub(super) fn toggle_query_mode(&mut self, mode: &str) -> Task<Message> {
        use crate::query_prefix::prefix_of;
        let current_prefix = prefix_of(self.query.trim());
        let target_prefix = prefix_of(mode);
        if current_prefix == target_prefix {
            self.clear_query_and_refocus()
        } else {
            self.query = mode.to_string();
            self.perform_search()
        }
    }
}
