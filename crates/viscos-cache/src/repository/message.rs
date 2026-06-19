//! `MessageRepository` trait + SQLite v1 implementation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{Connection, params};

use crate::cache::Message;
use crate::error::CacheError;

use super::open_sqlite_connection;

/// Thread-safe message repository abstraction. SQLite v1 implementasyonu
/// [`SqliteMessageRepository`]; gelecekte foyer-disk veya remote mock takılabilir.
pub trait MessageRepository: Send + Sync {
    /// Insert or replace a message. Idempotent: aynı `(channel_id, message_id)`
    /// tekrar geldiğinde üzerine yazar.
    fn upsert(&self, msg: &Message) -> Result<(), CacheError>;

    /// Look up a single message by composite key. `None` if missing.
    fn get(&self, channel_id: u64, message_id: u64) -> Result<Option<Message>, CacheError>;

    /// Most recent `limit` messages for `channel_id`, newest first.
    fn recent(&self, channel_id: u64, limit: u32) -> Result<Vec<Message>, CacheError>;

    /// Delete a single message. No-op if missing (idempotent).
    fn delete(&self, channel_id: u64, message_id: u64) -> Result<(), CacheError>;
}

/// SQLite-backed [`MessageRepository`]. Single connection with
/// `parking_lot::Mutex` — repository sync API'si v1'de async gerektirmiyor.
///
/// Connection-level PRAGMA'lar `open()` sırasında bir kez uygulanır; moka
/// facade katmanı gerekirse async `tokio::task::spawn_blocking` ile sarmalar.
pub struct SqliteMessageRepository {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl SqliteMessageRepository {
    /// Open (or create) a SQLite database at `path` and ensure the
    /// `cache_messages` schema is present. **Parent directory is auto-created**
    /// — regressing SQLITE_CANTOPEN errno 14 on first run.
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] on path open / pragma / schema apply failure.
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        let conn = open_sqlite_connection(path)?;
        ensure_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: path.to_path_buf(),
        })
    }

    /// Path backing this repository.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn ensure_schema(conn: &Connection) -> Result<(), CacheError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cache_messages (
             channel_id INTEGER NOT NULL,
             message_id INTEGER NOT NULL,
             author_id INTEGER NOT NULL,
             content TEXT NOT NULL,
             created_at INTEGER NOT NULL,
             PRIMARY KEY (channel_id, message_id)
         );
         CREATE INDEX IF NOT EXISTS messages_channel_created_idx
             ON cache_messages(channel_id, created_at DESC);",
    )?;
    Ok(())
}

impl MessageRepository for SqliteMessageRepository {
    fn upsert(&self, msg: &Message) -> Result<(), CacheError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO cache_messages (channel_id, message_id, author_id, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(channel_id, message_id) DO UPDATE SET
                 author_id = excluded.author_id,
                 content = excluded.content,
                 created_at = excluded.created_at",
            params![
                msg.channel_id,
                msg.id,
                msg.author_id.unwrap_or(0),
                msg.content,
                msg.timestamp,
            ],
        )?;
        Ok(())
    }

    fn get(&self, channel_id: u64, message_id: u64) -> Result<Option<Message>, CacheError> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT channel_id, message_id, author_id, content, created_at
             FROM cache_messages
             WHERE channel_id = ?1 AND message_id = ?2",
            params![channel_id, message_id],
            |row| {
                let author_raw: i64 = row.get(2)?;
                let author_id = u64::try_from(author_raw).ok();
                Ok(Message {
                    id: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                    channel_id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
                    author_id,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                })
            },
        );

        match result {
            Ok(msg) => Ok(Some(msg)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CacheError::Sqlite(e)),
        }
    }

    fn recent(&self, channel_id: u64, limit: u32) -> Result<Vec<Message>, CacheError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT channel_id, message_id, author_id, content, created_at
             FROM cache_messages
             WHERE channel_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![channel_id, limit as i64], |row| {
            let author_raw: i64 = row.get(2)?;
            let author_id = u64::try_from(author_raw).ok();
            Ok(Message {
                id: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                channel_id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
                author_id,
                content: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn delete(&self, channel_id: u64, message_id: u64) -> Result<(), CacheError> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM cache_messages WHERE channel_id = ?1 AND message_id = ?2",
            params![channel_id, message_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_db_path(prefix: &str) -> (TempDir, PathBuf) {
        let tmp = TempDir::new().expect("tempdir");
        let p = tmp.path().join(format!("{prefix}.db"));
        (tmp, p)
    }

    #[test]
    fn open_creates_parent_directories() {
        let tmp = TempDir::new().expect("tempdir");
        let nested = tmp.path().join("a").join("b").join("cache.db");
        let _ = SqliteMessageRepository::open(&nested).expect("open");
        assert!(nested.exists());
    }

    #[test]
    fn open_is_idempotent_schema() {
        let (_tmp, path) = temp_db_path("idem");
        let r1 = SqliteMessageRepository::open(&path).expect("first");
        let r2 = SqliteMessageRepository::open(&path).expect("second");
        assert_eq!(r1.path(), r2.path());
    }

    #[test]
    fn upsert_then_get_round_trip() {
        let (_tmp, path) = temp_db_path("rt");
        let repo = SqliteMessageRepository::open(&path).expect("open");
        let msg = Message::new(101, 7, "hello");
        repo.upsert(&msg).expect("upsert");

        let got = repo.get(7, 101).expect("get").expect("hit");
        assert_eq!(got.id, 101);
        assert_eq!(got.channel_id, 7);
        assert_eq!(got.content, "hello");
    }

    #[test]
    fn upsert_overwrites_existing() {
        let (_tmp, path) = temp_db_path("over");
        let repo = SqliteMessageRepository::open(&path).expect("open");
        repo.upsert(&Message::new(1, 1, "v1")).expect("upsert 1");
        repo.upsert(&Message::new(1, 1, "v2")).expect("upsert 2");
        let got = repo.get(1, 1).expect("get").expect("hit");
        assert_eq!(got.content, "v2");
    }

    #[test]
    fn get_missing_returns_none() {
        let (_tmp, path) = temp_db_path("miss");
        let repo = SqliteMessageRepository::open(&path).expect("open");
        let got = repo.get(99, 1000).expect("get");
        assert!(got.is_none());
    }

    #[test]
    fn recent_respects_limit_and_ordering() {
        let (_tmp, path) = temp_db_path("recent");
        let repo = SqliteMessageRepository::open(&path).expect("open");
        for i in 1..=5u64 {
            let mut m = Message::new(i, 42, format!("m{i}"));
            m.timestamp = i as i64;
            repo.upsert(&m).expect("upsert");
        }
        let mut other = Message::new(99, 7, "other");
        other.timestamp = 100;
        repo.upsert(&other).expect("upsert other");

        let recent = repo.recent(42, 3).expect("recent");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, 5);
        assert_eq!(recent[1].id, 4);
        assert_eq!(recent[2].id, 3);
    }

    #[test]
    fn delete_removes_entry() {
        let (_tmp, path) = temp_db_path("del");
        let repo = SqliteMessageRepository::open(&path).expect("open");
        repo.upsert(&Message::new(1, 1, "x")).expect("upsert");
        repo.delete(1, 1).expect("delete");
        assert!(repo.get(1, 1).expect("get").is_none());
    }

    #[test]
    fn delete_missing_is_noop() {
        let (_tmp, path) = temp_db_path("del-miss");
        let repo = SqliteMessageRepository::open(&path).expect("open");
        repo.delete(404, 404).expect("delete missing");
    }
}
