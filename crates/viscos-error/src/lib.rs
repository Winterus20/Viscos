//! `viscos-error` — typed error enum + anyhow re-export.
//!
//! ADR-0007: library boundary typed (`ViscosError`), application boundary (`anyhow`).
//! Public API her zaman `Result<T, ViscosError>` döner. `anyhow` yalnızca `viscos`
//! binary'sinin `main`'inde ve internal glue katmanında kullanılır.

use thiserror::Error;

/// Viscos'un library boundary hatası. `#[non_exhaustive]` sayesinde AI veya insan
/// yeni variant ekleyebilir → dış tüketiciler için breaking change olmaz.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ViscosError {
    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not yet implemented: {0}")]
    Unimplemented(&'static str),

    #[error("media error: {0}")]
    Media(String),
    // NOT: anyhow::Error variant YOK. Library sınırında typed hata döndür;
    // anyhow yalnızca application boundary (main, glue code) içinde.
    // Gerekçe: tüketicinin match edebilmesi için somut tip kalmalı.
    //
    // NOT: `Api` ve `Auth` varyantları kendi crate'lerinde typed kalır; tüketici
    // tarafında explicit match için `viscos_api::ApiError` / `viscos_auth::AuthError`
    // kullanılır. `viscos-error` diğer crate'lere bağımlı olmaz (dependency cycle yok).
}

/// Viscos library Result tipi.
pub type Result<T> = std::result::Result<T, ViscosError>;

// anyhow'yi re-export et: application boundary code (main, glue) için.
// Kütüphane crate'leri doğrudan `anyhow` kullanmaz — `ViscosError` döner.
pub use anyhow;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_io_error() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: ViscosError = io.into();
        assert!(matches!(err, ViscosError::Io(_)));
    }

    #[test]
    fn unimplemented_display() {
        let err = ViscosError::Unimplemented("phase-1.0 window");
        assert_eq!(err.to_string(), "not yet implemented: phase-1.0 window");
    }

    #[test]
    fn non_exhaustive_compiles() {
        // Sadece derleme zamanı güvencesi: `#[non_exhaustive]` olduğu için
        // dışarıdan exhaustive match yapılamaz. Burada wildcard ile yakalıyoruz.
        let err = ViscosError::Unimplemented("test");
        match err {
            ViscosError::Unimplemented(_) => {}
            _ => panic!("unreachable in this test"),
        }
    }
}
