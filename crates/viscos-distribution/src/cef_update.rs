//! CEF runtime self-update — Faz 8.5 (Chromium security feed stub).
//!
//! Faz 1.5 telemetry Faz 8.5'teki `ChromiumAdvisoryFeed`'i devralır; Faz 8.5
//! self-update flow'u Faz 8.0 `Updater` ile entegredir.
//!
//! Güncelleme stratejisi (ADR-0012 §CefUpdate):
//! - Routine haftalık kontrol.
//! - Kritik CVE tetikleyici (Son 7 gün `Severity: Critical`).
//! - Aylık major baseline.
//!
//! Cross-reference:
//! - [`phase-8.5-cef-backend.md` §6](../../.cursor/plans/phase-8.5-cef-backend.md#6-cef-self-update-faz-80-ile-entegre)

use serde::{Deserialize, Serialize};
use thiserror::Error;
use viscos_error::{Result, ViscosError};

/// CEF release bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CefRelease {
    /// CEF versiyonu (ör. `148.3.0`).
    pub version: String,
    /// Download URL'i (CEF builds CDN).
    pub download_url: String,
    /// SHA-256 hex digest.
    pub sha256: String,
    /// Release date (RFC 3339).
    pub release_date: String,
}

/// Güncelleme tetikleyicisi (Faz 1.5 telemetry + Faz 8.5 update stratejisi).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CefUpdateTrigger {
    /// Faz 8.5 plan'ında aylık major baseline.
    ScheduledMonthly,
    /// ADR-0012 §CefUpdate — routine haftalık kontrol.
    ScheduledWeekly,
    /// ADR-0012 §CefUpdate — kritik CVE tespit edildi.
    CriticalCveDetected,
}

/// CEF updater konfigürasyonu.
#[derive(Debug, Clone)]
pub struct CefUpdater {
    /// Chromium builds JSON feed URL'i.
    pub feed_url: String,
}

impl Default for CefUpdater {
    fn default() -> Self {
        Self {
            feed_url: "https://cef-builds.spotifycdn.com/index.json".to_string(),
        }
    }
}

/// CEF updater hatası.
#[derive(Error, Debug)]
pub enum CefUpdateError {
    #[error("feed fetch failed: {0}")]
    Fetch(String),
    #[error("feed parse failed: {0}")]
    Parse(String),
    #[error("hash verification failed: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
}

impl From<CefUpdateError> for ViscosError {
    fn from(err: CefUpdateError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("cef-update: {err}")))
    }
}

impl CefUpdater {
    /// Yeni CEF updater.
    #[must_use]
    pub fn new(feed_url: impl Into<String>) -> Self {
        Self {
            feed_url: feed_url.into(),
        }
    }

    /// Default feed URL'i ile başlat.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::default()
    }

    /// Yeni CEF release var mı kontrol et.
    ///
    /// Faz 8.5 stub: `Ok(None)` döner. Faz 8.x'te:
    /// 1. `feed_url`'den JSON parse (`tauri-apps/cef-rs` upstream cadence).
    /// 2. Faz 1.5'teki `ChromiumAdvisoryFeed` ile kritik CVE tespiti.
    /// 3. `current_version < latest_version` → `CefRelease` döner.
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda feed
    /// fetch/parse hatası `CefUpdateError` olarak dönecek.
    pub async fn check(&self) -> Result<Option<CefRelease>> {
        tracing::debug!(
            feed_url = %self.feed_url,
            "CefUpdater::check stub — Chromium advisory feed scrape Faz 8.x'te"
        );
        Ok(None)
    }

    /// Belirtilen release'i indir + DLL'leri replace et.
    ///
    /// Faz 8.5 stub: sadece loglar + SHA-256 verify'ın compile-time
    /// doğruluğunu test eder.
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda download
    /// ve hash mismatch hataları `CefUpdateError` varyantları olarak dönecek.
    pub async fn apply(&self, release: CefRelease) -> Result<()> {
        tracing::info!(
            version = %release.version,
            sha256_prefix = %&release.sha256[..8.min(release.sha256.len())],
            "CefUpdater::apply stub — binary replace Faz 8.x'te"
        );
        // SHA-256 hash tipi compile-time doğrulaması: eğer 64 hex char değilse
        // future real impl'da hash mismatch olur; stub'da skip.
        if release.sha256.len() != 64 {
            tracing::warn!(
                sha256_len = release.sha256.len(),
                "sha256 length 64 char değil — release engineering placeholder?"
            );
        }
        Ok(())
    }

    /// Trigger'a göre kullanıcı bildirim metni.
    #[must_use]
    pub fn notification_text(trigger: CefUpdateTrigger) -> &'static str {
        match trigger {
            CefUpdateTrigger::ScheduledMonthly => {
                "Viscos: Aylık CEF güncellemesi — Chromium runtime yenilendi."
            }
            CefUpdateTrigger::ScheduledWeekly => {
                "Viscos: Haftalık CEF kontrolü — yeni sürüm mevcut olabilir."
            }
            CefUpdateTrigger::CriticalCveDetected => {
                "Viscos: Kritik Chromium güvenlik güncellemesi — restart önerilir."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_feed_url_is_spotify_cdn() {
        let updater = CefUpdater::default();
        assert!(updater.feed_url.contains("cef-builds"));
        assert!(updater.feed_url.starts_with("https://"));
    }

    #[tokio::test]
    async fn check_returns_none_in_phase_8_5_stub() {
        let updater = CefUpdater::default();
        let result = updater.check().await.expect("check stub");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn apply_stub_succeeds_with_valid_release() {
        let updater = CefUpdater::default();
        let release = CefRelease {
            version: "148.3.0".to_string(),
            download_url: "https://cef-builds.spotifycdn.com/cef_binary_148.3.0.tar.bz2"
                .to_string(),
            sha256: "0".repeat(64),
            release_date: "2026-06-01".to_string(),
        };
        updater.apply(release).await.expect("apply stub");
    }

    #[test]
    fn notification_text_differs_by_trigger() {
        let monthly = CefUpdater::notification_text(CefUpdateTrigger::ScheduledMonthly);
        let weekly = CefUpdater::notification_text(CefUpdateTrigger::ScheduledWeekly);
        let critical = CefUpdater::notification_text(CefUpdateTrigger::CriticalCveDetected);

        assert!(monthly.contains("Aylık"));
        assert!(weekly.contains("Haftalık"));
        assert!(critical.contains("Kritik"));
        assert_ne!(monthly, weekly);
        assert_ne!(weekly, critical);
    }

    #[test]
    fn new_accepts_custom_feed_url() {
        let updater = CefUpdater::new("https://example.com/index.json");
        assert_eq!(updater.feed_url, "https://example.com/index.json");
    }
}
