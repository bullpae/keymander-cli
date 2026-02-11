//! Web service integration — @prefix queries for web searches

use crate::index::{IndexItem, ItemKind, Source};

/// Built-in web service definition
pub struct WebService {
    pub name: &'static str,
    pub prefixes: &'static [&'static str],
    pub icon: &'static str,
    pub url_template: &'static str,
    pub description: &'static str,
}

/// Built-in web services (10 services)
pub const WEB_SERVICES: &[WebService] = &[
    WebService {
        name: "Google",
        prefixes: &["@g", "@google"],
        icon: "\u{1F50D}",
        url_template: "https://google.com/search?q={query}",
        description: "Search Google",
    },
    WebService {
        name: "YouTube",
        prefixes: &["@yt", "@youtube"],
        icon: "\u{1F3AC}",
        url_template: "https://youtube.com/results?search_query={query}",
        description: "Search YouTube",
    },
    WebService {
        name: "GitHub",
        prefixes: &["@gh", "@github"],
        icon: "\u{1F419}",
        url_template: "https://github.com/search?q={query}",
        description: "Search GitHub",
    },
    WebService {
        name: "StackOverflow",
        prefixes: &["@so", "@stackoverflow"],
        icon: "\u{1F4DA}",
        url_template: "https://stackoverflow.com/search?q={query}",
        description: "Search StackOverflow",
    },
    WebService {
        name: "npm",
        prefixes: &["@npm"],
        icon: "\u{1F4E6}",
        url_template: "https://www.npmjs.com/search?q={query}",
        description: "Search npm packages",
    },
    WebService {
        name: "crates.io",
        prefixes: &["@crates", "@cargo"],
        icon: "\u{1F980}",
        url_template: "https://crates.io/search?q={query}",
        description: "Search Rust crates",
    },
    WebService {
        name: "Wikipedia",
        prefixes: &["@w", "@wiki"],
        icon: "\u{1F4D8}",
        url_template: "https://en.wikipedia.org/wiki/Special:Search/{query}",
        description: "Search Wikipedia",
    },
    WebService {
        name: "X (Twitter)",
        prefixes: &["@x", "@twitter"],
        icon: "\u{1F426}",
        url_template: "https://x.com/search?q={query}",
        description: "Search X (Twitter)",
    },
    WebService {
        name: "Google Maps",
        prefixes: &["@map", "@maps"],
        icon: "\u{1F5FA}\u{FE0F}",
        url_template: "https://maps.google.com/maps?q={query}",
        description: "Search Google Maps",
    },
    WebService {
        name: "Naver Dict",
        prefixes: &["@dict", "@naver"],
        icon: "\u{1F4D6}",
        url_template: "https://dict.naver.com/search?query={query}",
        description: "Search Naver Dictionary",
    },
    // AI Services
    WebService {
        name: "Perplexity",
        prefixes: &["@ai", "@pplx", "@perplexity"],
        icon: "\u{1F916}",
        url_template: "https://www.perplexity.ai/search?q={query}",
        description: "Ask Perplexity AI",
    },
    WebService {
        name: "ChatGPT",
        prefixes: &["@gpt", "@chatgpt"],
        icon: "\u{1F4AC}",
        url_template: "https://chatgpt.com/?q={query}",
        description: "Ask ChatGPT",
    },
    WebService {
        name: "Claude",
        prefixes: &["@claude"],
        icon: "\u{1F9E0}",
        url_template: "https://claude.ai/new?q={query}",
        description: "Ask Claude AI",
    },
    WebService {
        name: "Gemini",
        prefixes: &["@gemini"],
        icon: "\u{2728}",
        url_template: "https://gemini.google.com/app?q={query}",
        description: "Ask Google Gemini",
    },
];

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

/// Build a search URL with query encoding
pub fn build_search_url(service: &WebService, query: &str) -> String {
    let encoded = url_encode(query);
    service.url_template.replace("{query}", &encoded)
}

/// List all web services as IndexItems (for @ prefix browsing)
pub fn list_services_as_items(filter: &str) -> Vec<IndexItem> {
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
                icon: s.icon.to_string(),
                keywords: format!(
                    "{} {}",
                    s.prefixes.join(" "),
                    s.description
                ),
            }
        })
        .collect()
}

/// Create a search result item for a specific web query
pub fn search_result_item(service: &WebService, query: &str) -> IndexItem {
    let url = build_search_url(service, query);
    IndexItem {
        name: format!("{}: \"{}\"", service.name, query),
        path: url.clone(),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: service.icon.to_string(),
        keywords: url,
    }
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
}
