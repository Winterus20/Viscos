//! Typed errors for `viscos-telemetry`.
//!
//! `#[non_exhaustive]` ile AI veya insan yeni variant ekleyebilir; dış tüketiciler
//! için breaking change olmaz. ADR-0007 ile uyumlu: library boundary typed,
//! application boundary (`viscos` binary main) `anyhow` kullanır.

use thiserror::Error;

/// `viscos-telemetry` library boundary hatası.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TelemetryError {
    /// SQLite işlem hatası (rusqlite). Genelde SQL syntax / I/O / constraint.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// IO hatası (parent dizin create_dir_all, vb.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// `viscos-telemetry` Result tipi.
pub type Result<T> = std::result::Result<T, TelemetryError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_error_converts_to_telemetry_error() {
        let sqlite_err = rusqlite::Error::QueryReturnedNoRows;
        let err: TelemetryError = sqlite_err.into();
        assert!(matches!(err, TelemetryError::Sqlite(_)));
    }

    #[test]
    fn io_error_converts_to_telemetry_error() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: TelemetryError = io.into();
        assert!(matches!(err, TelemetryError::Io(_)));
    }
}
