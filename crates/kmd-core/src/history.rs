//! History tracking — frequency-based scoring for search results

use crate::db::Database;
use crate::search::SearchResult;

/// Maximum number of history entries to consider when boosting.
const HISTORY_BOOST_LIMIT: usize = 500;
/// Score boost per usage frequency unit.
const FREQUENCY_BOOST_SCORE: u32 = 100;

/// Boost search results based on usage history
pub fn boost_results(
    results: &mut [SearchResult],
    db: &Database,
) {
    let history = db.query_history(HISTORY_BOOST_LIMIT);

    // Build a frequency map: value -> frequency
    let freq_map: std::collections::HashMap<String, u32> = history
        .into_iter()
        .map(|h| (h.value, h.frequency))
        .collect();

    // Apply frequency boost to scores
    for result in results.iter_mut() {
        if let Some(&freq) = freq_map.get(&result.item.path) {
            result.score = result.score.saturating_add(freq * FREQUENCY_BOOST_SCORE);
        }
    }

    // Re-sort by score (descending)
    results.sort_by(|a, b| b.score.cmp(&a.score));
}

/// Record a launch event
pub fn record_launch(db: &Database, item_type: &str, value: &str, display: Option<&str>) {
    if let Err(e) = db.record_launch(item_type, value, display) {
        tracing::warn!("Failed to record launch history: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{IndexItem, ItemKind, Source};
    use crate::search::SearchResult;

    fn make_result(path: &str, score: u32) -> SearchResult {
        SearchResult {
            item: IndexItem {
                name: path.to_string(),
                path: path.to_string(),
                kind: ItemKind::File,
                source: Source::FileProvider,
                icon: "--".to_string(),
                keywords: String::new(),
            },
            score,
        }
    }

    #[test]
    fn test_boost_results_reorders_by_frequency() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        // Record launches for "b" 3 times
        for _ in 0..3 {
            record_launch(&db, "file", "b", None);
        }

        let mut results = vec![
            make_result("a", 500),
            make_result("b", 100),
        ];

        boost_results(&mut results, &db);

        // "b" should now be first because 100 + 3*100 = 400, but "a" has 500.
        // Actually 3 launches means frequency=3, so score = 100 + 300 = 400 < 500
        // Let's record more to overtake
        for _ in 0..5 {
            record_launch(&db, "file", "b", None);
        }
        // Now frequency=8, score = 100 + 800 = 900 > 500
        let mut results = vec![
            make_result("a", 500),
            make_result("b", 100),
        ];
        boost_results(&mut results, &db);

        assert_eq!(results[0].item.path, "b");
        assert_eq!(results[1].item.path, "a");
    }

    #[test]
    fn test_boost_no_history_preserves_order() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        let mut results = vec![
            make_result("a", 500),
            make_result("b", 300),
        ];

        boost_results(&mut results, &db);

        assert_eq!(results[0].item.path, "a");
        assert_eq!(results[1].item.path, "b");
    }
}
