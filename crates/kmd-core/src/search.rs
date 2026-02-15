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
        if q.starts_with('.')
            && q.len() >= 2
            && q[1..].chars().all(|c| c.is_ascii_alphanumeric())
        {
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
            lowercase_cache: LowercaseCache { entries: Vec::new() },
            kind_weights: KindWeights::default(),
        }
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
    fn apply_kind_boost(&self, results: &mut Vec<SearchResult>) {
        for result in results.iter_mut() {
            let boost = self.kind_weights.weight_for(result.item.kind);
            result.score = result.score.saturating_add(boost);
        }
        results.sort_by(|a, b| b.score.cmp(&a.score));
    }

    /// Fuzzy search using Nucleo
    fn search_fuzzy(&mut self, pattern: &str, limit: usize) -> Vec<SearchResult> {
        self.nucleo.pattern.reparse(
            0,
            pattern,
            CaseMatching::Smart,
            Normalization::Never,
            false,
        );
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
                score: (count as u32).saturating_sub(i as u32) * 10,
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
    fn filter_contains(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();

        self.all_items
            .iter()
            .zip(self.lowercase_cache.entries.iter())
            .filter(|(_, lc)| {
                lc.name.contains(&query_lower)
                    || lc.path.contains(&query_lower)
                    || lc.keywords.contains(&query_lower)
            })
            .take(limit)
            .map(|(item, _)| SearchResult {
                item: item.clone(),
                score: 0,
            })
            .collect()
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
    tld_part.len() >= 2
        && tld_part.len() <= 6
        && tld_part.chars().all(|c| c.is_ascii_alphabetic())
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
        assert_eq!(
            SearchMode::detect("https://example.com").0,
            SearchMode::Url
        );
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
}
