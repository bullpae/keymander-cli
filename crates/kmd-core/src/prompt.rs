//! 프롬프트 템플릿 관리 — 저장/조회/적용

use crate::config::PromptTemplate;
use crate::index::{IndexItem, ItemKind, Source};

/// 저장된 템플릿 목록을 IndexItem으로 변환 (`:prompt` 검색용)
pub fn list_templates_as_items(
    templates: &[PromptTemplate],
    filter: &str,
    use_emoji: bool,
) -> Vec<IndexItem> {
    let filter_lower = filter.to_lowercase();
    let mut items: Vec<IndexItem> = templates
        .iter()
        .filter(|t| {
            filter_lower.is_empty()
                || t.name.to_lowercase().contains(&filter_lower)
                || t.body.to_lowercase().contains(&filter_lower)
        })
        .map(|t| IndexItem {
            name: format!(":{:<16} {}", t.name, preview_body(&t.body, 60)),
            path: format!("@ll :{} <query> 형태로 사용", t.name),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: if use_emoji { "\u{1F4DD}" } else { "Pt" }.to_string(),
            keywords: format!("prompt template {}", t.name),
            icon_path: None,
        })
        .collect();

    // 빈 목록이면 안내 항목 추가
    if items.is_empty() && filter_lower.is_empty() {
        items.push(IndexItem {
            name: "저장된 템플릿 없음".to_string(),
            path: ":prompt add <name> <body>  또는  kmd prompt add <name> <body>".to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: if use_emoji { "\u{2139}\u{FE0F}" } else { "[i]" }.to_string(),
            keywords: "prompt template empty".to_string(),
            icon_path: None,
        });
    }

    // :prompt add 힌트
    if filter_lower.is_empty() || "add".contains(&filter_lower) {
        items.push(IndexItem {
            name: ":prompt add <name> <body>".to_string(),
            path: "새 프롬프트 템플릿 추가 (예: :prompt add review 코드를 리뷰해주세요)"
                .to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: if use_emoji { "\u{2795}" } else { "[+]" }.to_string(),
            keywords: "kmd:prompt:add".to_string(),
            icon_path: None,
        });
    }

    items
}

/// `@ll :template_name actual question` 에서 템플릿 적용 결과 반환.
/// 템플릿이 없으면 원본 쿼리를 그대로 반환.
pub fn apply_template(templates: &[PromptTemplate], query: &str) -> String {
    let trimmed = query.trim();
    if !trimmed.starts_with(':') {
        return query.to_string();
    }

    // `:name rest...` 파싱
    let (name_part, rest) = match trimmed.find(char::is_whitespace) {
        Some(pos) => (&trimmed[1..pos], trimmed[pos..].trim()),
        None => (&trimmed[1..], ""),
    };

    let Some(template) = templates
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name_part))
    else {
        return query.to_string();
    };

    if template.body.contains("{query}") {
        template.body.replace("{query}", rest)
    } else if rest.is_empty() {
        template.body.clone()
    } else {
        format!("{}\n\n{}", template.body, rest)
    }
}

/// 긴 텍스트를 지정 길이로 잘라 미리보기 문자열 생성
pub fn preview_body(text: &str, max_len: usize) -> String {
    if text.len() > max_len {
        let truncated = max_len.saturating_sub(3);
        format!("{}...", &text[..truncated])
    } else {
        text.to_string()
    }
}

/// 템플릿 이름 유효성 검사 (영문/숫자/하이픈/언더스코어)
pub fn validate_template_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_templates() -> Vec<PromptTemplate> {
        vec![
            PromptTemplate {
                name: "review".to_string(),
                body: "다음 코드를 리뷰해주세요:\n{query}".to_string(),
            },
            PromptTemplate {
                name: "translate".to_string(),
                body: "한국어로 번역해주세요".to_string(),
            },
        ]
    }

    #[test]
    fn test_apply_template_with_placeholder() {
        let templates = sample_templates();
        let result = apply_template(&templates, ":review fn main() {}");
        assert_eq!(result, "다음 코드를 리뷰해주세요:\nfn main() {}");
    }

    #[test]
    fn test_apply_template_without_placeholder() {
        let templates = sample_templates();
        let result = apply_template(&templates, ":translate hello world");
        assert_eq!(result, "한국어로 번역해주세요\n\nhello world");
    }

    #[test]
    fn test_apply_template_not_found() {
        let templates = sample_templates();
        let result = apply_template(&templates, ":unknown test");
        assert_eq!(result, ":unknown test");
    }

    #[test]
    fn test_apply_template_no_prefix() {
        let templates = sample_templates();
        let result = apply_template(&templates, "plain query");
        assert_eq!(result, "plain query");
    }

    #[test]
    fn test_validate_template_name() {
        assert!(validate_template_name("review"));
        assert!(validate_template_name("code-review"));
        assert!(validate_template_name("my_template_1"));
        assert!(!validate_template_name(""));
        assert!(!validate_template_name("with spaces"));
        assert!(!validate_template_name("한글이름"));
    }

    #[test]
    fn test_list_templates_empty() {
        let items = list_templates_as_items(&[], "", false);
        assert!(items.iter().any(|i| i.name.contains("저장된 템플릿 없음")));
    }

    #[test]
    fn test_list_templates_with_data() {
        let templates = sample_templates();
        let items = list_templates_as_items(&templates, "", false);
        assert!(items.iter().any(|i| i.name.contains("review")));
        assert!(items.iter().any(|i| i.name.contains("translate")));
    }
}
