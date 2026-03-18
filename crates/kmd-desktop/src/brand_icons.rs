//! 브랜드 아이콘 매핑 — 서비스 ID → PNG 이미지 Handle
//!
//! kmd-desktop 전용. IndexItem의 keywords/path 필드에서 서비스를 감지하여
//! 해당 브랜드 PNG를 `iced::widget::image::Handle`로 반환한다.
//! 매칭 실패 시 `None` → 기존 이모지 텍스트 fallback.
//!
//! **캐싱**: Handle은 `LazyLock`으로 프로세스 내 1회만 생성되어
//! iced 이미지 파이프라인의 텍스처 캐시가 정상 작동한다.

use std::collections::HashMap;
use std::sync::LazyLock;

use iced::widget::image::Handle;
use kmd_core::ItemKind;

// ── 임베드 PNG 바이트 ────────────────────────────────────────────────────────

const GOOGLE: &[u8] = include_bytes!("../assets/icons/google.png");
const YOUTUBE: &[u8] = include_bytes!("../assets/icons/youtube.png");
const GITHUB: &[u8] = include_bytes!("../assets/icons/github.png");
const STACKOVERFLOW: &[u8] = include_bytes!("../assets/icons/stackoverflow.png");
const NPM: &[u8] = include_bytes!("../assets/icons/npm.png");
const CRATES: &[u8] = include_bytes!("../assets/icons/crates.png");
const WIKIPEDIA: &[u8] = include_bytes!("../assets/icons/wikipedia.png");
const X_ICON: &[u8] = include_bytes!("../assets/icons/x.png");
const MAPS: &[u8] = include_bytes!("../assets/icons/maps.png");
const NAVER: &[u8] = include_bytes!("../assets/icons/naver.png");
const DAUM: &[u8] = include_bytes!("../assets/icons/daum.png");
const NAVER_DICT: &[u8] = include_bytes!("../assets/icons/naver_dict.png");
const PERPLEXITY: &[u8] = include_bytes!("../assets/icons/perplexity.png");
const CHATGPT: &[u8] = include_bytes!("../assets/icons/chatgpt.png");
const CLAUDE: &[u8] = include_bytes!("../assets/icons/claude.png");
const GEMINI: &[u8] = include_bytes!("../assets/icons/gemini.png");
const GROK: &[u8] = include_bytes!("../assets/icons/grok.png");
const PAPAGO: &[u8] = include_bytes!("../assets/icons/papago.png");
const DEEPL: &[u8] = include_bytes!("../assets/icons/deepl.png");
const NAVER_SPELL: &[u8] = include_bytes!("../assets/icons/naver_spell.png");

// ── Handle 캐시 (프로세스 내 1회 초기화) ─────────────────────────────────────

/// 서비스 ID → 캐시된 Handle 맵.
/// `LazyLock`으로 최초 접근 시 1회만 생성되며, 이후 `clone()`은
/// 내부 `Bytes`의 참조 카운트만 증가시켜 사실상 비용이 없다.
static HANDLE_CACHE: LazyLock<HashMap<&'static str, Handle>> = LazyLock::new(|| {
    let entries: &[(&str, &[u8])] = &[
        ("google", GOOGLE),
        ("google_translate", GOOGLE),
        ("youtube", YOUTUBE),
        ("github", GITHUB),
        ("stackoverflow", STACKOVERFLOW),
        ("npm", NPM),
        ("crates", CRATES),
        ("wikipedia", WIKIPEDIA),
        ("x", X_ICON),
        ("maps", MAPS),
        ("naver", NAVER),
        ("naver_search", NAVER),
        ("daum", DAUM),
        ("naver_dict", NAVER_DICT),
        ("perplexity", PERPLEXITY),
        ("chatgpt", CHATGPT),
        ("claude", CLAUDE),
        ("gemini", GEMINI),
        ("grok", GROK),
        ("papago", PAPAGO),
        ("deepl", DEEPL),
        ("naver_spell", NAVER_SPELL),
        ("pusan_spell", NAVER_SPELL),
    ];
    entries
        .iter()
        .map(|(id, bytes)| (*id, Handle::from_bytes(bytes.to_vec())))
        .collect()
});

// ── @prefix → 서비스 ID ──────────────────────────────────────────────────────

fn prefix_to_id(prefix: &str) -> Option<&'static str> {
    match prefix {
        "g" | "google" => Some("google"),
        "yt" | "youtube" => Some("youtube"),
        "gh" | "github" => Some("github"),
        "so" | "stackoverflow" => Some("stackoverflow"),
        "npm" => Some("npm"),
        "crates" | "cargo" => Some("crates"),
        "w" | "wiki" => Some("wikipedia"),
        "x" | "twitter" => Some("x"),
        "map" | "maps" => Some("maps"),
        "naver" | "kr" => Some("naver"),
        "daum" | "dm" => Some("daum"),
        "dict" | "ndict" => Some("naver_dict"),
        "ai" | "pplx" | "perplexity" => Some("perplexity"),
        "gpt" | "chatgpt" => Some("chatgpt"),
        "claude" => Some("claude"),
        "gemini" => Some("gemini"),
        "grok" => Some("grok"),
        _ => None,
    }
}

// ── URL 도메인 → 서비스 ID ───────────────────────────────────────────────────

fn url_to_id(url: &str) -> Option<&'static str> {
    // 더 구체적인 도메인을 먼저 매칭
    if url.contains("gemini.google.com") {
        return Some("gemini");
    }
    if url.contains("maps.google.com") {
        return Some("maps");
    }
    if url.contains("translate.google.com") {
        return Some("google");
    }
    if url.contains("dict.naver.com") {
        return Some("naver_dict");
    }
    if url.contains("papago.naver.com") {
        return Some("papago");
    }
    if url.contains("naver.com") {
        return Some("naver");
    }
    if url.contains("youtube.com") {
        return Some("youtube");
    }
    if url.contains("github.com") {
        return Some("github");
    }
    if url.contains("stackoverflow.com") {
        return Some("stackoverflow");
    }
    if url.contains("npmjs.com") {
        return Some("npm");
    }
    if url.contains("crates.io") {
        return Some("crates");
    }
    if url.contains("wikipedia.org") {
        return Some("wikipedia");
    }
    if url.contains("x.com") {
        return Some("x");
    }
    if url.contains("google.com") {
        return Some("google");
    }
    if url.contains("daum.net") {
        return Some("daum");
    }
    if url.contains("perplexity.ai") {
        return Some("perplexity");
    }
    if url.contains("chatgpt.com") {
        return Some("chatgpt");
    }
    if url.contains("claude.ai") {
        return Some("claude");
    }
    if url.contains("grok.com") {
        return Some("grok");
    }
    if url.contains("deepl.com") {
        return Some("deepl");
    }
    None
}

// ── 서비스 ID 직접 매칭 (browse 목록 토큰용) ────────────────────────────────

fn token_to_id(token: &str) -> Option<&'static str> {
    match token {
        "google" => Some("google"),
        "youtube" => Some("youtube"),
        "github" => Some("github"),
        "stackoverflow" => Some("stackoverflow"),
        "npm" => Some("npm"),
        "crates" => Some("crates"),
        "wikipedia" => Some("wikipedia"),
        "x" => Some("x"),
        "maps" => Some("maps"),
        "naver" | "naver_search" => Some("naver"),
        "daum" => Some("daum"),
        "naver_dict" => Some("naver_dict"),
        "perplexity" => Some("perplexity"),
        "chatgpt" => Some("chatgpt"),
        "claude" => Some("claude"),
        "gemini" => Some("gemini"),
        "grok" => Some("grok"),
        "papago" => Some("papago"),
        "deepl" => Some("deepl"),
        "naver_spell" => Some("naver_spell"),
        "pusan_spell" => Some("pusan_spell"),
        "google_translate" => Some("google_translate"),
        _ => None,
    }
}

// ── 공개 API ──────────────────────────────────────────────────────────────────

/// `IndexItem` 정보를 기반으로 **캐시된** 브랜드 아이콘 Handle 반환.
/// WebSearch 아이템만 처리하며, 매칭 실패 시 `None` (이모지 fallback).
///
/// Handle은 내부적으로 `Bytes`(참조 카운트) 기반이므로 clone 비용이 거의 없고,
/// iced의 텍스처 캐시가 동일 Handle ID를 인식하여 재디코딩 없이 렌더링한다.
pub fn brand_icon_for_item(kind: ItemKind, keywords: &str, path: &str) -> Option<Handle> {
    if kind != ItemKind::WebSearch {
        return None;
    }
    let id = detect_service(keywords, path)?;
    HANDLE_CACHE.get(id).cloned()
}

/// `:setting` 프로바이더 토글 항목용 브랜드 아이콘 반환.
///
/// `kmd:settings:{category}:toggle:{service_id}` 형식의 keywords에서
/// service_id를 추출하여 캐시된 브랜드 아이콘을 반환한다.
pub fn brand_icon_for_settings(keywords: &str) -> Option<Handle> {
    let id = keywords.strip_prefix("kmd:settings:").and_then(|rest| {
        if rest.contains(":toggle:") {
            rest.rsplit(':').next()
        } else {
            None
        }
    })?;
    HANDLE_CACHE.get(id).cloned()
}

/// keywords + path 필드에서 서비스 ID 감지.
///
/// 1단계: keywords 토큰을 순회하면서 @prefix 또는 서비스 ID 직접 매칭.
/// 2단계: path(URL)의 도메인 패턴으로 매칭.
fn detect_service(keywords: &str, path: &str) -> Option<&'static str> {
    // 배치 아이템 빠른 탈출 (kmd:multi_llm_urls 등)
    if keywords.starts_with("kmd:") {
        return None;
    }

    for token in keywords.split_whitespace() {
        if let Some(prefix) = token.strip_prefix('@') {
            let lower = prefix.to_ascii_lowercase();
            if let Some(id) = prefix_to_id(&lower) {
                return Some(id);
            }
        } else {
            let lower = token.to_ascii_lowercase();
            if let Some(id) = token_to_id(&lower) {
                return Some(id);
            }
        }
    }

    // URL 도메인 패턴 매칭
    url_to_id(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_매칭_google() {
        let id = detect_service(
            "@g @google Search Google",
            "https://google.com/search?q={query}",
        );
        assert_eq!(id, Some("google"));
    }

    #[test]
    fn prefix_매칭_chatgpt() {
        let id = detect_service(
            "@gpt @chatgpt Ask ChatGPT",
            "https://chatgpt.com/?q={query}",
        );
        assert_eq!(id, Some("chatgpt"));
    }

    #[test]
    fn browse_모드_서비스id_직접() {
        let id = detect_service("chatgpt @gpt @chatgpt", "https://chatgpt.com/?q={query}");
        assert_eq!(id, Some("chatgpt"));
    }

    #[test]
    fn browse_모드_naver_dict() {
        let id = detect_service(
            "naver_dict @dict @ndict",
            "https://dict.naver.com/search?query={query}",
        );
        assert_eq!(id, Some("naver_dict"));
    }

    #[test]
    fn url_fallback_github() {
        let id = detect_service(
            "https://github.com/search?q=test",
            "https://github.com/search?q=test",
        );
        assert_eq!(id, Some("github"));
    }

    #[test]
    fn url_gemini_우선_google() {
        let id = detect_service(
            "https://gemini.google.com/app?q=test",
            "https://gemini.google.com/app?q=test",
        );
        assert_eq!(id, Some("gemini"));
    }

    #[test]
    fn url_maps_우선_google() {
        let id = detect_service(
            "https://maps.google.com/maps?q=test",
            "https://maps.google.com/maps?q=test",
        );
        assert_eq!(id, Some("maps"));
    }

    #[test]
    fn url_naver_dict_우선_naver() {
        let id = detect_service(
            "https://dict.naver.com/search?query=test",
            "https://dict.naver.com/search?query=test",
        );
        assert_eq!(id, Some("naver_dict"));
    }

    #[test]
    fn url_papago_우선_naver() {
        let id = detect_service(
            "https://papago.naver.com/?sk=auto&tk=ko&st=hello",
            "https://papago.naver.com/?sk=auto&tk=ko&st=hello",
        );
        assert_eq!(id, Some("papago"));
    }

    #[test]
    fn batch_아이템_none() {
        let id = detect_service(
            "kmd:multi_llm_urls\nhttps://chatgpt.com/?q=test",
            "Open all selected LLMs for: \"test\"",
        );
        assert!(id.is_none());
    }

    #[test]
    fn websearch_아닌_kind_none() {
        let result = brand_icon_for_item(ItemKind::App, "some keywords", "C:\\app.exe");
        assert!(result.is_none());
    }

    #[test]
    fn spell_서비스_매칭() {
        let id = detect_service("naver_spell spell", "https://search.naver.com/...");
        assert_eq!(id, Some("naver_spell"));
    }

    #[test]
    fn translate_서비스_매칭() {
        let id = detect_service(
            "google_translate translate",
            "https://translate.google.com/...",
        );
        assert_eq!(id, Some("google_translate"));
    }

    #[test]
    fn settings_llm_toggle_매칭() {
        let h = brand_icon_for_settings("kmd:settings:llm:toggle:chatgpt");
        assert!(
            h.is_some(),
            "chatgpt 프로바이더 토글에 브랜드 아이콘이 있어야 함"
        );
    }

    #[test]
    fn settings_translate_toggle_매칭() {
        let h = brand_icon_for_settings("kmd:settings:translate:toggle:papago");
        assert!(
            h.is_some(),
            "papago 프로바이더 토글에 브랜드 아이콘이 있어야 함"
        );
    }

    #[test]
    fn settings_non_toggle_반환_none() {
        let h = brand_icon_for_settings("kmd:settings:config");
        assert!(h.is_none(), "toggle이 아닌 settings 항목은 None");
    }

    #[test]
    fn 캐시된_handle_동일_id() {
        // 같은 서비스로 두 번 호출해도 동일한 Handle이 반환되는지 확인
        let h1 = brand_icon_for_item(
            ItemKind::WebSearch,
            "@g @google Search Google",
            "https://google.com/search?q={query}",
        );
        let h2 = brand_icon_for_item(
            ItemKind::WebSearch,
            "@g @google Search Google",
            "https://google.com/search?q={query}",
        );
        assert!(h1.is_some());
        assert!(h2.is_some());
        // Handle::from_bytes가 1회만 호출되었으므로 내부 ID가 동일
    }
}
