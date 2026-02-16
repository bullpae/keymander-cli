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

/// Built-in spelling service definition.
pub struct SpellService {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub emoji_icon: &'static str,
    pub url_template: &'static str,
    pub description: &'static str,
}

/// Built-in translation service definition.
pub struct TranslateService {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub emoji_icon: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateDirection {
    Auto,
    EnToKo,
    KoToEn,
}

/// Built-in web services
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
        id: "naver_search",
        name: "Naver",
        prefixes: &["@naver", "@kr"],
        icon: "Nv",
        emoji_icon: "\u{1F1F0}\u{1F1F7}", // 🇰🇷
        url_template: "https://search.naver.com/search.naver?query={query}",
        description: "Search Naver",
    },
    WebService {
        id: "daum",
        name: "Daum",
        prefixes: &["@daum", "@dm"],
        icon: "Dm",
        emoji_icon: "\u{1F310}", // 🌐
        url_template: "https://search.daum.net/search?w=tot&q={query}",
        description: "Search Daum",
    },
    WebService {
        id: "naver_dict",
        name: "Naver Dict",
        prefixes: &["@dict", "@ndict"],
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

pub const SPELL_SERVICES: &[SpellService] = &[
    SpellService {
        id: "naver_spell",
        name: "Naver Spell",
        icon: "Sp",
        emoji_icon: "\u{270D}\u{FE0F}", // ✍️
        url_template: "https://search.naver.com/search.naver?query={query}+맞춤법+검사",
        description: "Korean spelling check via Naver",
    },
    SpellService {
        id: "pusan_spell",
        name: "Pusan Spell",
        icon: "Ps",
        emoji_icon: "\u{1F4D6}", // 📖
        url_template: "https://search.naver.com/search.naver?query=부산대+맞춤법+{query}",
        description: "Open Pusan spell checker search",
    },
];

pub const TRANSLATE_SERVICES: &[TranslateService] = &[
    TranslateService {
        id: "google_translate",
        name: "Google Translate",
        icon: "Tr",
        emoji_icon: "\u{1F310}", // 🌐
        description: "Translate with Google",
    },
    TranslateService {
        id: "papago",
        name: "Papago",
        icon: "Pg",
        emoji_icon: "\u{1F1F0}\u{1F1F7}", // 🇰🇷
        description: "Translate with Papago",
    },
    TranslateService {
        id: "deepl",
        name: "DeepL",
        icon: "Dl",
        emoji_icon: "\u{1F4D8}", // 📘
        description: "Translate with DeepL",
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

impl SpellService {
    pub fn pick_icon(&self, use_emoji: bool) -> &str {
        if use_emoji {
            self.emoji_icon
        } else {
            self.icon
        }
    }
}

impl TranslateService {
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
    parse_multi_llm_query_with_prefixes(input, selected_ids, &[])
}

/// Parse multi-LLM query with user-defined aliases.
pub fn parse_multi_llm_query_with_prefixes(
    input: &str,
    selected_ids: &[String],
    prefixes: &[String],
) -> Option<(Vec<&'static WebService>, String)> {
    if !input.starts_with('@') {
        return None;
    }
    let first_space = input.find(' ');
    let prefix = input[..first_space.unwrap_or(input.len())].to_lowercase();
    let aliases = normalized_aliases(prefixes, &["@llm", "@ll", "@multi", "@cmp", "@compare"]);
    if !aliases.iter().any(|p| p == &prefix) {
        return None;
    }
    let query = first_space
        .map(|i| input[i + 1..].trim())
        .unwrap_or("")
        .to_string();
    Some((selected_llm_services(selected_ids), query))
}

/// Parse `@msearch` / `@multisearch` / `@searchall` / `@krsearch` query.
pub fn parse_multi_web_query(
    input: &str,
    selected_ids: &[String],
) -> Option<(Vec<&'static WebService>, String)> {
    parse_multi_web_query_with_prefixes(input, selected_ids, &[])
}

/// Parse multi-web query with user-defined aliases.
pub fn parse_multi_web_query_with_prefixes(
    input: &str,
    selected_ids: &[String],
    prefixes: &[String],
) -> Option<(Vec<&'static WebService>, String)> {
    if !input.starts_with('@') {
        return None;
    }
    let first_space = input.find(' ');
    let prefix = input[..first_space.unwrap_or(input.len())].to_lowercase();
    let aliases = normalized_aliases(
        prefixes,
        &[
            "@m",
            "@mw",
            "@msearch",
            "@multisearch",
            "@searchall",
            "@krsearch",
        ],
    );
    if !aliases.iter().any(|p| p == &prefix) {
        return None;
    }
    let query = first_space
        .map(|i| input[i + 1..].trim())
        .unwrap_or("")
        .to_string();
    Some((selected_multi_web_services(selected_ids), query))
}

/// Parse spelling query with user-defined aliases.
pub fn parse_spell_query_with_prefixes(input: &str, prefixes: &[String]) -> Option<String> {
    if !input.starts_with('@') {
        return None;
    }
    let first_space = input.find(' ');
    let prefix = input[..first_space.unwrap_or(input.len())].to_lowercase();
    let aliases = normalized_aliases(prefixes, &["@sp", "@spell"]);
    if !aliases.iter().any(|p| p == &prefix) {
        return None;
    }
    Some(
        first_space
            .map(|i| input[i + 1..].trim())
            .unwrap_or("")
            .to_string(),
    )
}

/// Parse translate query with user-defined aliases.
pub fn parse_translate_query_with_prefixes(
    input: &str,
    prefixes: &[String],
) -> Option<(TranslateDirection, String)> {
    if !input.starts_with('@') {
        return None;
    }
    let first_space = input.find(' ');
    let prefix = input[..first_space.unwrap_or(input.len())].to_lowercase();
    let aliases = normalized_aliases(prefixes, &["@tr", "@trko", "@tren"]);
    if !aliases.iter().any(|p| p == &prefix) {
        return None;
    }
    let direction = if prefix.ends_with("ko") {
        TranslateDirection::EnToKo
    } else if prefix.ends_with("en") {
        TranslateDirection::KoToEn
    } else {
        TranslateDirection::Auto
    };
    let query = first_space
        .map(|i| input[i + 1..].trim())
        .unwrap_or("")
        .to_string();
    Some((direction, query))
}

/// Build a search URL with query encoding
pub fn build_search_url(service: &WebService, query: &str) -> String {
    let encoded = url_encode(query);
    service.url_template.replace("{query}", &encoded)
}

/// List all web services as IndexItems (for @ prefix browsing)
pub fn list_services_as_items(filter: &str, use_emoji: bool) -> Vec<IndexItem> {
    let filter_lower = filter.to_lowercase();

    let mut items: Vec<IndexItem> = WEB_SERVICES
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
        .collect();

    // Virtual entry for multi-LLM compare so users discover `@llm` from `@` list.
    let multi_match = filter_lower.is_empty()
        || "@llm".contains(&filter_lower)
        || "multi llm compare".contains(&filter_lower)
        || "@multi".contains(&filter_lower)
        || "@cmp".contains(&filter_lower);
    if multi_match {
        items.push(IndexItem {
            name: "@ll         Compare multiple LLMs with one prompt".to_string(),
            path: "Open selected LLM providers (some may require paste/Enter)".to_string(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: if use_emoji { "\u{1F9E0}" } else { "Ml" }.to_string(),
            keywords: "@ll @llm @multi @cmp multi llm compare".to_string(),
        });
    }

    // Virtual entry for multi-web compare (Google/Naver/Daum).
    let multi_web_match = filter_lower.is_empty()
        || "@msearch".contains(&filter_lower)
        || "@multisearch".contains(&filter_lower)
        || "@searchall".contains(&filter_lower)
        || "@krsearch".contains(&filter_lower)
        || "multi web search".contains(&filter_lower);
    if multi_web_match {
        items.push(IndexItem {
            name: "@m          Search multiple engines at once".to_string(),
            path: "Open Google/Naver/Daum in parallel tabs".to_string(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: if use_emoji { "\u{1F50E}" } else { "Mw" }.to_string(),
            keywords: "@m @mw @msearch @multisearch @searchall @krsearch multi web".to_string(),
        });
    }

    let spell_match = filter_lower.is_empty()
        || "@sp".contains(&filter_lower)
        || "@spell".contains(&filter_lower)
        || "spelling".contains(&filter_lower);
    if spell_match {
        items.push(IndexItem {
            name: "@sp         Korean spelling check".to_string(),
            path: "Run spelling check providers with one query".to_string(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: if use_emoji { "\u{270D}\u{FE0F}" } else { "Sp" }.to_string(),
            keywords: "@sp @spell spelling checker".to_string(),
        });
    }

    let translate_match = filter_lower.is_empty()
        || "@tr".contains(&filter_lower)
        || "@trko".contains(&filter_lower)
        || "@tren".contains(&filter_lower)
        || "translate".contains(&filter_lower);
    if translate_match {
        items.push(IndexItem {
            name: "@tr         Translate (auto / ko / en)".to_string(),
            path: "Open selected translate providers in parallel".to_string(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: if use_emoji { "\u{1F5E3}\u{FE0F}" } else { "Tr" }.to_string(),
            keywords: "@tr @trko @tren translate".to_string(),
        });
    }

    items
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
        path: format!(
            "Open all selected LLMs for: \"{}\" (some providers need paste/Enter)",
            query
        ),
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

/// Build result items for multi-web search (`@msearch`).
pub fn multi_web_result_items(
    query: &str,
    selected_ids: &[String],
    use_emoji: bool,
) -> Vec<IndexItem> {
    let services = selected_multi_web_services(selected_ids);
    if query.is_empty() {
        return services
            .into_iter()
            .map(|s| IndexItem {
                name: format!("@msearch {:<12} {}", s.id, s.description),
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
    let marker = format!("kmd:multi_web_urls\n{}", urls.join("\n"));

    items.push(IndexItem {
        name: format!("Multi Web search ({})", services.len()),
        path: format!("Open selected search engines for: \"{}\"", query),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: if use_emoji { "\u{1F50E}" } else { "Mw" }.to_string(),
        keywords: format!("{}\nproviders: {}", marker, provider_names),
    });

    for service in services {
        items.push(search_result_item(service, query, use_emoji));
    }
    items
}

pub fn spell_result_items(query: &str, selected_ids: &[String], use_emoji: bool) -> Vec<IndexItem> {
    let services = selected_spell_services(selected_ids);
    if query.is_empty() {
        return services
            .into_iter()
            .map(|s| IndexItem {
                name: format!("@sp {:<14} {}", s.id, s.description),
                path: s.url_template.to_string(),
                kind: ItemKind::WebSearch,
                source: Source::Plugin,
                icon: s.pick_icon(use_emoji).to_string(),
                keywords: format!("{} spell", s.id),
            })
            .collect();
    }

    let mut items = Vec::new();
    let urls: Vec<String> = services
        .iter()
        .map(|svc| svc.url_template.replace("{query}", &url_encode(query)))
        .collect();
    let provider_names = services
        .iter()
        .map(|svc| svc.name)
        .collect::<Vec<_>>()
        .join(", ");
    let marker = format!("kmd:spell_urls\n{}", urls.join("\n"));
    items.push(IndexItem {
        name: format!("Spell check ({})", services.len()),
        path: format!("Run spelling check for: \"{}\"", query),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: if use_emoji { "\u{270D}\u{FE0F}" } else { "Sp" }.to_string(),
        keywords: format!("{}\nproviders: {}", marker, provider_names),
    });
    for service in services {
        let url = service.url_template.replace("{query}", &url_encode(query));
        items.push(IndexItem {
            name: format!("{}: \"{}\"", service.name, query),
            path: url.clone(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: service.pick_icon(use_emoji).to_string(),
            keywords: url,
        });
    }
    items
}

pub fn translate_result_items(
    query: &str,
    direction: TranslateDirection,
    selected_ids: &[String],
    use_emoji: bool,
) -> Vec<IndexItem> {
    let services = selected_translate_services(selected_ids);
    if query.is_empty() {
        return services
            .into_iter()
            .map(|s| IndexItem {
                name: format!("@tr {:<16} {}", s.id, s.description),
                path: s.description.to_string(),
                kind: ItemKind::WebSearch,
                source: Source::Plugin,
                icon: s.pick_icon(use_emoji).to_string(),
                keywords: format!("{} translate", s.id),
            })
            .collect();
    }

    let mut items = Vec::new();
    let urls: Vec<String> = services
        .iter()
        .map(|svc| build_translate_url(svc, query, direction))
        .collect();
    let provider_names = services
        .iter()
        .map(|svc| svc.name)
        .collect::<Vec<_>>()
        .join(", ");
    let marker = format!("kmd:translate_urls\n{}", urls.join("\n"));
    items.push(IndexItem {
        name: format!("Translate ({})", services.len()),
        path: format!("Translate: \"{}\"", query),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: if use_emoji { "\u{1F5E3}\u{FE0F}" } else { "Tr" }.to_string(),
        keywords: format!("{}\nproviders: {}", marker, provider_names),
    });
    for service in services {
        let url = build_translate_url(service, query, direction);
        items.push(IndexItem {
            name: format!("{}: \"{}\"", service.name, query),
            path: url.clone(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: service.pick_icon(use_emoji).to_string(),
            keywords: url,
        });
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

/// Decode multi-web URLs from an item generated by `multi_web_result_items`.
pub fn extract_multi_web_urls(item: &IndexItem) -> Option<Vec<String>> {
    let prefix = "kmd:multi_web_urls\n";
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

pub fn extract_spell_urls(item: &IndexItem) -> Option<Vec<String>> {
    let prefix = "kmd:spell_urls\n";
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

pub fn extract_translate_urls(item: &IndexItem) -> Option<Vec<String>> {
    let prefix = "kmd:translate_urls\n";
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

/// Resolve configured engine IDs for `@msearch` multi-web search.
pub fn selected_multi_web_services(selected_ids: &[String]) -> Vec<&'static WebService> {
    let mut matched = Vec::new();
    for id in selected_ids {
        let normalized = id.trim().to_lowercase();
        if normalized.is_empty() || !is_multi_web_id(&normalized) {
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
            .filter(|svc| is_multi_web_id(svc.id))
            .collect()
    } else {
        matched
    }
}

pub fn selected_spell_services(selected_ids: &[String]) -> Vec<&'static SpellService> {
    let mut matched = Vec::new();
    for id in selected_ids {
        let normalized = id.trim().to_lowercase();
        if normalized.is_empty() || !is_spell_id(&normalized) {
            continue;
        }
        if let Some(service) = SPELL_SERVICES
            .iter()
            .find(|svc| svc.id.eq_ignore_ascii_case(&normalized))
        {
            matched.push(service);
        }
    }
    if matched.is_empty() {
        SPELL_SERVICES.iter().collect()
    } else {
        matched
    }
}

pub fn selected_translate_services(selected_ids: &[String]) -> Vec<&'static TranslateService> {
    let mut matched = Vec::new();
    for id in selected_ids {
        let normalized = id.trim().to_lowercase();
        if normalized.is_empty() || !is_translate_id(&normalized) {
            continue;
        }
        if let Some(service) = TRANSLATE_SERVICES
            .iter()
            .find(|svc| svc.id.eq_ignore_ascii_case(&normalized))
        {
            matched.push(service);
        }
    }
    if matched.is_empty() {
        TRANSLATE_SERVICES.iter().collect()
    } else {
        matched
    }
}

fn is_llm_id(id: &str) -> bool {
    matches!(id, "chatgpt" | "gemini" | "claude" | "grok" | "perplexity")
}

fn is_multi_web_id(id: &str) -> bool {
    matches!(id, "google" | "naver_search" | "daum")
}

fn is_spell_id(id: &str) -> bool {
    matches!(id, "naver_spell" | "pusan_spell")
}

fn is_translate_id(id: &str) -> bool {
    matches!(id, "google_translate" | "papago" | "deepl")
}

fn build_translate_url(
    service: &TranslateService,
    query: &str,
    direction: TranslateDirection,
) -> String {
    let encoded = url_encode(query);
    let (sl, tl) = match direction {
        TranslateDirection::Auto => ("auto", "ko"),
        TranslateDirection::EnToKo => ("en", "ko"),
        TranslateDirection::KoToEn => ("ko", "en"),
    };
    match service.id {
        "google_translate" => {
            format!("https://translate.google.com/?sl={sl}&tl={tl}&text={encoded}&op=translate")
        }
        "papago" => format!("https://papago.naver.com/?sk={sl}&tk={tl}&st={encoded}"),
        "deepl" => format!("https://www.deepl.com/translator#{sl}/{tl}/{encoded}"),
        _ => format!("https://translate.google.com/?sl={sl}&tl={tl}&text={encoded}&op=translate"),
    }
}

fn normalized_aliases(aliases: &[String], defaults: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = if aliases.is_empty() {
        defaults.iter().map(|s| (*s).to_string()).collect()
    } else {
        aliases
            .iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.starts_with('@') {
                    s
                } else {
                    format!("@{s}")
                }
            })
            .collect()
    };
    out.dedup();
    if out.is_empty() {
        defaults.iter().map(|s| (*s).to_string()).collect()
    } else {
        out
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
    fn test_parse_multi_llm_custom_prefix() {
        let selected = vec!["chatgpt".to_string()];
        let prefixes = vec!["@askllm".to_string()];
        let parsed =
            parse_multi_llm_query_with_prefixes("@askllm rust async", &selected, &prefixes);
        assert!(parsed.is_some());
        let (_, q) = parsed.unwrap();
        assert_eq!(q, "rust async");
    }

    #[test]
    fn test_parse_multi_web_query() {
        let selected = vec!["google".to_string(), "daum".to_string()];
        let parsed = parse_multi_web_query("@msearch rust ownership", &selected);
        assert!(parsed.is_some());
        let (services, q) = parsed.unwrap();
        assert_eq!(q, "rust ownership");
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].id, "google");
        assert_eq!(services[1].id, "daum");
    }

    #[test]
    fn test_parse_multi_web_custom_prefix() {
        let selected = vec!["google".to_string()];
        let prefixes = vec!["@searchx".to_string()];
        let parsed = parse_multi_web_query_with_prefixes("@searchx rust", &selected, &prefixes);
        assert!(parsed.is_some());
        let (_, q) = parsed.unwrap();
        assert_eq!(q, "rust");
    }

    #[test]
    fn test_parse_spell_query() {
        let prefixes = vec!["@sp".to_string(), "@spell".to_string()];
        let parsed = parse_spell_query_with_prefixes("@sp 안녕 하세요", &prefixes);
        assert_eq!(parsed, Some("안녕 하세요".to_string()));
    }

    #[test]
    fn test_parse_translate_query() {
        let prefixes = vec!["@tr".to_string(), "@trko".to_string(), "@tren".to_string()];
        let parsed = parse_translate_query_with_prefixes("@trko hello world", &prefixes).unwrap();
        assert_eq!(parsed.0, TranslateDirection::EnToKo);
        assert_eq!(parsed.1, "hello world");
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

    #[test]
    fn test_multi_web_item_urls_roundtrip() {
        let selected = vec!["google".to_string(), "naver_search".to_string()];
        let items = multi_web_result_items("러스트 소유권", &selected, false);
        assert!(!items.is_empty());
        let urls = extract_multi_web_urls(&items[0]).unwrap();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("%EB%9F%AC%EC%8A%A4%ED%8A%B8"));
    }

    #[test]
    fn test_spell_item_urls_roundtrip() {
        let selected = vec!["naver_spell".to_string(), "pusan_spell".to_string()];
        let items = spell_result_items("문장 검사", &selected, false);
        let urls = extract_spell_urls(&items[0]).unwrap();
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_translate_item_urls_roundtrip() {
        let selected = vec!["google_translate".to_string(), "papago".to_string()];
        let items = translate_result_items("hello", TranslateDirection::EnToKo, &selected, false);
        let urls = extract_translate_urls(&items[0]).unwrap();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("sl=en"));
    }

    #[test]
    fn test_list_services_contains_llm_hint() {
        let items = list_services_as_items("", false);
        assert!(items
            .iter()
            .any(|item| item.name.starts_with("@ll") || item.name.starts_with("@llm")));
    }
}
