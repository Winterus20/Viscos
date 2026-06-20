//! Cross-platform cache directory resolution (Faz 4.0 Dalga 1, ADR-0010 §2.1).
//!
//! `dirs::data_local_dir()` üzerinden OS-aware path üretir ve parent dizini
//! otomatik oluşturur (SQLITE_CANTOPEN errno 14 regresyonunu engeller).
//!
//! ## Platform paths
//!
//! - **Windows:** `%LOCALAPPDATA%\Viscos\cache\` (`C:\Users\<user>\AppData\Local\Viscos\cache\`).
//! - **Linux:** `~/.local/share/viscos/cache/` (XDG `XDG_DATA_HOME`).
//! - **macOS:** `~/Library/Application Support/Viscos/cache/`.
//!
//! Bu modülün bağımlılık zinciri dardır (`dirs` + `std::fs`); `viscos-cache`
//! kendi path üretimini yapmaz, konfigürasyondan okur.

use std::path::{Path, PathBuf};

use config::ConfigError;

/// OS-aware cache kök dizinini hesapla ve dizini oluştur.
///
/// Davranış:
/// 1. `dirs::data_local_dir()` çağrılır.
/// 2. Üzerine `viscos/cache` eklenir (final segment `cache/`).
/// 3. Tüm parent segmentler dahil dizin `create_dir_all` ile oluşturulur.
///
/// # Errors
///
/// [`ConfigError`] döner:
/// - `NotFound` — `dirs::data_local_dir()` OS'tan path döndüremedi (nadir;
///   Unix'te `$HOME` unset, Windows'ta `SHGetKnownFolderPath` failure).
/// - `Foreign` — parent dizin oluşturulamadı (permission denied, vb.).
///
/// # Examples
///
/// ```no_run
/// use viscos_config::resolve_cache_dir;
///
/// let dir = resolve_cache_dir().expect("cache dir");
/// assert!(dir.ends_with("Viscos/cache") || dir.ends_with("viscos/cache"));
/// ```
pub fn resolve_cache_dir() -> Result<PathBuf, ConfigError> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| ConfigError::NotFound("data_local_dir unavailable".to_string()))?;

    // OS-aware app segment: Windows convention PascalCase, POSIX lowercase.
    let app_segment: &str = if cfg!(windows) { "Viscos" } else { "viscos" };
    let cache_dir = base.join(app_segment).join("cache");

    ensure_dir_exists(&cache_dir)?;
    Ok(cache_dir)
}

/// Compute cache root inside a custom data directory (used by tests + callers
/// who override the platform default). Always creates the directory itself
/// (and any missing parents).
///
/// This helper does not consult `dirs::data_local_dir()` — it is the explicit
/// path used by [`CacheConfig::new`](crate::CacheConfig::new).
pub fn resolve_cache_dir_in(data_dir: &Path) -> Result<PathBuf, ConfigError> {
    ensure_dir_exists(data_dir)?;
    Ok(data_dir.to_path_buf())
}

fn ensure_dir_exists(path: &Path) -> Result<(), ConfigError> {
    std::fs::create_dir_all(path).map_err(|e| {
        ConfigError::Foreign(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_cache_dir_creates_and_returns_local_app_dir() {
        let dir = resolve_cache_dir().expect("resolve");
        // Suffix check — platform'a göre farklı prefix olabilir.
        let suffix = if cfg!(windows) {
            "Viscos\\cache"
        } else {
            "cache"
        };
        let rendered = dir.to_string_lossy();
        assert!(
            rendered.contains(suffix),
            "expected cache dir to end with '{suffix}', got {rendered}"
        );
    }

    #[test]
    fn resolve_cache_dir_creates_parent_recursively() {
        // İkinci çağrı idempotent olmalı — parent zaten var, hata vermemeli.
        let first = resolve_cache_dir().expect("first resolve");
        let second = resolve_cache_dir().expect("second resolve");
        assert_eq!(first, second);
        assert!(first.exists(), "cache dir must exist on disk");
    }

    #[test]
    fn resolve_cache_dir_in_creates_nested_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("a").join("b").join("cache");
        let result = resolve_cache_dir_in(&nested).expect("resolve");
        assert_eq!(result, nested);
        assert!(nested.exists());
    }

    #[test]
    fn resolve_cache_dir_in_accepts_existing_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let existing = tmp.path().join("cache");
        std::fs::create_dir_all(&existing).expect("precreate");
        let result = resolve_cache_dir_in(&existing).expect("resolve existing");
        assert_eq!(result, existing);
    }
}
