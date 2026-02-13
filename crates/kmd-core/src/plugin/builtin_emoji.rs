//! Built-in emoji search extension
//!
//! Activated with `:emoji` or `:e` prefix.
//! Searches Unicode emoji by name and keywords, copies to clipboard on selection.

use std::sync::OnceLock;

use crate::index::{IndexItem, ItemKind, Source};

use super::{Extension, ExtensionAction};

/// Single emoji entry parsed from the embedded data
struct EmojiEntry {
    emoji: &'static str,
    name: &'static str,
    category: &'static str,
}

/// Embedded emoji dataset (parsed lazily from TSV)
static EMOJI_DB: OnceLock<Vec<EmojiEntry>> = OnceLock::new();

fn emoji_db() -> &'static [EmojiEntry] {
    EMOJI_DB.get_or_init(|| {
        let raw = include_str!("../../data/emoji.tsv");
        raw.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                let emoji = parts.next()?.trim();
                let name = parts.next()?.trim();
                let category = parts.next().unwrap_or("").trim();
                Some(EmojiEntry {
                    emoji,
                    name,
                    category,
                })
            })
            .collect()
    })
}

pub struct EmojiExtension;

impl EmojiExtension {
    /// Search emoji with a query. Returns matching emoji as IndexItems.
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
                let mut score = 0usize;

                for token in &tokens {
                    if name_lower.contains(token) {
                        score += 10;
                        // Bonus for exact word match
                        if name_lower.split_whitespace().any(|w| w == *token) {
                            score += 5;
                        }
                        // Bonus for starts-with
                        if name_lower.starts_with(token) {
                            score += 3;
                        }
                    } else if cat_lower.contains(token) {
                        score += 2;
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
    IndexItem {
        name: format!("{} {}", entry.emoji, entry.name),
        path: entry.emoji.to_string(), // The actual emoji to copy
        kind: ItemKind::Emoji,
        source: Source::Plugin,
        icon: entry.emoji.to_string(),
        keywords: format!("{} {}", entry.name, entry.category),
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
    fn test_search_fire() {
        let ext = EmojiExtension;
        let results = ext.search_emoji("fire");
        assert!(!results.is_empty());
        // First result should contain fire emoji
        assert!(
            results[0].path.contains('🔥') || results[0].name.to_lowercase().contains("fire"),
            "Expected fire emoji or 'fire' in name, got {:?}",
            results.get(0)
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
}
