//! Web service integration — @prefix queries for web searches

use crate::index::{IndexItem, ItemKind, Source};

/// Built-in web service definition
pub struct WebService {
    pub id: &'static str,
    pub name: &'static str,
    pub prefixes: &'static [&'static str],
    /// ASCII icon (2-char, for legacy terminals)
    pub icon: &'static str,
    /// Emoji icon (for modern terminals)
    pub emoji_icon: &'static str,
    pub url_template: &'static str,
    pub description: &'static str,
}

/// Built-in web services (14 services)
pub const WEB_SERVICES: &[WebService] = &[
    WebService {
        id: "google",
        name: "Google",
        prefixes: &["@g", "@google"],
        icon: "Gg",
        emoji_icon: "\u{1F50D}", // 🔍
        url_template: "https://google.com/search?q={query}",
        description: "Search Google",
    },
    WebService {
        id: "youtube",
        name: "YouTube",
        prefixes: &["@yt", "@youtube"],
        icon: "Yt",
        emoji_icon: "\u{25B6}\u{FE0F}", // ▶️
        url_template: "https://youtube.com/results?search_query={query}",
        description: "Search YouTube",
    },
    WebService {
        id: "github",
        name: "GitHub",
        prefixes: &["@gh", "@github"],
        icon: "Gh",
        emoji_icon: "\u{1F431}", // 🐱
        url_template: "https://github.com/search?q={query}",
        description: "Search GitHub",
    },
    WebService {
        id: "stackoverflow",
        name: "StackOverflow",
        prefixes: &["@so", "@stackoverflow"],
        icon: "So",
        emoji_icon: "\u{1F4DA}", // 📚
        url_template: "https://stackoverflow.com/search?q={query}",
        description: "Search StackOverflow",
    },
    WebService {
        id: "npm",
        name: "npm",
        prefixes: &["@npm"],
        icon: "Np",
        emoji_icon: "\u{1F4E6}", // 📦
        url_template: "https://www.npmjs.com/search?q={query}",
        description: "Search npm packages",
    },
    WebService {
        id: "crates",
        name: "crates.io",
        prefixes: &["@crates", "@cargo"],
        icon: "Cr",
        emoji_icon: "\u{1F4E6}", // 📦
        url_template: "https://crates.io/search?q={query}",
        description: "Search Rust crates",
    },
    WebService {
        id: "wikipedia",
        name: "Wikipedia",
        prefixes: &["@w", "@wiki"],
        icon: "Wi",
        emoji_icon: "\u{1F4D6}", // 📖
        url_template: "https://en.wikipedia.org/wiki/Special:Search/{query}",
        description: "Search Wikipedia",
    },
    WebService {
        id: "x",
        name: "X (Twitter)",
        prefixes: &["@x", "@twitter"],
        icon: " X",
        emoji_icon: "\u{1D54F}", // 𝕏
        url_template: "https://x.com/search?q={query}",
        description: "Search X (Twitter)",
    },
    WebService {
        id: "maps",
        name: "Google Maps",
        prefixes: &["@map", "@maps"],
        icon: "Mp",
        emoji_icon: "\u{1F5FA}", // 🗺
        url_template: "https://maps.google.com/maps?q={query}",
        description: "Search Google Maps",
    },
    WebService {
        id: "naver_dict",
        name: "Naver Dict",
        prefixes: &["@dict", "@naver"],
        icon: "Nv",
        emoji_icon: "\u{1F4D7}", // 📗
        url_template: "https://dict.naver.com/search?query={query}",
        description: "Search Naver Dictionary",
    },
    // AI Services
    WebService {
        id: "perplexity",
        name: "Perplexity",
        prefixes: &["@ai", "@pplx", "@perplexity"],
        icon: "Ai",
        emoji_icon: "\u{1F916}", // 🤖
        url_template: "https://www.perplexity.ai/search?q={query}",
        description: "Ask Perplexity AI",
    },
    WebService {
        id: "chatgpt",
        name: "ChatGPT",
        prefixes: &["@gpt", "@chatgpt"],
        icon: "Gp",
        emoji_icon: "\u{1F4AC}", // 💬
        url_template: "https://chatgpt.com/?q={query}",
        description: "Ask ChatGPT",
    },
    WebService {
        id: "claude",
        name: "Claude",
        prefixes: &["@claude"],
        icon: "Cl",
        emoji_icon: "\u{2728}", // ✨
        url_template: "https://claude.ai/new?q={query}",
        description: "Ask Claude AI",
    },
    WebService {
        id: "gemini",
        name: "Gemini",
        prefixes: &["@gemini"],
        icon: "Gm",
        emoji_icon: "\u{264A}", // ♊
        url_template: "https://gemini.google.com/app?q={query}",
        description: "Ask Google Gemini",
    },
    WebService {
        id: "grok",
        name: "Grok",
        prefixes: &["@grok"],
        icon: "Gr",
        emoji_icon: "\u{1F680}", // 🚀
        url_template: "https://grok.com/?q={query}",
        description: "Ask xAI Grok",
    },
];

impl WebService {
    /// Pick the right icon based on emoji support.
    pub fn pick_icon(&self, use_emoji: bool) -> &str {
        if use_emoji {
            self.emoji_icon
        } else {
            self.icon
        }
    }
}

/// Parse a @prefix query → (service, query_text)
pub fn parse_web_query(input: &str) -> Option<(&'static WebService, String)> {
    if !input.starts_with('@') {
        return None;
    }

    let first_space = input.find(' ');
    let prefix = &input[..first_space.unwrap_or(input.len())];
    let query = first_space
        .map(|i| input[i + 1..].trim())
        .unwrap_or("")
        .to_string();

    WEB_SERVICES
        .iter()
        .find(|s| s.prefixes.contains(&prefix))
        .map(|s| (s, query))
}

/// Parse `@llm` / `@multi` / `@cmp` query and return selected LLM services.
pub fn parse_multi_llm_query(
    input: &str,
    selected_ids: &[String],
) -> Option<(Vec<&'static WebService>, String)> {
    if !input.starts_with('@') {
        return None;
    }
    let first_space = input.find(' ');
    let prefix = &input[..first_space.unwrap_or(input.len())];
    if !matches!(prefix, "@llm" | "@multi" | "@cmp" | "@compare") {
        return None;
    }
    let query = first_space
        .map(|i| input[i + 1..].trim())
        .unwrap_or("")
        .to_string();
    Some((selected_llm_services(selected_ids), query))
}

/// Build a search URL with query encoding
pub fn build_search_url(service: &WebService, query: &str) -> String {
    let encoded = url_encode(query);
    service.url_template.replace("{query}", &encoded)
}

/// List all web services as IndexItems (for @ prefix browsing)
pub fn list_services_as_items(filter: &str, use_emoji: bool) -> Vec<IndexItem> {
    let filter_lower = filter.to_lowercase();

    WEB_SERVICES
        .iter()
        .filter(|s| {
            filter_lower.is_empty()
                || s.name.to_lowercase().contains(&filter_lower)
                || s.prefixes.iter().any(|p| p.contains(&filter_lower))
                || s.description.to_lowercase().contains(&filter_lower)
        })
        .map(|s| {
            let prefix = s.prefixes.first().unwrap_or(&"@");
            IndexItem {
                name: format!("{:<12} {}", prefix, s.description),
                path: s.url_template.to_string(),
                kind: ItemKind::WebSearch,
                source: Source::Plugin,
                icon: s.pick_icon(use_emoji).to_string(),
                keywords: format!("{} {}", s.prefixes.join(" "), s.description),
            }
        })
        .collect()
}

/// Create a search result item for a specific web query
pub fn search_result_item(service: &WebService, query: &str, use_emoji: bool) -> IndexItem {
    let url = build_search_url(service, query);
    let name = format!("{}: \"{}\"", service.name, query);
    IndexItem {
        name,
        path: url.clone(),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: service.pick_icon(use_emoji).to_string(),
        keywords: url,
    }
}

/// Build result items for `@llm` multi-prompt comparison.
pub fn multi_llm_result_items(
    query: &str,
    selected_ids: &[String],
    use_emoji: bool,
) -> Vec<IndexItem> {
    let services = selected_llm_services(selected_ids);
    if query.is_empty() {
        return services
            .into_iter()
            .map(|s| IndexItem {
                name: format!("@llm {:<9} {}", s.id, s.description),
                path: s.url_template.to_string(),
                kind: ItemKind::WebSearch,
                source: Source::Plugin,
                icon: s.pick_icon(use_emoji).to_string(),
                keywords: format!("{} {}", s.id, s.prefixes.join(" ")),
            })
            .collect();
    }

    let mut items = Vec::new();
    let urls: Vec<String> = services
        .iter()
        .map(|svc| build_search_url(svc, query))
        .collect();
    let provider_names = services
        .iter()
        .map(|svc| svc.name)
        .collect::<Vec<_>>()
        .join(", ");
    let marker = format!("kmd:multi_llm_urls\n{}", urls.join("\n"));

    // First item: open every selected LLM at once.
    items.push(IndexItem {
        name: format!("Multi LLM compare ({})", services.len()),
        path: format!("Open all selected LLMs for: \"{}\"", query),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: if use_emoji { "\u{1F9E0}" } else { "Ml" }.to_string(),
        keywords: format!("{}\nproviders: {}", marker, provider_names),
    });

    // Additional items: individual providers.
    for service in services {
        items.push(search_result_item(service, query, use_emoji));
    }
    items
}

/// Decode multi-LLM URLs from an item generated by `multi_llm_result_items`.
pub fn extract_multi_llm_urls(item: &IndexItem) -> Option<Vec<String>> {
    let prefix = "kmd:multi_llm_urls\n";
    if !item.keywords.starts_with(prefix) {
        return None;
    }
    let urls = item.keywords[prefix.len()..]
        .lines()
        .take_while(|line| !line.starts_with("providers:"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if urls.is_empty() {
        None
    } else {
        Some(urls)
    }
}

/// Resolve configured LLM IDs to built-in service definitions.
pub fn selected_llm_services(selected_ids: &[String]) -> Vec<&'static WebService> {
    let mut matched = Vec::new();
    for id in selected_ids {
        let normalized = id.trim().to_lowercase();
        if normalized.is_empty() || !is_llm_id(&normalized) {
            continue;
        }
        if let Some(service) = WEB_SERVICES
            .iter()
            .find(|svc| svc.id.eq_ignore_ascii_case(&normalized))
        {
            matched.push(service);
        }
    }

    if matched.is_empty() {
        WEB_SERVICES
            .iter()
            .filter(|svc| is_llm_id(svc.id))
            .collect()
    } else {
        matched
    }
}

fn is_llm_id(id: &str) -> bool {
    matches!(id, "chatgpt" | "gemini" | "claude" | "grok" | "perplexity")
}

/// Simple URL percent-encoding
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_google() {
        let result = parse_web_query("@g rust tutorial");
        assert!(result.is_some());
        let (service, query) = result.unwrap();
        assert_eq!(service.name, "Google");
        assert_eq!(query, "rust tutorial");
    }

    #[test]
    fn test_parse_no_query() {
        let result = parse_web_query("@yt");
        assert!(result.is_some());
        let (_, query) = result.unwrap();
        assert_eq!(query, "");
    }

    #[test]
    fn test_unknown_prefix() {
        assert!(parse_web_query("@unknown test").is_none());
    }

    #[test]
    fn test_build_url() {
        let service = &WEB_SERVICES[0]; // Google
        let url = build_search_url(service, "rust tutorial");
        assert_eq!(url, "https://google.com/search?q=rust+tutorial");
    }

    #[test]
    fn test_parse_multi_llm_query() {
        let selected = vec!["chatgpt".to_string(), "claude".to_string()];
        let parsed = parse_multi_llm_query("@llm compare rust ownership", &selected);
        assert!(parsed.is_some());
        let (services, q) = parsed.unwrap();
        assert_eq!(q, "compare rust ownership");
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].id, "chatgpt");
        assert_eq!(services[1].id, "claude");
    }

    #[test]
    fn test_multi_llm_item_urls_roundtrip() {
        let selected = vec!["chatgpt".to_string(), "gemini".to_string()];
        let items = multi_llm_result_items("rust lifetimes", &selected, false);
        assert!(!items.is_empty());
        let urls = extract_multi_llm_urls(&items[0]).unwrap();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("rust+lifetimes"));
        assert!(urls[1].contains("rust+lifetimes"));
    }
}
