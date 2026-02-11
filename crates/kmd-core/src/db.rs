//! SQLite database for history, bookmarks, and key-value storage.

use std::path::Path;

use rusqlite::Connection;

/// Database wrapper (history + bookmarks + kv_store)
pub struct Database {
    pub conn: Connection,
}

impl Database {
    /// Open database file (creates parent dirs if needed)
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(DbError::Io)?;
        }

        let conn = Connection::open(path).map_err(DbError::Sqlite)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(DbError::Sqlite)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(DbError::Sqlite)?;

        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Open in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory().map_err(DbError::Sqlite)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Record a launch in history (upsert: increment frequency)
    pub fn record_launch(
        &self,
        item_type: &str,
        value: &str,
        display: Option<&str>,
    ) -> Result<(), DbError> {
        self.conn
            .execute(
                "INSERT INTO history(item_type, value, display, frequency)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(item_type, value) DO UPDATE SET
                   frequency = frequency + 1,
                   display = COALESCE(?3, display),
                   executed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                rusqlite::params![item_type, value, display],
            )
            .map_err(DbError::Sqlite)?;
        Ok(())
    }

    /// Query recent/frequent history entries
    pub fn query_history(&self, limit: usize) -> Vec<HistoryEntry> {
        let mut stmt = match self.conn.prepare(
            "SELECT COALESCE(display, value), value, item_type, frequency, executed_at
             FROM history
             ORDER BY frequency DESC, executed_at DESC
             LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok(HistoryEntry {
                    display: row.get(0)?,
                    value: row.get(1)?,
                    item_type: row.get(2)?,
                    frequency: row.get(3)?,
                    executed_at: row.get(4)?,
                })
            })
            .ok();

        match rows {
            Some(iter) => iter.filter_map(|r| r.ok()).collect(),
            None => Vec::new(),
        }
    }

    /// Search history entries matching a query
    pub fn search_history(&self, query: &str, limit: usize) -> Vec<HistoryEntry> {
        let pattern = format!("%{}%", query);
        let mut stmt = match self.conn.prepare(
            "SELECT COALESCE(display, value), value, item_type, frequency, executed_at
             FROM history
             WHERE value LIKE ?1 OR display LIKE ?1
             ORDER BY frequency DESC, executed_at DESC
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = stmt
            .query_map(rusqlite::params![pattern, limit as i64], |row| {
                Ok(HistoryEntry {
                    display: row.get(0)?,
                    value: row.get(1)?,
                    item_type: row.get(2)?,
                    frequency: row.get(3)?,
                    executed_at: row.get(4)?,
                })
            })
            .ok();

        match rows {
            Some(iter) => iter.filter_map(|r| r.ok()).collect(),
            None => Vec::new(),
        }
    }

    /// Clear all history
    pub fn clear_history(&self) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM history", [])
            .map_err(DbError::Sqlite)?;
        Ok(())
    }

    /// Add a bookmark
    pub fn add_bookmark(
        &self,
        name: &str,
        value: &str,
        item_type: &str,
    ) -> Result<(), DbError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO bookmarks(name, value, item_type) VALUES (?1, ?2, ?3)",
                rusqlite::params![name, value, item_type],
            )
            .map_err(DbError::Sqlite)?;
        Ok(())
    }

    /// Query all bookmarks
    pub fn query_bookmarks(&self) -> Vec<BookmarkEntry> {
        let mut stmt = match self.conn.prepare(
            "SELECT name, value, item_type, created_at FROM bookmarks ORDER BY created_at DESC",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = stmt
            .query_map([], |row| {
                Ok(BookmarkEntry {
                    name: row.get(0)?,
                    value: row.get(1)?,
                    item_type: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .ok();

        match rows {
            Some(iter) => iter.filter_map(|r| r.ok()).collect(),
            None => Vec::new(),
        }
    }

    /// Get a value from kv_store
    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM kv_store WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .ok()
    }

    /// Set a value in kv_store
    pub fn kv_set(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.conn
            .execute(
                "INSERT INTO kv_store(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET
                   value = ?2,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                rusqlite::params![key, value],
            )
            .map_err(DbError::Sqlite)?;
        Ok(())
    }

    /// Run database migrations
    fn migrate(&self) -> Result<(), DbError> {
        let version: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(DbError::Sqlite)?;

        if version < 1 {
            self.conn
                .execute_batch(
                    "
                    -- Launch history
                    CREATE TABLE IF NOT EXISTS history (
                        id          INTEGER PRIMARY KEY AUTOINCREMENT,
                        item_type   TEXT    NOT NULL,
                        value       TEXT    NOT NULL,
                        display     TEXT,
                        executed_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                        frequency   INTEGER NOT NULL DEFAULT 1
                    );
                    CREATE UNIQUE INDEX IF NOT EXISTS idx_history_unique ON history(item_type, value);

                    -- Bookmarks
                    CREATE TABLE IF NOT EXISTS bookmarks (
                        id          INTEGER PRIMARY KEY AUTOINCREMENT,
                        name        TEXT    NOT NULL,
                        value       TEXT    NOT NULL,
                        item_type   TEXT    NOT NULL,
                        created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
                    );

                    -- Key-value store
                    CREATE TABLE IF NOT EXISTS kv_store (
                        key        TEXT PRIMARY KEY,
                        value      TEXT NOT NULL,
                        updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
                    );
                    ",
                )
                .map_err(DbError::Sqlite)?;
            self.conn
                .pragma_update(None, "user_version", 1)
                .map_err(DbError::Sqlite)?;
        }

        Ok(())
    }
}

/// A history entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub display: String,
    pub value: String,
    pub item_type: String,
    pub frequency: u32,
    pub executed_at: String,
}

/// A bookmark entry
#[derive(Debug, Clone)]
pub struct BookmarkEntry {
    pub name: String,
    pub value: String,
    pub item_type: String,
    pub created_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_query_history() {
        let db = Database::open_in_memory().unwrap();
        db.record_launch("app", "firefox", Some("Firefox")).unwrap();
        db.record_launch("app", "firefox", Some("Firefox")).unwrap();
        db.record_launch("app", "code", Some("VS Code")).unwrap();

        let history = db.query_history(10);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].display, "Firefox");
        assert_eq!(history[0].frequency, 2);
    }

    #[test]
    fn test_search_history() {
        let db = Database::open_in_memory().unwrap();
        db.record_launch("app", "firefox", Some("Firefox")).unwrap();
        db.record_launch("app", "code", Some("VS Code")).unwrap();

        let results = db.search_history("fire", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, "firefox");
    }

    #[test]
    fn test_clear_history() {
        let db = Database::open_in_memory().unwrap();
        db.record_launch("app", "firefox", None).unwrap();
        db.clear_history().unwrap();
        assert!(db.query_history(10).is_empty());
    }

    #[test]
    fn test_kv_store() {
        let db = Database::open_in_memory().unwrap();
        db.kv_set("last_index", "2026-01-01").unwrap();
        assert_eq!(db.kv_get("last_index"), Some("2026-01-01".to_string()));
    }

    #[test]
    fn test_bookmarks() {
        let db = Database::open_in_memory().unwrap();
        db.add_bookmark("Firefox", "firefox", "app").unwrap();
        let bookmarks = db.query_bookmarks();
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].name, "Firefox");
    }
}
