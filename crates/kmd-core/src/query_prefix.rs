//! 검색창 특수 프리픽스 분류 — TUI와 데스크톱이 공유하는 단일 파서.
//!
//! 별칭 매칭 규칙: **완전일치 또는 별칭 바로 뒤 공백**(토큰 경계).
//! 예: `:pt hello` → Prompt, `:pto` → General (별칭 오인 방지),
//! `:settings` → Settings, `:setup` → General.

use crate::index::{IndexItem, ItemKind, Source};

/// 검색창 입력의 특수 프리픽스 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryPrefix {
    /// `@` — 웹/AI 서비스 검색
    Web,
    /// `:t` / `:transform` — 클립보드 변환 (맞춤법/번역)
    Transform,
    /// `:prompt` / `:pt` — 프롬프트 템플릿 관리
    Prompt,
    /// `:calc` — 계산기
    Calc,
    /// `:emoji` / `:e` — 이모지 검색
    Emoji,
    /// `:set` / `:settings` — 설정
    Settings,
    /// `:help` / `:h` — 도움말
    Help,
    /// `:version` / `:ver` / `:v` — 버전 정보
    Version,
    /// `!` — 셸 명령 실행
    Shell,
    /// `:keymap` / `:km` — kanata 키맵 제어
    Keymap,
    /// `:keys` / `:k` — 키 바인딩 치트시트
    Keys,
    /// `:f` — 특정 폴더 내 즉석 파일 검색
    FolderSearch,
    /// 일반 검색 (fuzzy / glob / regex / contains / url)
    General,
}

/// `:` 명령과 별칭 테이블 — `prefix_of`가 순회하는 단일 소스.
pub const COMMAND_ALIASES: &[(QueryPrefix, &[&str])] = &[
    (QueryPrefix::Transform, &[":transform", ":t"]),
    (QueryPrefix::Prompt, &[":prompt", ":pt"]),
    (QueryPrefix::Calc, &[":calc"]),
    (QueryPrefix::Emoji, &[":emoji", ":e"]),
    (QueryPrefix::Settings, &[":settings", ":set"]),
    (QueryPrefix::Help, &[":help", ":h"]),
    (QueryPrefix::Version, &[":version", ":ver", ":v"]),
    (QueryPrefix::Keymap, &[":keymap", ":km"]),
    (QueryPrefix::Keys, &[":keys", ":k"]),
    (QueryPrefix::FolderSearch, &[":f"]),
];

/// query가 별칭 중 하나와 토큰 경계로 일치하는지 검사.
///
/// "완전일치" 또는 "별칭 + 공백"만 인정한다 — `:pto`는 `:pt`에 걸리지 않는다.
pub fn matches_command(query: &str, aliases: &[&str]) -> bool {
    aliases.iter().any(|alias| {
        query
            .strip_prefix(alias)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

/// `:help` 결과 목록 — TUI와 데스크톱이 공유하는 명령 안내 항목.
///
/// 항목을 Enter로 선택하면 [`help_query_seed`]가 주는 시작 쿼리로 전환된다.
pub fn help_items(use_emoji: bool) -> Vec<IndexItem> {
    let emoji = use_emoji;
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
            ":t  Quick Transform",
            "Type :t spell/tr/trko/tren  (clipboard text → spell/translate)",
            if emoji { "\u{26A1}" } else { "[QT]" },
        ),
        (
            ":prompt  Prompt Templates",
            "Type :prompt  (manage reusable prompt templates for @ll)",
            if emoji { "\u{1F4DD}" } else { "[PT]" },
        ),
        (
            ":f  Folder Search",
            "Type :f /path query  (search inside a specific folder)",
            if emoji { "\u{1F4C2}" } else { "[DIR]" },
        ),
        (
            ":keys  Key Mapping Sheet",
            "Type :keys or :k  (show all keybinding cheatsheet)",
            if emoji { "\u{1F5FA}\u{FE0F}" } else { "[KEY]" },
        ),
        (
            ":keymap  Keymap Control",
            "Type :keymap or :km  (kanata status, on/off, profile switch)",
            if emoji { "\u{2328}\u{FE0F}" } else { "[KM]" },
        ),
        (
            ":version  Version Info",
            "Type :version  (show app/core/target/os versions)",
            if emoji { "\u{1F4E6}" } else { "[VER]" },
        ),
        (
            "@llm  Multi LLM Compare",
            "Type @ll prompt  (alias: @llm, open selected LLM providers)",
            if emoji { "\u{1F9E0}" } else { "[LLM]" },
        ),
        (
            "@msearch  Multi Web Search",
            "Type @m query  (alias: @msearch, open selected web engines)",
            if emoji { "\u{1F50E}" } else { "[MWEB]" },
        ),
        (
            "@sp  Spell Check",
            "Type @sp text  (Korean spelling check on selected providers)",
            if emoji { "\u{270D}\u{FE0F}" } else { "[SPL]" },
        ),
        (
            "@tr  Translate",
            "Type @tr/@trko/@tren text  (auto / en->ko / ko->en)",
            if emoji { "\u{1F5E3}\u{FE0F}" } else { "[TR]" },
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

    entries
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
                "kmd:help:entry".to_string()
            },
            icon_path: None,
        })
        .collect()
}

/// 도움말 항목 이름 → Enter 시 검색창에 채울 시작 쿼리(퀵 템플릿).
pub fn help_query_seed(name: &str) -> Option<&'static str> {
    if name.starts_with("@ll") || name.starts_with("@llm") {
        Some("@ll ")
    } else if name.starts_with("@m") {
        Some("@m ")
    } else if name.starts_with("@sp") {
        Some("@sp ")
    } else if name.starts_with("@tr") {
        Some("@tr ")
    } else if name.starts_with('@') {
        Some("@")
    } else if name.starts_with(":calc") {
        Some(":calc ")
    } else if name.starts_with(":emoji") {
        Some(":emoji ")
    } else if name.starts_with(":set") {
        Some(":set")
    } else if name.starts_with(":t ") {
        Some(":t ")
    } else if name.starts_with(":prompt") {
        Some(":prompt")
    } else if name.starts_with(":f ") {
        Some(":f ")
    } else if name.starts_with(":keys") {
        Some(":keys")
    } else if name.starts_with(":keymap") {
        Some(":keymap")
    } else if name.starts_with(":version") || name.starts_with("Version Info") {
        Some(":version")
    } else if name.starts_with('!') {
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

/// `:version` 결과 목록 — 앱 이름/버전만 프런트엔드별로 다르다.
pub fn version_items(app_label: &str, app_version: &str, use_emoji: bool) -> Vec<IndexItem> {
    let emoji = use_emoji;
    vec![
        IndexItem {
            name: format!("{app_label} {app_version}"),
            path: "Application version".to_string(),
            icon: if emoji { "\u{1F4E6}" } else { "[VER]" }.to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            keywords: "kmd:settings:noop".to_string(),
            icon_path: None,
        },
        IndexItem {
            name: format!("kmd-core {}", crate::Index::current_version()),
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
    ]
}

/// 검색창 입력을 프리픽스 종류로 분류한다.
pub fn prefix_of(query: &str) -> QueryPrefix {
    if query.starts_with('@') {
        return QueryPrefix::Web;
    }
    if query.starts_with('!') {
        return QueryPrefix::Shell;
    }
    if query.starts_with(':') {
        for (prefix, aliases) in COMMAND_ALIASES {
            if matches_command(query, aliases) {
                return *prefix;
            }
        }
    }
    QueryPrefix::General
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigil_prefixes() {
        assert_eq!(prefix_of("@g rust"), QueryPrefix::Web);
        assert_eq!(prefix_of("@"), QueryPrefix::Web);
        assert_eq!(prefix_of("!ip"), QueryPrefix::Shell);
    }

    #[test]
    fn command_aliases_match_on_token_boundary() {
        assert_eq!(prefix_of(":t spell hello"), QueryPrefix::Transform);
        assert_eq!(prefix_of(":transform"), QueryPrefix::Transform);
        assert_eq!(prefix_of(":pt"), QueryPrefix::Prompt);
        assert_eq!(prefix_of(":prompt add x y"), QueryPrefix::Prompt);
        assert_eq!(prefix_of(":calc (2+3)*4"), QueryPrefix::Calc);
        assert_eq!(prefix_of(":e fire"), QueryPrefix::Emoji);
        assert_eq!(prefix_of(":emoji"), QueryPrefix::Emoji);
        assert_eq!(prefix_of(":set"), QueryPrefix::Settings);
        assert_eq!(prefix_of(":settings theme"), QueryPrefix::Settings);
        assert_eq!(prefix_of(":h"), QueryPrefix::Help);
        assert_eq!(prefix_of(":version"), QueryPrefix::Version);
        assert_eq!(prefix_of(":km on"), QueryPrefix::Keymap);
        assert_eq!(prefix_of(":k"), QueryPrefix::Keys);
        assert_eq!(prefix_of(":f /tmp report"), QueryPrefix::FolderSearch);
    }

    #[test]
    fn no_false_prefix_matches() {
        // 토큰 경계 규칙: 별칭 뒤에 공백 없이 글자가 이어지면 명령이 아니다
        assert_eq!(prefix_of(":pto"), QueryPrefix::General);
        assert_eq!(prefix_of(":setup"), QueryPrefix::General);
        assert_eq!(prefix_of(":verbose"), QueryPrefix::General);
        assert_eq!(prefix_of(":ex"), QueryPrefix::General);
        assert_eq!(prefix_of(":helpme"), QueryPrefix::General);
        assert_eq!(prefix_of(":keysx"), QueryPrefix::General);
    }

    #[test]
    fn general_queries() {
        assert_eq!(prefix_of("firefox"), QueryPrefix::General);
        assert_eq!(prefix_of("*.pdf"), QueryPrefix::General);
        assert_eq!(prefix_of("/test\\d+/"), QueryPrefix::General);
        assert_eq!(prefix_of(":"), QueryPrefix::General);
        assert_eq!(prefix_of(""), QueryPrefix::General);
    }
}
