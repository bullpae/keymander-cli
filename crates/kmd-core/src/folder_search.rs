//! `:f /경로 쿼리` — 특정 폴더 내 즉석 파일 검색 (TUI/데스크톱 공용)

use crate::index::{IndexItem, ItemKind, Source};
use crate::search::SearchResult;

/// `:f` 이후의 입력을 파싱해 폴더 내 검색 결과를 만든다.
///
/// 형식: `:f /path/to/dir 검색어` 또는 `:f ~/dir 검색어`
/// - 입력이 비어 있으면 사용법 안내 항목을 반환
/// - 경로가 없으면 오류 안내 항목을 반환
/// - 검색어 없이 경로만 있으면 최상위 목록을 반환
pub fn folder_search_results(query: &str, use_emoji: bool) -> Vec<SearchResult> {
    let after_prefix = query.strip_prefix(":f").unwrap_or(query).trim();

    if after_prefix.is_empty() {
        return vec![help_item()];
    }

    // 첫 번째 토큰을 경로로, 나머지를 검색어로 사용
    let (dir_part, name_query) = match after_prefix.find(' ') {
        Some(pos) => (after_prefix[..pos].trim(), after_prefix[pos + 1..].trim()),
        None => (after_prefix, ""),
    };

    // ~ 확장 (HOME, Windows는 USERPROFILE 폴백)
    let dir_str = if dir_part.starts_with('~') {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        dir_part.replacen('~', &home, 1)
    } else {
        dir_part.to_string()
    };

    let dir = std::path::Path::new(&dir_str);
    if !dir.is_dir() {
        return vec![not_found_item(dir_part)];
    }

    // 폴더 내 항목 열거 (1단계)
    let query_lower = name_query.to_lowercase();
    let mut results: Vec<SearchResult> = Vec::new();

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
                ItemKind::Directory
            } else {
                ItemKind::File
            };
            let icon = if is_dir {
                if use_emoji {
                    "\u{1F4C1}"
                } else {
                    "D/"
                }
            } else if use_emoji {
                "\u{1F4C4}"
            } else {
                "F "
            };

            // 검색어와 얼마나 일치하는지 점수 부여 (이름 접두사 일치 우대)
            let score: u32 = if !query_lower.is_empty() {
                let nl = file_name.to_ascii_lowercase();
                if nl.starts_with(query_lower.as_str()) {
                    20
                } else {
                    10
                }
            } else {
                0
            };

            results.push(SearchResult {
                item: IndexItem {
                    name: file_name,
                    path: path.to_string_lossy().to_string(),
                    kind,
                    source: Source::FileProvider,
                    icon: icon.to_string(),
                    keywords: String::new(),
                    icon_path: None,
                },
                score,
            });
        }
    }

    // 점수 내림차순 → 같은 점수는 폴더 우선 → 이름 오름차순
    results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| {
                let a_dir = a.item.kind == ItemKind::Directory;
                let b_dir = b.item.kind == ItemKind::Directory;
                b_dir.cmp(&a_dir)
            })
            .then(a.item.name.cmp(&b.item.name))
    });

    if results.is_empty() {
        results.push(no_match_item(dir_part, name_query));
    }

    results
}

fn help_item() -> SearchResult {
    SearchResult {
        item: IndexItem {
            name: ":f /경로  또는  :f ~/경로 검색어".to_string(),
            path: "Enter로 폴더를 열거나, 경로 뒤에 검색어를 입력해 파일을 찾으세요".to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: "\u{1F4C2}".to_string(),
            keywords: "kmd:folder_search:hint".to_string(),
            icon_path: None,
        },
        score: 0,
    }
}

fn not_found_item(dir: &str) -> SearchResult {
    SearchResult {
        item: IndexItem {
            name: format!("폴더를 찾을 수 없음: {dir}"),
            path: "경로가 올바른지 확인하거나 Tab으로 경로를 완성해 보세요".to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: "\u{26A0}\u{FE0F}".to_string(),
            keywords: "kmd:folder_search:error".to_string(),
            icon_path: None,
        },
        score: 0,
    }
}

fn no_match_item(dir: &str, query: &str) -> SearchResult {
    SearchResult {
        item: IndexItem {
            name: format!("'{query}'에 해당하는 파일이 없습니다"),
            path: format!("검색 위치: {dir}"),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: "\u{1F50D}".to_string(),
            keywords: "kmd:folder_search:empty".to_string(),
            icon_path: None,
        },
        score: 0,
    }
}
