//! 결과 아이템 빌더 — IndexItem 생성, URL 추출, 서비스 목록

use crate::index::{IndexItem, ItemKind, Source};

use super::services::*;

// ── URL 빌드 ──────────────────────────────────────────────────────────────────

/// `@` 브라우징용 가상 항목 — `keywords` 첫 줄이 `kmd:web_hint:*` 이면 입력창에 넣을 프리픽스
pub fn web_browse_hint_seed(keywords: &str) -> Option<&'static str> {
    let first = keywords.lines().next()?.trim();
    match first {
        "kmd:web_hint:multi_llm" => Some("@llm "),
        "kmd:web_hint:multi_web" => Some("@msearch "),
        "kmd:web_hint:spell" => Some("@sp "),
        "kmd:web_hint:translate" => Some("@tr "),
        _ => None,
    }
}

/// 쿼리를 인코딩해 검색 URL 생성
pub fn build_search_url(service: &WebService, query: &str) -> String {
    let encoded = url_encode(query);
    service.url_template.replace("{query}", &encoded)
}

/// 번역 서비스 URL 생성
pub fn build_translate_url(
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

// ── @ 목록 / 검색 결과 아이템 ────────────────────────────────────────────────

/// @ 프리픽스 브라우징용 서비스 목록
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
                icon_path: None,
            }
        })
        .collect();

    // 가상 항목 — @ll, @m, @sp, @tr 발견용 (path는 안내 문구이므로 URL로 열리면 안 됨)
    let virtual_entries: &[(&str, &str, &str, &str, &str, &[&str])] = &[
        (
            "kmd:web_hint:multi_llm",
            "@ll         Compare multiple LLMs with one prompt",
            "Open selected LLM providers (some may require paste/Enter)",
            if use_emoji { "\u{1F9E0}" } else { "Ml" },
            "@ll @llm @multi @cmp multi llm compare",
            &["@llm", "multi llm compare", "@multi", "@cmp"],
        ),
        (
            "kmd:web_hint:multi_web",
            "@m          Search multiple engines at once",
            "Open Google/Naver/Daum in parallel tabs",
            if use_emoji { "\u{1F50E}" } else { "Mw" },
            "@m @mw @msearch @multisearch @searchall @krsearch multi web",
            &[
                "@msearch",
                "@multisearch",
                "@searchall",
                "@krsearch",
                "multi web search",
            ],
        ),
        (
            "kmd:web_hint:spell",
            "@sp         Korean spelling check",
            "Run spelling check providers with one query",
            if use_emoji { "\u{270D}\u{FE0F}" } else { "Sp" },
            "@sp @spell spelling checker",
            &["@sp", "@spell", "spelling"],
        ),
        (
            "kmd:web_hint:translate",
            "@tr         Translate (auto / ko / en)",
            "Open selected translate providers in parallel",
            if use_emoji { "\u{1F5E3}\u{FE0F}" } else { "Tr" },
            "@tr @trko @tren translate",
            &["@tr", "@trko", "@tren", "translate"],
        ),
    ];

    for (hint, name, path, icon, keywords, match_terms) in virtual_entries {
        let matched =
            filter_lower.is_empty() || match_terms.iter().any(|t| t.contains(&filter_lower));
        if matched {
            items.push(IndexItem {
                name: name.to_string(),
                path: path.to_string(),
                kind: ItemKind::WebSearch,
                source: Source::Plugin,
                icon: icon.to_string(),
                keywords: format!("{hint}\n{keywords}"),
                icon_path: None,
            });
        }
    }

    items
}

/// 단일 서비스 검색 결과 아이템
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
        icon_path: None,
    }
}

// ── 멀티 결과 아이템 빌더 ────────────────────────────────────────────────────

pub fn multi_llm_result_items(
    query: &str,
    selected_ids: &[String],
    use_emoji: bool,
) -> Vec<IndexItem> {
    let services = selected_llm_services(selected_ids);
    if query.is_empty() {
        return services_to_browse_items(
            &services,
            "@llm",
            9,
            use_emoji,
            |s| s.url_template.to_string(),
            |s| format!("{} {}", s.id, s.prefixes.join(" ")),
        );
    }
    build_batch_items(
        services
            .iter()
            .map(|svc| build_search_url(svc, query))
            .collect(),
        &services.iter().map(|s| s.name).collect::<Vec<_>>(),
        "kmd:multi_llm_urls",
        &format!("Multi LLM compare ({})", services.len()),
        &format!(
            "Open all selected LLMs for: \"{}\" (some providers need paste/Enter)",
            query
        ),
        if use_emoji { "\u{1F9E0}" } else { "Ml" },
        services
            .iter()
            .map(|s| search_result_item(s, query, use_emoji))
            .collect(),
    )
}

pub fn multi_web_result_items(
    query: &str,
    selected_ids: &[String],
    use_emoji: bool,
) -> Vec<IndexItem> {
    let services = selected_multi_web_services(selected_ids);
    if query.is_empty() {
        return services_to_browse_items(
            &services,
            "@msearch",
            12,
            use_emoji,
            |s| s.url_template.to_string(),
            |s| format!("{} {}", s.id, s.prefixes.join(" ")),
        );
    }
    build_batch_items(
        services
            .iter()
            .map(|svc| build_search_url(svc, query))
            .collect(),
        &services.iter().map(|s| s.name).collect::<Vec<_>>(),
        "kmd:multi_web_urls",
        &format!("Multi Web search ({})", services.len()),
        &format!("Open selected search engines for: \"{}\"", query),
        if use_emoji { "\u{1F50E}" } else { "Mw" },
        services
            .iter()
            .map(|s| search_result_item(s, query, use_emoji))
            .collect(),
    )
}

pub fn spell_result_items(query: &str, selected_ids: &[String], use_emoji: bool) -> Vec<IndexItem> {
    let services = selected_spell_services(selected_ids);
    if query.is_empty() {
        return services_to_browse_items(
            &services,
            "@sp",
            14,
            use_emoji,
            |s| s.url_template.to_string(),
            |s| format!("{} spell", s.id),
        );
    }
    let urls: Vec<String> = services
        .iter()
        .map(|svc| svc.url_template.replace("{query}", &url_encode(query)))
        .collect();
    let individual: Vec<IndexItem> = services
        .iter()
        .zip(urls.iter())
        .map(|(svc, url)| IndexItem {
            name: format!("{}: \"{}\"", svc.name, query),
            path: url.clone(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: svc.pick_icon(use_emoji).to_string(),
            keywords: url.clone(),
            icon_path: None,
        })
        .collect();
    build_batch_items(
        urls,
        &services.iter().map(|s| s.name).collect::<Vec<_>>(),
        "kmd:spell_urls",
        &format!("Spell check ({})", services.len()),
        &format!("Run spelling check for: \"{}\"", query),
        if use_emoji { "\u{270D}\u{FE0F}" } else { "Sp" },
        individual,
    )
}

pub fn translate_result_items(
    query: &str,
    direction: TranslateDirection,
    selected_ids: &[String],
    use_emoji: bool,
) -> Vec<IndexItem> {
    let services = selected_translate_services(selected_ids);
    if query.is_empty() {
        return services_to_browse_items(
            &services,
            "@tr",
            16,
            use_emoji,
            |s| s.description.to_string(),
            |s| format!("{} translate", s.id),
        );
    }
    let urls: Vec<String> = services
        .iter()
        .map(|svc| build_translate_url(svc, query, direction))
        .collect();
    let individual: Vec<IndexItem> = services
        .iter()
        .zip(urls.iter())
        .map(|(svc, url)| IndexItem {
            name: format!("{}: \"{}\"", svc.name, query),
            path: url.clone(),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: svc.pick_icon(use_emoji).to_string(),
            keywords: url.clone(),
            icon_path: None,
        })
        .collect();
    build_batch_items(
        urls,
        &services.iter().map(|s| s.name).collect::<Vec<_>>(),
        "kmd:translate_urls",
        &format!("Translate ({})", services.len()),
        &format!("Translate: \"{}\"", query),
        if use_emoji { "\u{1F5E3}\u{FE0F}" } else { "Tr" },
        individual,
    )
}

// ── URL 추출 ──────────────────────────────────────────────────────────────────

/// 통합 URL 추출 — 4개 extract 함수를 한 번에 시도
pub fn extract_batch_urls(item: &IndexItem) -> Option<Vec<String>> {
    extract_urls_with_prefix(item, "kmd:translate_urls\n")
        .or_else(|| extract_urls_with_prefix(item, "kmd:spell_urls\n"))
        .or_else(|| extract_urls_with_prefix(item, "kmd:multi_web_urls\n"))
        .or_else(|| extract_urls_with_prefix(item, "kmd:multi_llm_urls\n"))
}

pub fn extract_multi_llm_urls(item: &IndexItem) -> Option<Vec<String>> {
    extract_urls_with_prefix(item, "kmd:multi_llm_urls\n")
}

pub fn extract_multi_web_urls(item: &IndexItem) -> Option<Vec<String>> {
    extract_urls_with_prefix(item, "kmd:multi_web_urls\n")
}

pub fn extract_spell_urls(item: &IndexItem) -> Option<Vec<String>> {
    extract_urls_with_prefix(item, "kmd:spell_urls\n")
}

pub fn extract_translate_urls(item: &IndexItem) -> Option<Vec<String>> {
    extract_urls_with_prefix(item, "kmd:translate_urls\n")
}

/// 공통 URL 추출 로직 — 중복 제거
fn extract_urls_with_prefix(item: &IndexItem, prefix: &str) -> Option<Vec<String>> {
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

// ── 내부 헬퍼 ─────────────────────────────────────────────────────────────────

/// empty-query 시 서비스 목록을 IndexItem으로 변환하는 공용 헬퍼
fn services_to_browse_items<T: HasId>(
    services: &[&'static T],
    prefix_label: &str,
    id_width: usize,
    use_emoji: bool,
    path_fn: impl Fn(&T) -> String,
    keywords_fn: impl Fn(&T) -> String,
) -> Vec<IndexItem> {
    services
        .iter()
        .map(|s| IndexItem {
            name: format!(
                "{} {:width$} {}",
                prefix_label,
                s.service_id(),
                s.service_description(),
                width = id_width
            ),
            path: path_fn(s),
            kind: ItemKind::WebSearch,
            source: Source::Plugin,
            icon: s.pick_icon(use_emoji).to_string(),
            keywords: keywords_fn(s),
            icon_path: None,
        })
        .collect()
}

/// 배치 결과 아이템 (첫 번째 = 전체 실행, 이후 = 개별 서비스)
fn build_batch_items(
    urls: Vec<String>,
    provider_names: &[&str],
    marker_prefix: &str,
    name: &str,
    path: &str,
    icon: &str,
    mut individual: Vec<IndexItem>,
) -> Vec<IndexItem> {
    let providers_str = provider_names.join(", ");
    let marker = format!("{}\n{}", marker_prefix, urls.join("\n"));
    let mut items = vec![IndexItem {
        name: name.to_string(),
        path: path.to_string(),
        kind: ItemKind::WebSearch,
        source: Source::Plugin,
        icon: icon.to_string(),
        keywords: format!("{}\nproviders: {}", marker, providers_str),
        icon_path: None,
    }];
    items.append(&mut individual);
    items
}

/// 간단한 URL 퍼센트 인코딩
pub fn url_encode(s: &str) -> String {
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
