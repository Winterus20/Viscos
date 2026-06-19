//! `ChannelRepository` trait + SQLite v1 implementation.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::CacheError;

use super::open_sqlite_connection;

/// Channel row (cache_facade namespace).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Channel {
    /// Discord channel snowflake.
    pub id: u64,
    /// Parent guild snowflake (`None` for DMs).
    pub guild_id: Option<u64>,
    /// Channel name (`#general` gibi).
    pub name: String,
    /// Channel kind (0=text, 2=voice, 4=category, ...).
    pub kind: i32,
}

/// Channel upsert + list. v1 minimal.
pub trait ChannelRepository: Send + Sync {
    fn upsert(&self, channel: &Channel) -> Result<(), CacheError>;
    fn list(&self) -> Result<Vec<Channel>, CacheError>;
}

/// SQLite-backed [`ChannelRepository`].
pub struct SqliteChannelRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteChannelRepository {
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] on open / schema apply failure.
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        let conn = open_sqlite_connection(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cache_channels (
                 id INTEGER PRIMARY KEY,
                 guild_id INTEGER,
                 name TEXT NOT NULL,
                 kind INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS cache_channels_guild_idx
                 ON cache_channels(guild_id);",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl ChannelRepository for SqliteChannelRepository {
    fn upsert(&self, channel: &Channel) -> Result<(), CacheError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO cache_channels (id, guild_id, name, kind) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 guild_id = excluded.guild_id,
                 name = excluded.name,
                 kind = excluded.kind",
            params![channel.id, channel.guild_id, channel.name, channel.kind],
        )?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<Channel>, CacheError> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT id, guild_id, name, kind FROM cache_channels ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            let guild_raw: Option<i64> = row.get(1)?;
            Ok(Channel {
                id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
                guild_id: guild_raw.and_then(|v| u64::try_from(v).ok()),
                name: row.get(2)?,
                kind: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn channel_repository_round_trip() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("channels.db");
        let repo = SqliteChannelRepository::open(&path).expect("open");
        repo.upsert(&Channel {
            id: 100,
            guild_id: Some(1),
            name: "general".into(),
            kind: 0,
        })
        .expect("upsert");
        repo.upsert(&Channel {
            id: 101,
            guild_id: None,
            name: "DM".into(),
            kind: 1,
        })
        .expect("upsert dm");
        let list = repo.list().expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].guild_id, Some(1));
        assert_eq!(list[1].guild_id, None);
    }

    #[test]
    fn upsert_overwrites_existing() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("channels.db");
        let repo = SqliteChannelRepository::open(&path).expect("open");
        repo.upsert(&Channel {
            id: 1,
            guild_id: Some(1),
            name: "old".into(),
            kind: 0,
        })
        .expect("upsert 1");
        repo.upsert(&Channel {
            id: 1,
            guild_id: Some(1),
            name: "new".into(),
            kind: 0,
        })
        .expect("upsert 2");
        let list = repo.list().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "new");
    }
}
