//! `kmd grep` — 문서 본문 검색 (FTS5, docs/15)

use color_eyre::Result;
use kmd_core::content_index;

pub fn run(query: &str, limit: usize, sync_first: bool) -> Result<()> {
    let config = super::load_config()?;
    let db = super::open_db()?;

    if !config.launcher.content_search.enabled {
        println!("본문 검색이 비활성화되어 있습니다 (launcher.content_search.enabled = false).");
        return Ok(());
    }

    let (count, _) = content_index::stats(&db)?;
    if sync_first || count == 0 {
        if count == 0 {
            println!("본문 인덱스가 비어 있어 먼저 생성합니다...");
        }
        let start = std::time::Instant::now();
        let s = content_index::sync(&db, &config.launcher)?;
        println!(
            "본문 인덱스 동기화: 스캔 {} · 갱신 {} · 스킵 {} · 제거 {} · 실패 {} ({:.1}ms)",
            s.scanned,
            s.indexed,
            s.skipped,
            s.removed,
            s.failed,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }

    if query.chars().filter(|c| !c.is_whitespace()).count() < content_index::MIN_QUERY_CHARS {
        println!("질의가 너무 짧습니다 (최소 {}자).", content_index::MIN_QUERY_CHARS);
        return Ok(());
    }

    let start = std::time::Instant::now();
    let hits = content_index::search(&db, query, limit)?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    if hits.is_empty() {
        println!("결과 없음 ({elapsed_ms:.1}ms)");
        return Ok(());
    }
    for hit in &hits {
        println!("{}", hit.path);
        println!("  {}", hit.snippet);
    }
    println!("\n{}건 ({elapsed_ms:.1}ms)", hits.len());
    Ok(())
}
