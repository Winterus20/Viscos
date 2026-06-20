//! Repository pattern for `viscos-cache` (Faz 4.0 Dalga 1, ADR-0010).
//!
//! Repository trait'leri `Send + Sync` thread-safe; SQLite implementasyonları
//! `Arc<Mutex<Connection>>` ile korunur. İleride `foyer` disk, mock veya
//! remote repository aynı trait üzerinden takılabilir (PR-5+ scope).
//!
//! ## Tables
//!
//! Yeni repository tabloları `cache_` prefix'i taşır — V001__initial.sql
//! içindeki `messages/guilds/channels` tablolarından ayrışır (farklı
//! şema + PR-3 scope yaratmamak için). Refinery'nin `refinery_schema_history`
//! tablosunu görmezden gelir; idempotent `CREATE TABLE IF NOT EXISTS` kullanır.

use std::path::Path;

use rusqlite::OpenFlags;

use crate::error::CacheError;

mod channel;
mod guild;
mod message;

pub use channel::{Channel, ChannelRepository, SqliteChannelRepository};
pub use guild::{Guild, GuildRepository, SqliteGuildRepository};
pub use message::{MessageRepository, SqliteMessageRepository};

/// Helper: open a SQLite connection at `path` with WAL mode + parent
/// auto-create. Shared by the three repository impls below.
pub(crate) fn open_sqlite_connection(path: &Path) -> Result<rusqlite::Connection, CacheError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CacheError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(e),
            ))
        })?;
    }
    let conn = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(conn)
}
