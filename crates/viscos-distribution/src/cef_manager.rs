//! CEF runtime management — Faz 8.5 (default-out management).
//!
//! Faz 1.6'da CEF Win11 default olarak MVP'ye alındı. Faz 8.5 artık
//! **backend management infrastructure** sağlıyor:
//! - `CefManager::detect_installed()` → CEF binary varlık kontrolü.
//! - `CefManager::current_backend()` → aktif backend seçimi.
//! - `CefManager::set_default_backend()` → config.toml'a yaz.
//!
//! Gerçek `CefWebView2Backend::create_window()` implementasyonu Faz 1.6
//! worker'ına ait; bu modül yalnızca **management API**'yi kurar.
//!
//! Cross-references:
//! - [`phase-8.5-cef-backend.md`](../../.cursor/plans/phase-8.5-cef-backend.md)
//! - [`phase-1.6-cef-default-rollout.md`](../../.cursor/plans/phase-1.6-cef-default-rollout.md)
//! - [`webview2-hardening.md` Katman 3](../../.cursor/plans/webview2-hardening.md#katman-3-cef-backend-faz-16--win11-default-mvpnin-parçası)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use viscos_config::Config;
use viscos_error::{Result, ViscosError};
use viscos_webview::{BackendKind, select_default_backend};

/// CEF manager seçim backend'i (re-export `viscos-webview`'den).
///
/// Faz 8.5'te `Config`'ten de okunabilir hale getiriyoruz (string → enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CefBackendChoice {
    /// Microsoft Edge WebView2 — hafif, OS WebView.
    WebView2,
    /// Chromium Embedded Framework — leak'siz, RDP güvenli.
    Cef,
}

impl From<BackendKind> for CefBackendChoice {
    fn from(kind: BackendKind) -> Self {
        match kind {
            BackendKind::WebView2 => Self::WebView2,
            BackendKind::Cef => Self::Cef,
        }
    }
}

impl From<CefBackendChoice> for BackendKind {
    fn from(choice: CefBackendChoice) -> Self {
        match choice {
            CefBackendChoice::WebView2 => Self::WebView2,
            CefBackendChoice::Cef => Self::Cef,
        }
    }
}

impl CefBackendChoice {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WebView2 => "webview2",
            Self::Cef => "cef",
        }
    }
}

/// CEF runtime tespit sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CefManager {
    /// CEF versiyonu (ör. `148.3.0`). Boş ise CEF kurulu değil.
    pub version: String,
    /// CEF install path (`{data_dir}/cef` default).
    pub install_path: PathBuf,
}

impl CefManager {
    /// Yeni `CefManager` kaydı.
    #[must_use]
    pub fn new(version: impl Into<String>, install_path: PathBuf) -> Self {
        Self {
            version: version.into(),
            install_path,
        }
    }

    /// Sistemde kurulu CEF runtime var mı?
    ///
    /// Faz 8.5 stub: `Ok(None)` döner — gerçek dosya varlık kontrolü Faz 8.x'te.
    /// Faz 1.6 ile birlikte CEF runtime binary'si (`chromium-sandbox.exe`,
    /// `libcef.dll`) ile detection yapılacak.
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda I/O hataları
    /// `CefManagerError::Scan` olarak dönecek.
    pub fn detect_installed(_config: &Config) -> Result<Option<Self>> {
        tracing::debug!("CefManager::detect_installed stub — binary scan Faz 8.x'te");
        Ok(None)
    }

    /// Config + runtime detection'dan aktif backend'i hesapla.
    ///
    /// Öncelik sırası (Faz 1.6 kararı, ADR-0012 §4):
    /// 1. `Config.webview.backend == "cef"` veya `"webview2"` → explicit override.
    /// 2. `Config.webview.backend == "auto"` → `select_default_backend()` (Win11 → CEF).
    #[must_use]
    pub fn current_backend(config: &Config) -> CefBackendChoice {
        match config.webview.backend.as_str() {
            "cef" => CefBackendChoice::Cef,
            "webview2" => CefBackendChoice::WebView2,
            _ => select_default_backend(None).into(),
        }
    }

    /// Default backend'i config.toml'a yaz.
    ///
    /// Faz 8.5 stub: sadece loglar, gerçek dosya yazımı Faz 8.x'te.
    /// `Config` tipi şu an read-only (config-rs deserialize); persistence için
    /// `Config::serialize()` çıktısı `config/local.toml` üzerine yazılacak.
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda I/O hataları
    /// `CefManagerError::Persist` olarak dönecek.
    pub fn set_default_backend(_config: &Config, choice: CefBackendChoice) -> Result<()> {
        tracing::info!(
            backend = choice.as_str(),
            "CefManager::set_default_backend stub — config persistence Faz 8.x'te"
        );
        Ok(())
    }

    /// CEF install path'inin varlığını kontrol et (utility).
    #[must_use]
    pub fn is_path_valid(&self) -> bool {
        !self.install_path.as_os_str().is_empty()
    }

    /// CEF kurulu mu? (version boş değil + path set edilmiş).
    #[must_use]
    pub fn is_installed(&self) -> bool {
        !self.version.is_empty() && self.is_path_valid()
    }

    /// CEF cache dizini (path altında `cache/`).
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        let mut p = self.install_path.clone();
        p.push("cache");
        p
    }
}

/// CEF manager hatası.
#[derive(Error, Debug)]
pub enum CefManagerError {
    #[error("invalid backend choice string: {0}")]
    InvalidChoice(String),
    #[error("config persistence failed: {0}")]
    Persist(String),
    #[error("CEF binary scan failed: {0}")]
    Scan(String),
}

impl From<CefManagerError> for ViscosError {
    fn from(err: CefManagerError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("cef-manager: {err}")))
    }
}

/// Helper: path'in var olup olmadığını kontrol et (gerçek detection Faz 8.x).
#[must_use]
pub fn path_exists(p: &Path) -> bool {
    p.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_backend_matches_select_default_when_auto() {
        let cfg = Config::default();
        let backend = CefManager::current_backend(&cfg);
        let expected: CefBackendChoice = select_default_backend(None).into();
        assert_eq!(backend, expected);
    }

    #[test]
    fn current_backend_respects_explicit_cef_override() {
        let mut cfg = Config::default();
        cfg.webview.backend = "cef".to_string();
        assert_eq!(CefManager::current_backend(&cfg), CefBackendChoice::Cef);
    }

    #[test]
    fn current_backend_respects_explicit_webview2_override() {
        let mut cfg = Config::default();
        cfg.webview.backend = "webview2".to_string();
        assert_eq!(
            CefManager::current_backend(&cfg),
            CefBackendChoice::WebView2
        );
    }

    #[test]
    fn cef_manager_installed_status_reflects_fields() {
        let empty = CefManager::new("", PathBuf::from("/"));
        assert!(
            !empty.is_installed(),
            "empty version + empty path → not installed"
        );

        let no_version = CefManager::new("", PathBuf::from("/tmp/cef"));
        assert!(
            !no_version.is_installed(),
            "empty version should keep status false"
        );

        let installed = CefManager::new("148.3.0", PathBuf::from("/tmp/cef"));
        assert!(installed.is_installed());
        assert!(installed.is_path_valid());
    }

    #[test]
    fn cache_dir_is_install_path_plus_cache() {
        let mgr = CefManager::new("148.3.0", PathBuf::from("/tmp/cef"));
        let cache = mgr.cache_dir();
        assert_eq!(cache, PathBuf::from("/tmp/cef/cache"));
    }

    #[test]
    fn detect_installed_returns_none_in_phase_8_5_stub() {
        let cfg = Config::default();
        let result = CefManager::detect_installed(&cfg).expect("stub");
        assert!(result.is_none());
    }

    #[test]
    fn set_default_backend_stub_succeeds() {
        let cfg = Config::default();
        assert!(CefManager::set_default_backend(&cfg, CefBackendChoice::Cef).is_ok());
        assert!(CefManager::set_default_backend(&cfg, CefBackendChoice::WebView2).is_ok());
    }

    #[test]
    fn choice_as_str_is_lowercase() {
        assert_eq!(CefBackendChoice::WebView2.as_str(), "webview2");
        assert_eq!(CefBackendChoice::Cef.as_str(), "cef");
    }

    #[test]
    fn choice_conversions_round_trip() {
        let webview2: CefBackendChoice = BackendKind::WebView2.into();
        let cef: CefBackendChoice = BackendKind::Cef.into();
        assert_eq!(webview2, CefBackendChoice::WebView2);
        assert_eq!(cef, CefBackendChoice::Cef);

        let back: BackendKind = webview2.into();
        let back2: BackendKind = cef.into();
        assert_eq!(back, BackendKind::WebView2);
        assert_eq!(back2, BackendKind::Cef);
    }
}
