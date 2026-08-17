//! 문서 본문 검색 인덱스 (SQLite FTS5) — docs/15 P1.
//!
//! 설계 원칙 (Docufinder 분석에서 채택, 코드는 전부 자체 구현):
//! - `content_files`(메타) + `content_fts`(FTS5) 두 테이블, rowid 수동 동기화
//! - 증분 판정은 mtime+size **둘 다** 일치할 때만 스킵 — FAT 2초 해상도,
//!   네트워크 드라이브 등 플랫폼별 mtime 편차에 보수적으로 대응
//! - 파일당 크기 상한 + 확장자 허용 목록 + NUL 스니핑으로 바이너리 배제
//! - UTF-8 실패 시 EUC-KR/CP949 폴백 (Windows 한국어 legacy 텍스트)
//! - 읽기/디코드 실패는 조용히 스킵 — 인덱싱은 항상 완주한다
//!
//! 스캔 범위는 `launcher.search_paths` + `ignore_patterns` + `search_depth`를
//! 파일명 인덱스와 그대로 공유한다 (범위 개념이 둘로 갈라지지 않게).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::config::LauncherConfig;
use crate::db::{Database, DbError};

/// 내장 기본 확장자 목록 (플레인 텍스트 + 주요 소스코드).
/// config `content_search.extensions`가 비어 있지 않으면 통째로 대체된다.
const DEFAULT_EXTENSIONS: &[&str] = &[
    // plain text / docs
    "txt",
    "md",
    "markdown",
    "rst",
    "org",
    "log",
    "csv",
    // config / data
    "json",
    "yaml",
    "yml",
    "toml",
    "ini",
    "conf",
    "env",
    "properties",
    // scripts / source
    "sh",
    "zsh",
    "bash",
    "ps1",
    "bat",
    "cmd",
    "py",
    "rs",
    "js",
    "ts",
    "jsx",
    "tsx",
    "go",
    "java",
    "kt",
    "c",
    "h",
    "cpp",
    "hpp",
    "cc",
    "cs",
    "rb",
    "php",
    "swift",
    "sql",
    "lua",
    // web
    "html",
    "htm",
    "css",
    "xml",
    "svg",
];

/// 검색 최소 질의 길이(문자). 1자는 FTS 재현율도 낮고 결과가 폭주한다 —
/// 런처는 모든 쿼리가 1자를 통과하므로 명시적 하한이 필요하다 (docs/15).
pub const MIN_QUERY_CHARS: usize = 2;

/// 유효 확장자 집합 — 설정이 비어 있으면 내장 기본 목록.
/// (folder_suggest의 활동 스캔도 같은 대상 정의를 공유한다)
pub(crate) fn allowed_extensions(cs: &crate::config::ContentSearchConfig) -> HashSet<String> {
    if cs.extensions.is_empty() {
        DEFAULT_EXTENSIONS.iter().map(|s| s.to_string()).collect()
    } else {
        cs.extensions.iter().map(|s| s.to_lowercase()).collect()
    }
}

/// 배치 트랜잭션 크기 — fsync 횟수를 줄인다.
const TXN_BATCH: usize = 200;

/// 본문 검색 결과 1건.
#[derive(Debug, Clone)]
pub struct ContentHit {
    /// 파일 절대 경로
    pub path: String,
    /// FTS5 snippet — 매치 구간은 `«»`로 감싸져 있다
    pub snippet: String,
    /// bm25 점수 (음수, 작을수록 좋음 — 정렬 완료 상태로 반환)
    pub score: f64,
}

/// sync 실행 결과 요약 (로그/CLI 출력용).
#[derive(Debug, Default, Clone, Copy)]
pub struct SyncStats {
    /// 스캔에서 발견한 대상 파일 수
    pub scanned: usize,
    /// 새로 인덱싱했거나 갱신한 파일 수
    pub indexed: usize,
    /// mtime+size 일치로 스킵한 파일 수
    pub skipped: usize,
    /// 디스크에서 사라져 인덱스에서 제거한 파일 수
    pub removed: usize,
    /// 읽기/디코드 실패로 건너뛴 파일 수
    pub failed: usize,
}

/// search_paths를 스캔해 본문 인덱스를 증분 갱신한다.
///
/// 항상 완주를 지향한다 — 개별 파일의 실패는 통계에만 반영하고 계속 진행.
pub fn sync(db: &Database, launcher: &LauncherConfig) -> Result<SyncStats, DbError> {
    let cs = &launcher.content_search;
    let mut stats = SyncStats::default();
    if !cs.enabled {
        return Ok(stats);
    }

    let allowed = allowed_extensions(cs);
    let max_bytes = cs.max_file_kb.saturating_mul(1024);
    let ignore_set: HashSet<&str> = launcher
        .ignore_patterns
        .iter()
        .map(|s| s.as_str())
        .collect();

    // 1. 기존 인덱스 상태 로드 (path → (id, size, mtime))
    let mut existing: HashMap<String, (i64, u64, i64)> = HashMap::new();
    {
        let mut stmt = db
            .conn()
            .prepare("SELECT id, path, size, mtime FROM content_files")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows.flatten() {
            existing.insert(row.1, (row.0, row.2 as u64, row.3));
        }
    }

    // 2. 스캔 + 증분 upsert (배치 트랜잭션)
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending: Vec<(String, u64, i64, String)> = Vec::new(); // (path, size, mtime, body)
    let mut truncated = false;

    'roots: for root in &launcher.search_paths {
        if !root.is_dir() {
            continue;
        }
        // depth 0(루트 자신)은 무시 규칙에서 제외 — 사용자가 숨김 폴더를
        // 의도적으로 search_paths에 넣은 경우까지 걸러버리지 않게.
        let walker = walkdir::WalkDir::new(root)
            .max_depth(launcher.search_depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                e.depth() == 0 || !crate::index::files::is_ignored_dir(e, &ignore_set)
            });
        for entry in walker.flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_str().unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            let ext = Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            if !allowed.contains(&ext) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let size = meta.len();
            if size == 0 || size > max_bytes {
                continue;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let path_str = entry.path().to_string_lossy().to_string();

            if seen.len() >= cs.max_files {
                truncated = true;
                break 'roots;
            }
            if !seen.insert(path_str.clone()) {
                continue; // search_paths 중첩으로 인한 중복
            }
            stats.scanned += 1;

            // 증분 스킵: mtime과 size가 둘 다 그대로면 본문도 그대로라고 본다
            if let Some(&(_, old_size, old_mtime)) = existing.get(&path_str) {
                if old_size == size && old_mtime == mtime {
                    stats.skipped += 1;
                    continue;
                }
            }

            match read_text(entry.path(), size as usize) {
                Some(body) => pending.push((path_str, size, mtime, body)),
                None => {
                    stats.failed += 1;
                    // 과거에 인덱싱됐던 파일이 바이너리로 바뀐 경우 제거 대상으로.
                    // seen에서 빼두면 4단계 삭제 로직이 정리한다.
                    seen.remove(&entry.path().to_string_lossy().to_string());
                }
            }

            if pending.len() >= TXN_BATCH {
                stats.indexed += flush_batch(db, &mut pending)?;
            }
        }
    }
    stats.indexed += flush_batch(db, &mut pending)?;
    if truncated {
        tracing::warn!(
            "본문 인덱스 상한({}) 도달 — 초과 파일은 건너뜀 (content_search.max_files)",
            cs.max_files
        );
    }

    // 3. 디스크에서 사라진(또는 대상에서 빠진) 파일 제거
    let tx = db.conn().unchecked_transaction()?;
    for (path, (id, _, _)) in &existing {
        if !seen.contains(path) {
            tx.execute("DELETE FROM content_fts WHERE rowid = ?1", [id])?;
            tx.execute("DELETE FROM content_files WHERE id = ?1", [id])?;
            stats.removed += 1;
        }
    }
    tx.commit()?;

    Ok(stats)
}

/// pending 배치를 한 트랜잭션으로 커밋. 성공적으로 반영한 건수를 반환.
fn flush_batch(
    db: &Database,
    pending: &mut Vec<(String, u64, i64, String)>,
) -> Result<usize, DbError> {
    if pending.is_empty() {
        return Ok(0);
    }
    let tx = db.conn().unchecked_transaction()?;
    let mut n = 0usize;
    for (path, size, mtime, body) in pending.drain(..) {
        tx.execute(
            "INSERT INTO content_files(path, size, mtime) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET
               size = ?2, mtime = ?3,
               indexed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
            rusqlite::params![path, size as i64, mtime],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM content_files WHERE path = ?1",
            [&path],
            |row| row.get(0),
        )?;
        tx.execute("DELETE FROM content_fts WHERE rowid = ?1", [id])?;
        tx.execute(
            "INSERT INTO content_fts(rowid, body) VALUES (?1, ?2)",
            rusqlite::params![id, body],
        )?;
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}

/// 파일을 텍스트로 읽는다. UTF-8 → EUC-KR 폴백, 바이너리는 None.
fn read_text(path: &Path, size_hint: usize) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    debug_assert!(bytes.len() <= size_hint.saturating_add(4096));
    // NUL 스니핑: 텍스트 파일에 NUL이 들어갈 일은 사실상 없다
    let sniff = &bytes[..bytes.len().min(8192)];
    if sniff.contains(&0) {
        return None;
    }
    match String::from_utf8(bytes) {
        Ok(s) => Some(s),
        Err(e) => {
            let bytes = e.into_bytes();
            let (decoded, _, had_errors) = encoding_rs::EUC_KR.decode(&bytes);
            // 치환문자 비율로 "진짜 EUC-KR"인지 판정 — 잡음 바이너리 배제
            if had_errors {
                let bad = decoded.chars().filter(|&c| c == '\u{FFFD}').count();
                if bad * 10 > decoded.chars().count().max(1) {
                    return None;
                }
            }
            Some(decoded.into_owned())
        }
    }
}

/// 본문 검색 — bm25 랭킹 + snippet. 질의가 짧으면(2자 미만) 빈 결과.
pub fn search(db: &Database, query: &str, limit: usize) -> Result<Vec<ContentHit>, DbError> {
    let Some(match_expr) = build_match_expr(query) else {
        return Ok(Vec::new());
    };
    let mut stmt = db.conn().prepare(
        "SELECT f.path,
                snippet(content_fts, 0, '«', '»', '…', 16),
                bm25(content_fts)
         FROM content_fts
         JOIN content_files f ON f.id = content_fts.rowid
         WHERE content_fts MATCH ?1
         ORDER BY bm25(content_fts)
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![match_expr, limit as i64], |row| {
        Ok(ContentHit {
            path: row.get(0)?,
            snippet: row.get::<_, String>(1)?.replace(['\n', '\r'], " "),
            score: row.get(2)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// 사용자 질의 → FTS5 MATCH 식.
///
/// - 각 토큰을 큰따옴표로 감싸 FTS5 연산자(AND/OR/NEAR/괄호 등) 해석을 차단
/// - 마지막 토큰에는 prefix `*` — 런처의 "타이핑 중" 부분어에 대응
/// - 전체 유효 문자가 [`MIN_QUERY_CHARS`] 미만이면 None
fn build_match_expr(query: &str) -> Option<String> {
    let tokens: Vec<&str> = query.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let total_chars: usize = tokens.iter().map(|t| t.chars().count()).sum();
    if total_chars < MIN_QUERY_CHARS {
        return None;
    }
    let last = tokens.len() - 1;
    let expr = tokens
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let quoted = format!("\"{}\"", t.replace('"', "\"\""));
            if i == last {
                format!("{quoted}*")
            } else {
                quoted
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(expr)
}

/// 인덱스 통계 (CLI `kmd index --stats`용).
pub fn stats(db: &Database) -> Result<(usize, u64), DbError> {
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM content_files", [], |r| r.get(0))?;
    let bytes: i64 = db.conn().query_row(
        "SELECT COALESCE(SUM(size), 0) FROM content_files",
        [],
        |r| r.get(0),
    )?;
    Ok((count as usize, bytes as u64))
}

// ── 런처(`?` prefix) 통합 — TUI/데스크톱 공용 (folder_search.rs 패턴) ────────

/// `?질의`/`:grep 질의`에서 프리픽스를 벗긴 실제 질의.
/// UI가 "빈 질의(폴더 제안 노출 시점)" 판단에도 같은 규칙을 쓴다.
pub fn strip_query(raw: &str) -> &str {
    raw.strip_prefix('?')
        .or_else(|| raw.strip_prefix(":grep"))
        .unwrap_or(raw)
        .trim()
}

/// `?질의` / `:grep 질의` 입력을 파싱해 런처 결과 목록을 만든다.
///
/// - `db`가 None이면 DB 열기 실패 안내 항목
/// - 질의가 비었거나 2자 미만이면 사용법 안내 항목
/// - 결과의 name은 "파일명 — 스니펫" (매치 구간 «») , path는 실제 경로라
///   Enter로 그대로 파일이 열린다
pub fn launcher_results(
    db: Option<&Database>,
    raw: &str,
    use_emoji: bool,
    limit: usize,
) -> Vec<crate::search::SearchResult> {
    use crate::index::{IndexItem, ItemKind, Source};
    use crate::search::SearchResult;

    let query = strip_query(raw);

    let info = |name: String, path: String, icon: &str| SearchResult {
        item: IndexItem {
            name,
            path,
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: icon.to_string(),
            keywords: "kmd:content:hint".to_string(),
            icon_path: None,
        },
        score: 0,
    };

    let Some(db) = db else {
        return vec![info(
            "본문 인덱스 DB를 열 수 없습니다".to_string(),
            "kmd index --rebuild 실행 또는 데몬 상태를 확인하세요".to_string(),
            "\u{26A0}\u{FE0F}",
        )];
    };

    if query.chars().filter(|c| !c.is_whitespace()).count() < MIN_QUERY_CHARS {
        return vec![info(
            "?검색어  — 문서 본문 검색 (2자 이상)".to_string(),
            "파일 이름이 아니라 내용으로 찾습니다  ·  예: ?예산 삭감".to_string(),
            "\u{1F50E}",
        )];
    }

    let hits = match search(db, query, limit) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("본문 검색 실패: {e}");
            return vec![info(
                "본문 검색 중 오류가 발생했습니다".to_string(),
                e.to_string(),
                "\u{26A0}\u{FE0F}",
            )];
        }
    };

    if hits.is_empty() {
        let (count, _) = stats(db).unwrap_or((0, 0));
        if count == 0 {
            return vec![info(
                "본문 인덱스가 아직 비어 있습니다".to_string(),
                "kmd index --rebuild 로 생성하거나 데몬 리프레시를 기다리세요".to_string(),
                "\u{1F4ED}",
            )];
        }
        return vec![info(
            format!("'{query}' 본문 매치 없음"),
            format!("인덱싱된 파일 {count}개에서 찾지 못했습니다"),
            "\u{1F50D}",
        )];
    }

    let total = hits.len();
    hits.into_iter()
        .enumerate()
        .map(|(i, hit)| {
            let fname = std::path::Path::new(&hit.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&hit.path)
                .to_string();
            let mut snippet = hit.snippet;
            if snippet.chars().count() > 90 {
                snippet = snippet.chars().take(90).collect::<String>() + "…";
            }
            SearchResult {
                item: IndexItem {
                    name: format!("{fname}  —  {snippet}"),
                    path: hit.path,
                    kind: ItemKind::File,
                    source: Source::FileProvider,
                    icon: if use_emoji { "\u{1F4C4}" } else { "F " }.to_string(),
                    keywords: "kmd:content:hit".to_string(),
                    icon_path: None,
                },
                // bm25 정렬 순서를 그대로 보존하는 순위 역산 점수
                score: ((total - i) * 10) as u32,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_launcher(root: &Path) -> LauncherConfig {
        LauncherConfig {
            search_paths: vec![root.to_path_buf()],
            ..Default::default()
        }
    }

    fn write_file(dir: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }

    #[test]
    fn 인덱싱_후_한국어_영어_검색() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "예산.md",
            "2026년 예산 삭감 계획과 집행 일정".as_bytes(),
        );
        write_file(
            dir.path(),
            "notes.txt",
            b"quarterly budget review for the launcher",
        );
        let db = Database::open_in_memory().unwrap();

        let stats = sync(&db, &test_launcher(dir.path())).unwrap();
        assert_eq!(stats.indexed, 2);

        let hits = search(&db, "예산", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("예산.md"));
        assert!(
            hits[0].snippet.contains("«예산»"),
            "스니펫 하이라이트: {}",
            hits[0].snippet
        );

        // prefix: "budg" → budget
        let hits = search(&db, "budg", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("notes.txt"));
    }

    #[test]
    fn mtime_size_동일하면_스킵_변경되면_재인덱싱() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_file(dir.path(), "a.md", b"alpha original");
        let db = Database::open_in_memory().unwrap();
        let launcher = test_launcher(dir.path());

        let s1 = sync(&db, &launcher).unwrap();
        assert_eq!((s1.indexed, s1.skipped), (1, 0));

        let s2 = sync(&db, &launcher).unwrap();
        assert_eq!((s2.indexed, s2.skipped), (0, 1), "무변경 재실행은 스킵");

        // 내용 변경 (size가 달라지므로 mtime 해상도와 무관하게 감지)
        std::fs::write(&p, b"alpha rewritten completely").unwrap();
        let s3 = sync(&db, &launcher).unwrap();
        assert_eq!(s3.indexed, 1, "변경 파일은 재인덱싱");
        let hits = search(&db, "rewritten", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(
            search(&db, "original", 10).unwrap().is_empty(),
            "구 본문은 교체됨"
        );
    }

    #[test]
    fn 삭제된_파일은_인덱스에서_제거() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_file(dir.path(), "gone.txt", b"ephemeral content here");
        let db = Database::open_in_memory().unwrap();
        let launcher = test_launcher(dir.path());

        sync(&db, &launcher).unwrap();
        assert_eq!(search(&db, "ephemeral", 10).unwrap().len(), 1);

        std::fs::remove_file(&p).unwrap();
        let s = sync(&db, &launcher).unwrap();
        assert_eq!(s.removed, 1);
        assert!(search(&db, "ephemeral", 10).unwrap().is_empty());
    }

    #[test]
    fn euc_kr_폴백과_바이너리_배제() {
        let dir = tempfile::tempdir().unwrap();
        // "한글 보고서" EUC-KR 인코딩
        let (encoded, _, _) = encoding_rs::EUC_KR.encode("한글 보고서 내용입니다");
        write_file(dir.path(), "legacy.txt", &encoded);
        // NUL 포함 바이너리 (허용 확장자여도 배제)
        write_file(dir.path(), "fake.txt", b"PK\x03\x04\x00\x00binary");
        let db = Database::open_in_memory().unwrap();

        let s = sync(&db, &test_launcher(dir.path())).unwrap();
        assert_eq!(s.indexed, 1, "EUC-KR만 인덱싱, 바이너리는 실패 처리");
        assert_eq!(s.failed, 1);
        assert_eq!(search(&db, "보고서", 10).unwrap().len(), 1);
    }

    #[test]
    fn 확장자_필터와_크기_상한() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "keep.md", b"indexed body");
        write_file(dir.path(), "skip.bin", b"not a text extension");
        write_file(
            dir.path(),
            "big.txt",
            "x".repeat(2 * 1024 * 1024).as_bytes(),
        );
        let db = Database::open_in_memory().unwrap();

        let s = sync(&db, &test_launcher(dir.path())).unwrap();
        assert_eq!(s.indexed, 1, "확장자 밖·상한 초과는 대상 아님");
    }

    #[test]
    fn 짧은_질의와_fts5_연산자_무력화() {
        let db = Database::open_in_memory().unwrap();
        assert!(
            search(&db, "a", 10).unwrap().is_empty(),
            "1자 질의는 빈 결과"
        );
        assert!(search(&db, " ", 10).unwrap().is_empty());
        // FTS5 문법 문자가 그대로 오면 오류 없이 리터럴 처리되어야 한다
        for q in ["AND OR", "NEAR(", "\"열린따옴표", "col:val", "a*b(c)"] {
            search(&db, q, 10).unwrap_or_else(|e| panic!("질의 {q:?} 에서 오류: {e}"));
        }
    }

    #[test]
    fn 런처_결과_변환과_안내_항목() {
        use crate::index::ItemKind;
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "메모.md", "예산 회의 정리".as_bytes());
        let db = Database::open_in_memory().unwrap();
        sync(&db, &test_launcher(dir.path())).unwrap();

        let rs = launcher_results(Some(&db), "?예산", true, 10);
        assert_eq!(rs.len(), 1);
        assert!(
            rs[0].item.name.starts_with("메모.md"),
            "{}",
            rs[0].item.name
        );
        assert!(rs[0].item.name.contains("«예산»"));
        assert!(rs[0].item.path.ends_with("메모.md"), "Enter 실행용 실경로");
        assert_eq!(rs[0].item.kind, ItemKind::File);

        // :grep 별칭도 같은 결과
        assert_eq!(launcher_results(Some(&db), ":grep 예산", true, 10).len(), 1);

        // 안내 항목들: 짧은 질의 / DB 없음 / 매치 없음
        let hint = launcher_results(Some(&db), "?", true, 10);
        assert_eq!(hint[0].item.kind, ItemKind::SystemCommand);
        assert!(launcher_results(None, "?예산", true, 10)[0]
            .item
            .name
            .contains("열 수 없습니다"));
        assert!(launcher_results(Some(&db), "?존재안하는단어", true, 10)[0]
            .item
            .name
            .contains("매치 없음"));
    }

    #[test]
    fn match_식_합성() {
        assert_eq!(build_match_expr("예산"), Some("\"예산\"*".into()));
        assert_eq!(
            build_match_expr("예산 삭감"),
            Some("\"예산\" \"삭감\"*".into())
        );
        assert_eq!(build_match_expr("a"), None, "2자 미만");
        assert_eq!(build_match_expr("ab"), Some("\"ab\"*".into()));
        assert_eq!(build_match_expr("  "), None);
    }
}
