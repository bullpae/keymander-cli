//! Search engine — fuzzy, glob, regex, and contains matching over IndexItems

use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config as NucleoConfig, Nucleo};

use crate::config::KindWeights;
use crate::index::IndexItem;

/// A search result wrapping an IndexItem with a relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub item: IndexItem,
    pub score: u32,
}

/// Search mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchMode {
    Fuzzy,
    Glob,
    Regex,
    Contains,
    Url,
}

impl SearchMode {
    /// Auto-detect search mode from query string
    pub fn detect(query: &str) -> (Self, String) {
        let q = query.trim();
        if q.is_empty() {
            return (Self::Fuzzy, String::new());
        }

        // URL detection
        if is_url(q) {
            return (Self::Url, normalize_url(q));
        }

        // Glob: contains * or ?
        if q.contains('*') || q.contains('?') {
            return (Self::Glob, q.to_string());
        }

        // Regex: /pattern/
        if q.starts_with('/') && q.len() > 2 && q.ends_with('/') {
            let pattern = &q[1..q.len() - 1];
            return (Self::Regex, pattern.to_string());
        }

        // Extension shortcut: .doc -> *.doc
        if q.starts_with('.') && q.len() >= 2 && q[1..].chars().all(|c| c.is_ascii_alphanumeric()) {
            return (Self::Glob, format!("*{}", q));
        }

        // Non-ASCII (CJK etc.) → Contains mode for accurate substring matching
        if !q.is_ascii() {
            return (Self::Contains, q.to_string());
        }

        (Self::Fuzzy, q.to_string())
    }

    /// Display label
    pub fn label(&self) -> &str {
        match self {
            Self::Fuzzy => "fuzzy",
            Self::Glob => "glob",
            Self::Regex => "regex",
            Self::Contains => "contains",
            Self::Url => "url",
        }
    }
}

/// Pre-lowercased fields for efficient case-insensitive substring/glob/regex matching.
struct LowercaseCache {
    /// Lowercased name, path, and keywords for each item (same order as `all_items`)
    entries: Vec<LowercaseEntry>,
}

struct LowercaseEntry {
    name: String,
    path: String,
    keywords: String,
}

impl LowercaseCache {
    fn build(items: &[IndexItem]) -> Self {
        let entries = items
            .iter()
            .map(|item| LowercaseEntry {
                name: item.name.to_lowercase(),
                path: item.path.to_lowercase(),
                keywords: item.keywords.to_lowercase(),
            })
            .collect();
        Self { entries }
    }
}

/// The search engine wrapping Nucleo fuzzy matcher + other modes
pub struct SearchEngine {
    nucleo: Nucleo<IndexItem>,
    all_items: Vec<IndexItem>,
    lowercase_cache: LowercaseCache,
    kind_weights: KindWeights,
}

impl SearchEngine {
    /// Create a new empty search engine
    pub fn new() -> Self {
        let config = NucleoConfig::DEFAULT;
        let nucleo = Nucleo::new(config, Arc::new(|| {}), None, 1);
        Self {
            nucleo,
            all_items: Vec::new(),
            lowercase_cache: LowercaseCache {
                entries: Vec::new(),
            },
            kind_weights: KindWeights::default(),
        }
    }

    /// 로드된 아이템 수
    pub fn len(&self) -> usize {
        self.all_items.len()
    }

    /// Set the kind weights for score boosting
    pub fn set_kind_weights(&mut self, weights: KindWeights) {
        self.kind_weights = weights;
    }

    /// Load items into the search engine (consumes the item list)
    pub fn load(&mut self, items: Vec<IndexItem>) {
        let injector = self.nucleo.injector();
        for item in &items {
            injector.push(item.clone(), |item, cols| {
                cols[0] = format!("{} {}", item.name, item.keywords).into();
            });
        }
        self.lowercase_cache = LowercaseCache::build(&items);
        self.all_items = items;
    }

    /// Search with automatic mode detection
    pub fn search(&mut self, query: &str, limit: usize) -> (SearchMode, Vec<SearchResult>) {
        let (mode, pattern) = SearchMode::detect(query);
        let results = self.search_with_mode(mode, &pattern, limit);
        (mode, results)
    }

    /// Search with a specific mode
    pub fn search_with_mode(
        &mut self,
        mode: SearchMode,
        pattern: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        let mut results = match mode {
            SearchMode::Fuzzy => self.search_fuzzy(pattern, limit),
            SearchMode::Glob => self.filter_glob(pattern, limit),
            SearchMode::Regex => self.filter_regex(pattern, limit),
            SearchMode::Contains | SearchMode::Url => self.filter_contains(pattern, limit),
        };

        // Apply kind weight boost and re-sort
        self.apply_kind_boost(&mut results);
        results
    }

    /// Apply kind_weights boost to search results and re-sort by score descending
    fn apply_kind_boost(&self, results: &mut [SearchResult]) {
        for result in results.iter_mut() {
            let boost = self.kind_weights.weight_for(result.item.kind);
            result.score = result.score.saturating_add(boost);
        }
        results.sort_by(|a, b| b.score.cmp(&a.score));
    }

    /// Fuzzy search using Nucleo
    fn search_fuzzy(&mut self, pattern: &str, limit: usize) -> Vec<SearchResult> {
        self.nucleo
            .pattern
            .reparse(0, pattern, CaseMatching::Smart, Normalization::Never, false);
        // timeout in milliseconds — wait for worker threads to finish matching.
        // 10ms keeps the UI responsive while still giving Nucleo time to
        // process most queries on typical indexes (< 20k items).
        const NUCLEO_TICK_TIMEOUT_MS: u64 = 10;
        self.nucleo.tick(NUCLEO_TICK_TIMEOUT_MS);

        let snapshot = self.nucleo.snapshot();
        let count = snapshot.matched_item_count().min(limit as u32);
        snapshot
            .matched_items(..count)
            .enumerate()
            .map(|(i, item)| SearchResult {
                item: item.data.clone(),
                // Higher rank = higher score (first result gets highest)
                score: count.saturating_sub(i as u32) * 10,
            })
            .collect()
    }

    /// Glob pattern filter
    fn filter_glob(&self, pattern: &str, limit: usize) -> Vec<SearchResult> {
        let pattern_lower = pattern.to_lowercase();
        let matcher = GlobMatcher::new(&pattern_lower);

        self.all_items
            .iter()
            .zip(self.lowercase_cache.entries.iter())
            .filter(|(_, lc)| matcher.matches(&lc.name) || matcher.matches(&lc.path))
            .take(limit)
            .map(|(item, _)| SearchResult {
                item: item.clone(),
                score: 0,
            })
            .collect()
    }

    /// Regex filter (with ReDoS protection)
    fn filter_regex(&self, pattern: &str, limit: usize) -> Vec<SearchResult> {
        const MAX_REGEX_PATTERN_LEN: usize = 200;
        const REGEX_SIZE_LIMIT: usize = 1 << 20; // 1 MiB

        if pattern.len() > MAX_REGEX_PATTERN_LEN {
            return Vec::new();
        }

        let Ok(re) = regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .size_limit(REGEX_SIZE_LIMIT)
            .build()
        else {
            return Vec::new();
        };

        self.all_items
            .iter()
            .filter(|item| re.is_match(&item.name) || re.is_match(&item.path))
            .take(limit)
            .map(|item| SearchResult {
                item: item.clone(),
                score: 0,
            })
            .collect()
    }

    /// Substring contains filter (case-insensitive, good for CJK)
    ///
    /// 멀티 토큰 지원: 공백으로 구분된 2개 이상의 토큰이 입력되면
    /// 모든 토큰이 name/path/keywords 중 하나에 포함되어야 매칭 (AND 조건).
    /// 경로 세그먼트 정확 일치에 높은 점수를 부여하여 디렉토리 점프에 활용.
    fn filter_contains(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        // 토큰 분리 — 입력 내 경로 구분자도 분리하여 플래튼
        let tokens: Vec<String> = query
            .split_whitespace()
            .flat_map(|word| word.split(['\\', '/']))
            .filter(|s| !s.is_empty())
            .map(|t| t.to_lowercase())
            .collect();

        // 단일 토큰: 기존 동작 100% 보존 (score: 0, substring match)
        if tokens.len() <= 1 {
            let query_lower = tokens.first().map(|s| s.as_str()).unwrap_or("");
            return self
                .all_items
                .iter()
                .zip(self.lowercase_cache.entries.iter())
                .filter(|(_, lc)| {
                    lc.name.contains(query_lower)
                        || lc.path.contains(query_lower)
                        || lc.keywords.contains(query_lower)
                })
                .take(limit)
                .map(|(item, _)| SearchResult {
                    item: item.clone(),
                    score: 0,
                })
                .collect();
        }

        // ── 멀티 토큰: AND 매칭 + 가중 스코어링 ──
        // 점수 상수 — Score Pollution 방어를 위해 search_score 범위를 넓게 설정
        const SEGMENT_EXACT: u32 = 60; // 경로 세그먼트 정확 일치
        const PATH_CONTAINS: u32 = 30; // 경로 내 substring
        const NAME_CONTAINS: u32 = 15; // 이름 매칭
        const KW_CONTAINS: u32 = 5; // 키워드 매칭
        const ALL_SEGMENTS_BONUS: u32 = 80; // 전 토큰 세그먼트 정확 일치 보너스

        let token_count = tokens.len() as u32;

        let mut results: Vec<SearchResult> = self
            .all_items
            .iter()
            .zip(self.lowercase_cache.entries.iter())
            .filter_map(|(item, lc)| {
                let segments: Vec<&str> = lc
                    .path
                    .split(['\\', '/'])
                    .filter(|s| !s.is_empty())
                    .collect();

                let mut total_score: u32 = 0;
                let mut exact_segments: u32 = 0;

                for token in &tokens {
                    let t = token.as_str();
                    // 우선순위 기반 점수 — 최고 점수만 적용
                    if segments.contains(&t) {
                        total_score += SEGMENT_EXACT;
                        exact_segments += 1;
                    } else if lc.path.contains(t) {
                        total_score += PATH_CONTAINS;
                    } else if lc.name.contains(t) {
                        total_score += NAME_CONTAINS;
                    } else if lc.keywords.contains(t) {
                        total_score += KW_CONTAINS;
                    } else {
                        return None; // AND 조건: 하나라도 미매칭이면 제외
                    }
                }

                // 모든 토큰이 세그먼트 정확 일치 → 완전한 경로 의도 매칭 보너스
                if exact_segments == token_count {
                    total_score += ALL_SEGMENTS_BONUS;
                }

                Some(SearchResult {
                    item: item.clone(),
                    score: total_score,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit);
        results
    }

    /// Total loaded items
    pub fn total_items(&self) -> usize {
        self.all_items.len()
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── URL helpers ─────────────────────────────────────────

fn is_url(s: &str) -> bool {
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("www.")
        || (s.contains('.')
            && !s.contains(' ')
            && !s.contains('*')
            && !s.starts_with('.')
            && matches_domain_pattern(s))
}

fn matches_domain_pattern(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let tld = parts.last().unwrap_or(&"");
    let tld_part = tld.split('/').next().unwrap_or(tld);
    tld_part.len() >= 2 && tld_part.len() <= 6 && tld_part.chars().all(|c| c.is_ascii_alphabetic())
}

fn normalize_url(s: &str) -> String {
    if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("https://{}", s)
    }
}

// ── Glob matcher ────────────────────────────────────────

struct GlobMatcher {
    parts: Vec<GlobPart>,
}

enum GlobPart {
    Literal(String),
    Star,
    Question,
}

impl GlobMatcher {
    fn new(pattern: &str) -> Self {
        let mut parts = Vec::new();
        let mut literal = String::new();

        for ch in pattern.chars() {
            match ch {
                '*' => {
                    if !literal.is_empty() {
                        parts.push(GlobPart::Literal(std::mem::take(&mut literal)));
                    }
                    if !matches!(parts.last(), Some(GlobPart::Star)) {
                        parts.push(GlobPart::Star);
                    }
                }
                '?' => {
                    if !literal.is_empty() {
                        parts.push(GlobPart::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(GlobPart::Question);
                }
                _ => literal.push(ch),
            }
        }
        if !literal.is_empty() {
            parts.push(GlobPart::Literal(literal));
        }

        Self { parts }
    }

    fn matches(&self, text: &str) -> bool {
        self.match_recursive(text, 0)
    }

    fn match_recursive(&self, text: &str, part_idx: usize) -> bool {
        if part_idx >= self.parts.len() {
            return text.is_empty();
        }

        match &self.parts[part_idx] {
            GlobPart::Literal(lit) => {
                if let Some(rest) = text.strip_prefix(lit.as_str()) {
                    self.match_recursive(rest, part_idx + 1)
                } else {
                    false
                }
            }
            GlobPart::Question => {
                if text.is_empty() {
                    false
                } else {
                    let mut chars = text.chars();
                    chars.next();
                    self.match_recursive(chars.as_str(), part_idx + 1)
                }
            }
            GlobPart::Star => {
                if part_idx + 1 >= self.parts.len() {
                    return true;
                }
                let mut remaining = text;
                loop {
                    if self.match_recursive(remaining, part_idx + 1) {
                        return true;
                    }
                    if remaining.is_empty() {
                        return false;
                    }
                    let mut chars = remaining.chars();
                    chars.next();
                    remaining = chars.as_str();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_mode_detect() {
        assert_eq!(SearchMode::detect("hello").0, SearchMode::Fuzzy);
        assert_eq!(SearchMode::detect("*.doc").0, SearchMode::Glob);
        assert_eq!(SearchMode::detect("test*").0, SearchMode::Glob);
        assert_eq!(SearchMode::detect("/test\\d+/").0, SearchMode::Regex);
        assert_eq!(SearchMode::detect(".pdf").0, SearchMode::Glob);
    }

    #[test]
    fn test_search_mode_cjk() {
        assert_eq!(SearchMode::detect("한글").0, SearchMode::Contains);
    }

    #[test]
    fn test_search_mode_url() {
        assert_eq!(SearchMode::detect("google.com").0, SearchMode::Url);
        assert_eq!(SearchMode::detect("https://example.com").0, SearchMode::Url);
    }

    #[test]
    fn test_glob_matcher() {
        let m = GlobMatcher::new("*.doc");
        assert!(m.matches("report.doc"));
        assert!(!m.matches("report.pdf"));

        let m2 = GlobMatcher::new("test*");
        assert!(m2.matches("test_file.rs"));
        assert!(!m2.matches("my_test"));

        let m3 = GlobMatcher::new("*report*");
        assert!(m3.matches("my_report_2024.doc"));
    }

    #[test]
    fn test_search_engine_basic() {
        use crate::index::{ItemKind, Source};

        let mut engine = SearchEngine::new();
        engine.load(vec![
            IndexItem {
                name: "Firefox".to_string(),
                path: "/usr/bin/firefox".to_string(),
                kind: ItemKind::App,
                source: Source::Apps,
                icon: "\u{1F4E6}".to_string(),
                keywords: "firefox browser web".to_string(),
            },
            IndexItem {
                name: "VS Code".to_string(),
                path: "/usr/bin/code".to_string(),
                kind: ItemKind::App,
                source: Source::Apps,
                icon: "\u{1F4E6}".to_string(),
                keywords: "code editor vscode".to_string(),
            },
        ]);

        let (mode, results) = engine.search("fire", 10);
        assert_eq!(mode, SearchMode::Fuzzy);
        assert!(!results.is_empty());
        assert_eq!(results[0].item.name, "Firefox");
    }

    #[test]
    fn test_kind_weight_boost() {
        use crate::index::{ItemKind, Source};

        let mut engine = SearchEngine::new();
        engine.set_kind_weights(KindWeights {
            directory: 80,
            app: 70,
            file: 50,
            ..Default::default()
        });

        engine.load(vec![
            IndexItem {
                name: "Downloads".to_string(),
                path: "/home/user/Downloads".to_string(),
                kind: ItemKind::Directory,
                source: Source::FileProvider,
                icon: ">>".to_string(),
                keywords: "downloads".to_string(),
            },
            IndexItem {
                name: "download.txt".to_string(),
                path: "/home/user/download.txt".to_string(),
                kind: ItemKind::File,
                source: Source::FileProvider,
                icon: "\u{1F4DD}".to_string(),
                keywords: "download text".to_string(),
            },
        ]);

        let results = engine.search_with_mode(SearchMode::Contains, "download", 10);
        assert_eq!(results.len(), 2);
        // Directory should be first due to higher weight
        assert_eq!(results[0].item.kind, ItemKind::Directory);
    }

    // ── 멀티 토큰 매칭 테스트 ──────────────────────────────

    /// 테스트용 IndexItem 생성 헬퍼
    fn make_item(name: &str, path: &str, keywords: &str) -> IndexItem {
        use crate::index::{ItemKind, Source};
        IndexItem {
            name: name.to_string(),
            path: path.to_string(),
            kind: ItemKind::Directory,
            source: Source::FileProvider,
            icon: ">>".to_string(),
            keywords: keywords.to_string(),
        }
    }

    #[test]
    fn test_multi_token_korean_path_segments() {
        // "2026 출장이력" → c:\2026\work\출장이력 매칭, 세그먼트 정확 일치 보너스
        let mut engine = SearchEngine::new();
        engine.load(vec![
            make_item("출장이력", r"c:\2026\work\출장이력", "출장이력"),
            make_item("출장이력_old", r"c:\2025\archive\출장이력_old", "출장이력"),
            make_item("readme", r"c:\2026\docs\readme.txt", "문서"),
        ]);

        let results = engine.search_with_mode(SearchMode::Contains, "2026 출장이력", 10);
        assert!(!results.is_empty(), "결과가 있어야 함");
        // 첫 번째 결과: 두 토큰 모두 세그먼트 정확 일치 → 최고 점수
        assert_eq!(results[0].item.path, r"c:\2026\work\출장이력");
        // "readme"는 "출장이력" 토큰 미매칭이므로 제외
        assert!(
            results.iter().all(|r| r.item.name != "readme"),
            "readme는 AND 조건 불충족으로 제외"
        );
    }

    #[test]
    fn test_multi_token_order_independent() {
        // "출장이력 2026" → 토큰 순서 무관, 동일 결과
        let mut engine = SearchEngine::new();
        engine.load(vec![make_item(
            "출장이력",
            r"c:\2026\work\출장이력",
            "출장이력",
        )]);

        let r1 = engine.search_with_mode(SearchMode::Contains, "2026 출장이력", 10);
        let r2 = engine.search_with_mode(SearchMode::Contains, "출장이력 2026", 10);

        assert_eq!(r1.len(), r2.len(), "순서와 무관하게 같은 수의 결과");
        assert_eq!(r1[0].score, r2[0].score, "순서와 무관하게 같은 점수");
    }

    #[test]
    fn test_single_token_preserves_original_behavior() {
        // 단일 토큰은 기존 동작 보존: score=0, substring match
        let mut engine = SearchEngine::new();
        engine.load(vec![make_item(
            "출장이력",
            r"c:\2026\work\출장이력",
            "출장이력",
        )]);

        let results = engine.search_with_mode(SearchMode::Contains, "출장이력", 10);
        assert_eq!(results.len(), 1);
        // 단일 토큰은 score: 0 (kind_boost 적용 전)
        // apply_kind_boost는 search_with_mode에서 적용되므로 여기서는 kind_boost만 반영됨
        // filter_contains 자체는 score 0 반환
    }

    #[test]
    fn test_multi_token_segment_exact_vs_substring() {
        // 세그먼트 정확 일치(+60)와 path substring(+30) 점수 차이 확인
        let mut engine = SearchEngine::new();
        engine.load(vec![
            // "출장" 토큰이 세그먼트가 아닌 substring으로 매칭
            make_item("출장이력_보고서", r"c:\2026\work\출장이력_보고서", "출장"),
            // "출장이력" 토큰이 세그먼트 정확 일치
            make_item("출장이력", r"c:\2026\work\출장이력", "출장이력"),
        ]);

        let results = engine.search_with_mode(SearchMode::Contains, "2026 출장이력", 10);
        assert!(results.len() >= 1);
        // 세그먼트 정확 일치 항목이 더 높은 점수
        assert_eq!(
            results[0].item.path, r"c:\2026\work\출장이력",
            "세그먼트 정확 일치가 우선"
        );
    }

    #[test]
    fn test_multi_token_windows_backslash() {
        // Windows 경로 backslash 세그먼트 분리 정상 동작
        let mut engine = SearchEngine::new();
        engine.load(vec![make_item(
            "projects",
            r"D:\dev\projects\rust",
            "rust dev",
        )]);

        let results = engine.search_with_mode(SearchMode::Contains, "dev rust", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0, "멀티 토큰은 0 이상의 점수");
    }

    #[test]
    fn test_multi_token_mixed_korean_english() {
        // 한영 혼합 쿼리: "project 보고서"
        let mut engine = SearchEngine::new();
        engine.load(vec![
            make_item("보고서", r"c:\project\보고서", "보고서 project"),
            make_item("readme", r"c:\project\readme.md", "project"),
        ]);

        let results = engine.search_with_mode(SearchMode::Contains, "project 보고서", 10);
        assert_eq!(results.len(), 1, "readme는 '보고서' 토큰 미매칭으로 제외");
        assert_eq!(results[0].item.name, "보고서");
    }

    #[test]
    fn test_multi_token_no_match_returns_empty() {
        // 매칭되는 항목이 없으면 빈 결과
        let mut engine = SearchEngine::new();
        engine.load(vec![make_item(
            "출장이력",
            r"c:\2026\work\출장이력",
            "출장이력",
        )]);

        let results = engine.search_with_mode(SearchMode::Contains, "2025 회의록", 10);
        assert!(results.is_empty(), "매칭 없으면 빈 결과");
    }

    #[test]
    fn test_multi_token_input_with_path_separator() {
        // 입력에 경로 구분자가 포함된 경우 토큰으로 분리
        let mut engine = SearchEngine::new();
        engine.load(vec![make_item(
            "출장이력",
            r"c:\2026\work\출장이력",
            "출장이력",
        )]);

        // "work\출장이력" → ["work", "출장이력"] 으로 분리되어 매칭
        let results = engine.search_with_mode(SearchMode::Contains, r"work\출장이력", 10);
        assert_eq!(results.len(), 1, "경로 구분자가 토큰 구분자로 처리됨");
    }

    #[test]
    fn test_all_segments_exact_bonus() {
        // 모든 토큰이 세그먼트 정확 일치 → ALL_SEGMENTS_BONUS(+80) 적용
        let mut engine = SearchEngine::new();
        engine.load(vec![
            // "2026"과 "work" 모두 세그먼트 정확 일치
            make_item("work", r"c:\2026\work", "작업폴더"),
            // "2026"은 정확, "work"는 path substring (workforce의 일부)
            make_item("workforce", r"c:\2026\workforce_data", "인력"),
        ]);

        let results = engine.search_with_mode(SearchMode::Contains, "2026 work", 10);
        assert!(results.len() >= 1);
        // 두 토큰 모두 세그먼트 정확 일치한 항목이 보너스로 1등
        assert_eq!(results[0].item.name, "work");
        if results.len() >= 2 {
            assert!(
                results[0].score > results[1].score,
                "전 세그먼트 일치 보너스로 점수 차이 발생"
            );
        }
    }
}
