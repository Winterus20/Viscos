//! `GuildRepository` trait + SQLite v1 implementation.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::CacheError;

use super::open_sqlite_connection;

/// Guild row (cache_facade namespace).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Guild {
    /// Discord guild snowflake.
    pub id: u64,
    /// Display name.
    pub name: String,
}

/// Guild upsert + list. v1 minimal: sadece DM list yükleme senaryosu için.
pub trait GuildRepository: Send + Sync {
    fn upsert(&self, guild: &Guild) -> Result<(), CacheError>;
    fn list(&self) -> Result<Vec<Guild>, CacheError>;
}

/// SQLite-backed [`GuildRepository`]. Separate connection (v1) — pool
/// orchestration later.
pub struct SqliteGuildRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteGuildRepository {
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] on open / schema apply failure.
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        let conn = open_sqlite_connection(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cache_guilds (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL
             );",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl GuildRepository for SqliteGuildRepository {
    fn upsert(&self, guild: &Guild) -> Result<(), CacheError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO cache_guilds (id, name) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name",
            params![guild.id, guild.name],
        )?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<Guild>, CacheError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id, name FROM cache_guilds ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok(Guild {
                id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
                name: row.get(1)?,
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
    fn guild_repository_round_trip() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("guilds.db");
        let repo = SqliteGuildRepository::open(&path).expect("open");
        repo.upsert(&Guild {
            id: 1,
            name: "alpha".into(),
        })
        .expect("upsert");
        repo.upsert(&Guild {
            id: 2,
            name: "beta".into(),
        })
        .expect("upsert");
        let list = repo.list().expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "beta");
    }

    #[test]
    fn upsert_overwrites_existing() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("guilds.db");
        let repo = SqliteGuildRepository::open(&path).expect("open");
        repo.upsert(&Guild {
            id: 1,
            name: "old".into(),
        })
        .expect("upsert 1");
        repo.upsert(&Guild {
            id: 1,
            name: "new".into(),
        })
        .expect("upsert 2");
        let list = repo.list().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "new");
    }
}
