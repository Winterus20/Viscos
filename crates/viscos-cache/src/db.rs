//! `viscos-cache` database layer (rusqlite + r2d2 connection pool).
//!
//! Faz 4.0 Dalga 1 (ADR-0010): SQLite WAL mode + `synchronous=NORMAL`, 256MB mmap,
//! 64MB journal size limit. Connection pool `r2d2` ile yönetilir (`max_size=8`,
//! `min_idle=2`). Refinery runner migration'ları `migrations/` dizininden embed eder.
//!
//! ## Usage
//!
//! ```no_run
//! use viscos_cache::Db;
//! use std::path::PathBuf;
//!
//! let path = PathBuf::from("cache.db");
//! let db = Db::open(&path).expect("db open");
//! db.migrate().expect("migrate");
//! let conn = db.conn().expect("acquire connection");
//! ```

use std::path::Path;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OpenFlags;

use crate::error::{CacheError, Result};
use crate::migrations::runner;

/// SQLite-backed metadata DB. Connection pool + WAL + refinery migrations.
pub struct Db {
    pool: Pool<SqliteConnectionManager>,
}

impl Db {
    /// Open (or create) a SQLite database at the given path with WAL mode + pool.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Pool`] if r2d2 fails to build the pool (path
    /// not writable, max_size invalid, vb.). Connection acquisition failures
    /// sonradan [`Db::conn`] üzerinden gelir.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path)
            .with_flags(OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)
            .with_init(|c| {
                // Her yeni connection'a uygulanır — pool büyüdükçe de invariant.
                c.execute_batch(
                    "PRAGMA journal_mode = WAL;
                     PRAGMA synchronous = NORMAL;
                     PRAGMA temp_store = MEMORY;
                     PRAGMA mmap_size = 268435456;     -- 256 MB
                     PRAGMA journal_size_limit = 67108864; -- 64 MB
                     PRAGMA foreign_keys = ON;",
                )
            });

        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(2))
            .build(manager)?;

        Ok(Self { pool })
    }

    /// Run all pending refinery migrations (V001__initial.sql → Vxxx).
    ///
    /// Refinery `refinery_schema_history` tablosunu oluşturur ve uygulanmamış
    /// migration'ları sırayla çalıştırır. **Up + Down** migration'lar reversible
    /// (veri kaybı olmadan geri alınabilir).
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Migration`] on schema parse / SQL execution failures.
    pub fn migrate(&self) -> Result<()> {
        let mut conn = self.conn()?;
        runner()
            .run(&mut *conn)
            .map_err(|e| CacheError::Migration(format!("refinery runner: {e}")))?;
        Ok(())
    }

    /// Acquire a pooled connection. Blocks until a connection is available (or
    /// the pool is exhausted — then [`CacheError::Pool`]).
    pub fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    /// Raw pool reference (escape hatch — kullanım önerilmez, repository pattern tercih edilir).
    pub fn pool(&self) -> &Pool<SqliteConnectionManager> {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn open_creates_wal_mode_database() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cache.db");
        let db = Db::open(&path).expect("open");
        let conn = db.conn().expect("conn");

        // WAL mode hemen okunabilir (journal_mode read-only query).
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("journal_mode query");
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn migrate_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cache.db");
        let db = Db::open(&path).expect("open");

        db.migrate().expect("first migrate");
        // İkinci çağrı hata vermemeli (zaten uygulanmış migration'lar skip).
        db.migrate().expect("second migrate (idempotent)");
    }

    #[test]
    fn pool_acquires_multiple_connections_concurrently() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("cache.db");
        let db = Db::open(&path).expect("open");
        db.migrate().expect("migrate");

        // 8 connection paralel al — pool max_size kadarı OK.
        let mut conns = Vec::new();
        for _ in 0..8 {
            conns.push(db.conn().expect("conn"));
        }
        assert_eq!(conns.len(), 8);
        for c in conns {
            // Smoke test: her connection can execute a query.
            let _: i32 = c.query_row("SELECT 1", [], |r| r.get(0)).expect("query");
        }
    }
}
