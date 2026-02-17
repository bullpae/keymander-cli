//! 클립보드 변환 명령 — `:t` 계열
//!
//! `:t spell <text>` → 맞춤법 검사 서비스로 열기
//! `:t trko <text>`  → 한국어 번역 서비스로 열기
//! `:t tren <text>`  → 영어 번역 서비스로 열기
//! `:t tr <text>`    → 자동 번역으로 열기
//!
//! `<text>` 생략 시 클립보드 내용을 자동으로 사용.

use crate::index::{IndexItem, ItemKind, Source};
use crate::web::services::TranslateDirection;

/// 변환 명령의 종류
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformKind {
    Spell,
    Translate(TranslateDirection),
}

/// `:t` 쿼리 파싱 결과
#[derive(Debug, Clone)]
pub struct TransformQuery {
    pub kind: TransformKind,
    /// 사용자 입력 텍스트 (빈 문자열이면 클립보드 사용)
    pub text: String,
}

/// `:t` / `:transform` 쿼리 파싱
pub fn parse_transform_query(input: &str) -> Option<TransformQuery> {
    let sub = input
        .strip_prefix(":transform")
        .or_else(|| input.strip_prefix(":t"))
        .map(|s| s.trim())?;

    // `:t` 만 입력 → 도움말 모드 (None)
    if sub.is_empty() {
        return None;
    }

    let (cmd, rest) = match sub.find(char::is_whitespace) {
        Some(pos) => (&sub[..pos], sub[pos..].trim()),
        None => (sub, ""),
    };

    let kind = match cmd.to_lowercase().as_str() {
        "spell" | "sp" => TransformKind::Spell,
        "trko" | "ko" => TransformKind::Translate(TranslateDirection::EnToKo),
        "tren" | "en" => TransformKind::Translate(TranslateDirection::KoToEn),
        "tr" | "auto" => TransformKind::Translate(TranslateDirection::Auto),
        _ => return None,
    };

    Some(TransformQuery {
        kind,
        text: rest.to_string(),
    })
}

/// `:t` 명령 도움말 항목 생성
pub fn help_items(use_emoji: bool) -> Vec<IndexItem> {
    let entries: &[(&str, &str, &str)] = &[
        (
            ":t spell <text>",
            "맞춤법 검사 (텍스트 생략 시 클립보드 사용)",
            if use_emoji { "\u{270D}\u{FE0F}" } else { "Sp" },
        ),
        (
            ":t tr <text>",
            "번역 — 자동 감지 (텍스트 생략 시 클립보드 사용)",
            if use_emoji { "\u{1F310}" } else { "Tr" },
        ),
        (
            ":t trko <text>",
            "영어 → 한국어 번역",
            if use_emoji {
                "\u{1F1F0}\u{1F1F7}"
            } else {
                "Ko"
            },
        ),
        (
            ":t tren <text>",
            "한국어 → 영어 번역",
            if use_emoji {
                "\u{1F1FA}\u{1F1F8}"
            } else {
                "En"
            },
        ),
    ];

    entries
        .iter()
        .map(|(name, desc, icon)| IndexItem {
            name: name.to_string(),
            path: desc.to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: icon.to_string(),
            keywords: "transform clipboard".to_string(),
        })
        .collect()
}

/// 변환 결과 URL 목록 생성
pub fn build_transform_urls(
    query: &TransformQuery,
    spell_providers: &[String],
    translate_providers: &[String],
) -> Vec<String> {
    match &query.kind {
        TransformKind::Spell => {
            let services = crate::web::selected_spell_services(spell_providers);
            services
                .iter()
                .map(|svc| {
                    svc.url_template
                        .replace("{query}", &crate::web::url_encode(&query.text))
                })
                .collect()
        }
        TransformKind::Translate(dir) => {
            let services = crate::web::selected_translate_services(translate_providers);
            services
                .iter()
                .map(|svc| crate::web::build_translate_url(svc, &query.text, *dir))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spell() {
        let q = parse_transform_query(":t spell 안녕하세요").unwrap();
        assert_eq!(q.kind, TransformKind::Spell);
        assert_eq!(q.text, "안녕하세요");
    }

    #[test]
    fn test_parse_translate_auto() {
        let q = parse_transform_query(":t tr hello world").unwrap();
        assert_eq!(q.kind, TransformKind::Translate(TranslateDirection::Auto));
        assert_eq!(q.text, "hello world");
    }

    #[test]
    fn test_parse_translate_ko() {
        let q = parse_transform_query(":t trko hello").unwrap();
        assert_eq!(q.kind, TransformKind::Translate(TranslateDirection::EnToKo));
    }

    #[test]
    fn test_parse_empty_returns_none() {
        assert!(parse_transform_query(":t").is_none());
    }

    #[test]
    fn test_parse_unknown_subcmd() {
        assert!(parse_transform_query(":t foobar test").is_none());
    }

    #[test]
    fn test_parse_clipboard_mode() {
        let q = parse_transform_query(":t spell").unwrap();
        assert_eq!(q.kind, TransformKind::Spell);
        assert_eq!(q.text, ""); // 클립보드 모드
    }

    #[test]
    fn test_build_spell_urls() {
        let q = TransformQuery {
            kind: TransformKind::Spell,
            text: "테스트".to_string(),
        };
        let urls = build_transform_urls(&q, &[], &[]);
        assert!(!urls.is_empty());
        assert!(urls[0].contains("%ED%85%8C%EC%8A%A4%ED%8A%B8"));
    }

    #[test]
    fn test_build_translate_urls() {
        let q = TransformQuery {
            kind: TransformKind::Translate(TranslateDirection::EnToKo),
            text: "hello".to_string(),
        };
        let urls = build_transform_urls(&q, &[], &[]);
        assert!(!urls.is_empty());
        assert!(urls[0].contains("sl=en"));
    }

    #[test]
    fn test_help_items() {
        let items = help_items(false);
        assert_eq!(items.len(), 4);
    }
}
