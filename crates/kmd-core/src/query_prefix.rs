//! 검색창 특수 프리픽스 분류 — TUI와 데스크톱이 공유하는 단일 파서.
//!
//! 별칭 매칭 규칙: **완전일치 또는 별칭 바로 뒤 공백**(토큰 경계).
//! 예: `:pt hello` → Prompt, `:pto` → General (별칭 오인 방지),
//! `:settings` → Settings, `:setup` → General.

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
