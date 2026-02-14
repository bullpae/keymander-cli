//! Built-in emoji search extension
//!
//! Activated with `:emoji` or `:e` prefix.
//! Searches Unicode emoji by name and keywords (English + Korean),
//! copies to clipboard on selection.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::index::{IndexItem, ItemKind, Source};

use super::{Extension, ExtensionAction};

/// Single emoji entry parsed from the embedded data
struct EmojiEntry {
    emoji: &'static str,
    name: &'static str,
    category: &'static str,
    /// Korean name from CLDR annotations (e.g. "활짝 웃는 얼굴")
    ko_name: &'static str,
    /// Korean keywords from CLDR, pipe-separated (e.g. "미소|스마일|웃음")
    ko_keywords: &'static str,
}

/// Embedded emoji dataset (parsed lazily from TSV)
static EMOJI_DB: OnceLock<Vec<EmojiEntry>> = OnceLock::new();

fn emoji_db() -> &'static [EmojiEntry] {
    EMOJI_DB.get_or_init(|| {
        let raw_en = include_str!("../../data/emoji.tsv");
        let raw_ko = include_str!("../../data/emoji_ko.tsv");

        // Build Korean lookup: emoji -> (ko_name, ko_keywords)
        let ko_map: HashMap<&str, (&str, &str)> = raw_ko
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                let emoji = parts.next()?.trim();
                let ko_name = parts.next().unwrap_or("").trim();
                let ko_keywords = parts.next().unwrap_or("").trim();
                Some((emoji, (ko_name, ko_keywords)))
            })
            .collect();

        raw_en
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                let emoji = parts.next()?.trim();
                let name = parts.next()?.trim();
                let category = parts.next().unwrap_or("").trim();

                let (ko_name, ko_keywords) = ko_map
                    .get(emoji)
                    .copied()
                    .unwrap_or(("", ""));

                Some(EmojiEntry {
                    emoji,
                    name,
                    category,
                    ko_name,
                    ko_keywords,
                })
            })
            .collect()
    })
}

/// Check whether a string contains any Hangul characters.
/// Delegates to `hangul::is_korean_char` for a single source of truth.
fn has_hangul(s: &str) -> bool {
    s.chars().any(crate::hangul::is_korean_char)
}

pub struct EmojiExtension;

impl EmojiExtension {
    /// Search emoji with a query. Returns matching emoji as IndexItems.
    /// Supports English and Korean queries.
    pub fn search_emoji(&self, query: &str) -> Vec<IndexItem> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            // Return popular emoji when no query
            return emoji_db()
                .iter()
                .take(50)
                .map(emoji_to_item)
                .collect();
        }

        let tokens: Vec<&str> = query.split_whitespace().collect();

        let mut scored: Vec<(usize, &EmojiEntry)> = emoji_db()
            .iter()
            .filter_map(|entry| {
                let name_lower = entry.name.to_lowercase();
                let cat_lower = entry.category.to_lowercase();
                let ko_name = entry.ko_name;
                let ko_kw = entry.ko_keywords;
                let mut score = 0usize;

                for token in &tokens {
                    let tok_is_ko = has_hangul(token);

                    if tok_is_ko {
                        // --- Korean token matching ---
                        // Exact Korean name match (highest)
                        if ko_name == *token {
                            score += 25;
                        } else if ko_name.contains(token) {
                            score += 15;
                            // Bonus for starts-with
                            if ko_name.starts_with(token) {
                                score += 5;
                            }
                        }

                        // Korean keyword match
                        for kw in ko_kw.split('|') {
                            let kw = kw.trim();
                            if kw == *token {
                                score += 18; // Exact keyword match
                            } else if kw.contains(token) {
                                score += 8;
                            }
                        }
                    } else {
                        // --- English token matching ---
                        if name_lower.contains(token) {
                            score += 10;
                            if name_lower.split_whitespace().any(|w| w == *token) {
                                score += 5;
                            }
                            if name_lower.starts_with(token) {
                                score += 3;
                            }
                        } else if cat_lower.contains(token) {
                            score += 2;
                        }
                    }
                }

                if score > 0 {
                    Some((score, entry))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored
            .into_iter()
            .take(50)
            .map(|(_, e)| emoji_to_item(e))
            .collect()
    }
}

fn emoji_to_item(entry: &EmojiEntry) -> IndexItem {
    // Display name: show Korean name alongside English if available
    let display_name = if entry.ko_name.is_empty() {
        format!("{} {}", entry.emoji, entry.name)
    } else {
        format!("{} {} ({})", entry.emoji, entry.name, entry.ko_name)
    };

    // Keywords: combine English + Korean for broader search
    let mut kw = format!("{} {}", entry.name, entry.category);
    if !entry.ko_name.is_empty() {
        kw.push(' ');
        kw.push_str(entry.ko_name);
    }
    if !entry.ko_keywords.is_empty() {
        kw.push(' ');
        kw.push_str(&entry.ko_keywords.replace('|', " "));
    }

    IndexItem {
        name: display_name,
        path: entry.emoji.to_string(), // The actual emoji to copy
        kind: ItemKind::Emoji,
        source: Source::Plugin,
        icon: entry.emoji.to_string(),
        keywords: kw,
    }
}

impl Extension for EmojiExtension {
    fn name(&self) -> &str {
        "Emoji"
    }
    fn prefix(&self) -> Option<&str> {
        Some(":emoji")
    }

    fn search(&self, query: &str) -> Vec<IndexItem> {
        self.search_emoji(query)
    }

    fn execute(&self, item: &IndexItem) -> ExtensionAction {
        if !item.path.is_empty() {
            ExtensionAction::CopyToClipboard(item.path.clone())
        } else {
            ExtensionAction::Noop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoji_db_loads() {
        let db = emoji_db();
        assert!(
            db.len() > 100,
            "Expected at least 100 emoji, got {}",
            db.len()
        );
    }

    #[test]
    fn test_korean_data_loaded() {
        let db = emoji_db();
        // At least some entries should have Korean data
        let with_ko = db.iter().filter(|e| !e.ko_name.is_empty()).count();
        assert!(
            with_ko > 500,
            "Expected at least 500 emoji with Korean data, got {}",
            with_ko
        );
    }

    #[test]
    fn test_search_fire() {
        let ext = EmojiExtension;
        let results = ext.search_emoji("fire");
        assert!(!results.is_empty());
        assert!(
            results[0].path.contains('🔥') || results[0].name.to_lowercase().contains("fire"),
            "Expected fire emoji or 'fire' in name, got {:?}",
            results.first()
        );
    }

    #[test]
    fn test_search_empty_returns_popular() {
        let ext = EmojiExtension;
        let results = ext.search_emoji("");
        assert_eq!(results.len(), 50);
    }

    #[test]
    fn test_search_heart() {
        let ext = EmojiExtension;
        let results = ext.search_emoji("heart");
        assert!(results.len() > 5);
    }

    #[test]
    fn test_search_korean_heart() {
        let ext = EmojiExtension;
        let results = ext.search_emoji("하트");
        assert!(
            !results.is_empty(),
            "Expected results for Korean '하트' search"
        );
        // Should contain heart-related emoji
        let has_heart = results.iter().any(|r| {
            r.name.contains("heart") || r.name.contains("하트")
        });
        assert!(has_heart, "Expected heart emoji in results for '하트'");
    }

    #[test]
    fn test_search_korean_fire() {
        let ext = EmojiExtension;
        let results = ext.search_emoji("불");
        assert!(
            !results.is_empty(),
            "Expected results for Korean '불' search"
        );
        let has_fire = results.iter().any(|r| r.path == "🔥");
        assert!(has_fire, "Expected 🔥 in results for '불'");
    }

    #[test]
    fn test_search_korean_smile() {
        let ext = EmojiExtension;
        let results = ext.search_emoji("웃음");
        assert!(
            !results.is_empty(),
            "Expected results for Korean '웃음' search"
        );
    }

    #[test]
    fn test_search_mixed_ko_en() {
        let ext = EmojiExtension;
        // Mixed Korean + English query should work
        let results = ext.search_emoji("사랑 heart");
        assert!(
            !results.is_empty(),
            "Expected results for mixed Korean/English query"
        );
    }

    #[test]
    fn test_has_hangul() {
        assert!(has_hangul("하트"));
        assert!(has_hangul("abc하트def"));
        assert!(!has_hangul("heart"));
        assert!(!has_hangul("fire"));
        assert!(!has_hangul("123"));
    }
}
