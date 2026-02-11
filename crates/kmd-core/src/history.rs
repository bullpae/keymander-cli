//! History tracking — frequency-based scoring for search results

use crate::db::Database;
use crate::search::SearchResult;

/// Boost search results based on usage history
pub fn boost_results(
    results: &mut [SearchResult],
    db: &Database,
) {
    let history = db.query_history(500);

    // Build a frequency map: value -> frequency
    let freq_map: std::collections::HashMap<String, u32> = history
        .into_iter()
        .map(|h| (h.value, h.frequency))
        .collect();

    // Apply frequency boost to scores
    for result in results.iter_mut() {
        if let Some(&freq) = freq_map.get(&result.item.path) {
            // Boost score by frequency (each usage adds 100 to score)
            result.score = result.score.saturating_add(freq * 100);
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
