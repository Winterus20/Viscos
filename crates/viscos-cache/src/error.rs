//! Typed errors for `viscos-cache`.
//!
//! `#[non_exhaustive]` ile AI veya insan yeni variant ekleyebilir; dış tüketiciler
//! için breaking change olmaz. ADR-0007 ile uyumlu: library boundary typed, application
//! boundary (`viscos` binary main) `anyhow` kullanır.

use thiserror::Error;

/// `viscos-cache` library boundary hatası.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CacheError {
    /// SQLite işlem hatası (rusqlite). Genelde SQL syntax / I/O / constraint violation.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// JSON payload shape mismatch (Discord Gateway payload → `Message` adapter).
    #[error("json error: {0}")]
    Json(String),

    /// I/O hatası (parent dir create, file read, vb.).
    #[error("io error: {0}")]
    Io(String),

    /// Connection pool hatası (r2d2). Genelde pool exhausted veya connection broken.
    #[error("connection pool error: {0}")]
    Pool(String),

    /// Refinery migration hatası (schema uygulama veya version mismatch).
    #[error("migration error: {0}")]
    Migration(String),

    /// moka cache hatası (genelde nadir — capacity invalid, vb.).
    #[error("moka cache error: {0}")]
    Moka(String),

    /// Tokio task join hatası (spawn_blocking panic / cancellation).
    #[error("task join error: {0}")]
    Join(String),

    /// Config parse hatası (örn. `CacheTiers` from TOML).
    #[error("config error: {0}")]
    Config(#[from] viscos_error::anyhow::Error),
}

// `r2d2::Error` ve `refinery::Error` için manuel `From` impl'leri —
// kendi Display mesajlarımızla sarmalayarak anlamlı error chain üretiriz.

impl From<r2d2::Error> for CacheError {
    fn from(err: r2d2::Error) -> Self {
        Self::Pool(err.to_string())
    }
}

impl From<refinery::Error> for CacheError {
    fn from(err: refinery::Error) -> Self {
        Self::Migration(err.to_string())
    }
}

impl From<std::io::Error> for CacheError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// `viscos-cache` Result tipi.
pub type Result<T> = std::result::Result<T, CacheError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_error_converts_to_cache_error() {
        let sqlite_err = rusqlite::Error::QueryReturnedNoRows;
        let err: CacheError = sqlite_err.into();
        assert!(matches!(err, CacheError::Sqlite(_)));
    }

    #[test]
    fn pool_error_converts_to_cache_error() {
        // r2d2::Error'un Display'lenebilir bir formu yok; manuel Error üretmiyoruz.
        // Sadece From impl'in compile-time doğrulaması yeterli.
        let _ = std::marker::PhantomData::<fn() -> CacheError>;
        fn assert_from<T: Into<CacheError>>() {}
        assert_from::<r2d2::Error>();
        assert_from::<refinery::Error>();
    }

    #[test]
    fn non_exhaustive_compiles() {
        let err = CacheError::Pool("test".to_string());
        match err {
            CacheError::Pool(_) => {}
            _ => panic!("unreachable"),
        }
    }
}
