//! WebView2 periyodik refresh (Faz 8.0 stub).
//!
//! Faz 8.0 deliverable: `WebView2Refresher::refresh_if_needed()` periyodik
//! recreate kontrolü. Faz 1.0'da no-op; gerçek `WebViewController::Close()`
//! + recreate Faz 8.x'te WebView2 lifecycle finalize edildikten sonra.
//!
//! Karar verisi (Faz 1 24h soak test):
//! - 6 saatte bir refresh → GDI 8000'e ulaşmadan önce reset
//! - Watchdog tetikli refresh → sadece kritik durumda (Faz 1'de hazır)
//!
//! Cross-reference:
//! - [`phase-8.0-distribution.md` §2.1](../../.cursor/plans/phase-8.0-distribution.md#21-periyodik-refresh)

use std::time::Duration;

use viscos_error::Result;

/// WebView2 periyodik refresh konfigürasyonu (Faz 8.0 stub).
#[derive(Debug, Clone)]
pub struct WebViewRefreshConfig {
    /// Kanal değişiminde recreate tetikle.
    pub channel_change: bool,
    /// Watchdog uyarısında recreate tetikle (Faz 1'de aktif).
    pub watchdog_trigger: bool,
    /// Zaman bazlı recreate interval. `None` ise disabled.
    pub time_based: Option<Duration>,
}

impl Default for WebViewRefreshConfig {
    fn default() -> Self {
        Self {
            channel_change: false,
            watchdog_trigger: true,
            time_based: Some(Duration::from_secs(6 * 3600)),
        }
    }
}

/// WebView2 periyodik refresher (Faz 8.0 stub).
///
/// Faz 1.0 + Faz 8.0 davranışı: `refresh_if_needed()` her zaman `Ok(false)` döner
/// (no recreate tetiklendi). Faz 8.x'te config + son recreate zamanı + watchdog
/// state'i birleştirilerek gerçek recreate tetiklenecek.
#[derive(Debug, Clone, Default)]
pub struct WebView2Refresher {
    config: WebViewRefreshConfig,
}

impl WebView2Refresher {
    /// Yeni refresher (default config ile).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Custom config ile.
    #[must_use]
    pub fn with_config(config: WebViewRefreshConfig) -> Self {
        Self { config }
    }

    /// Refresh tetiklenmeli mi? Stub: her zaman `false`.
    ///
    /// # Errors
    ///
    /// Faz 8.0 stub: hata dönmez. Faz 8.x'te WebView2 API hataları.
    pub async fn refresh_if_needed(&self) -> Result<bool> {
        tracing::debug!(
            channel_change = self.config.channel_change,
            watchdog_trigger = self.config.watchdog_trigger,
            time_based_secs = ?self.config.time_based.map(|d| d.as_secs()),
            "WebView2Refresher::refresh_if_needed stub — gerçek recreate Faz 8.x'te"
        );
        Ok(false)
    }

    /// Config'i döndür.
    #[must_use]
    pub const fn config(&self) -> &WebViewRefreshConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_6h_time_based_and_watchdog() {
        let cfg = WebViewRefreshConfig::default();
        assert!(
            !cfg.channel_change,
            "Faz 1 verisi: agresif recreate UX'i bozar"
        );
        assert!(cfg.watchdog_trigger, "Watchdog zaten recreate tetikliyor");
        assert_eq!(cfg.time_based, Some(Duration::from_secs(6 * 3600)));
    }

    #[tokio::test]
    async fn refresh_if_needed_returns_false_in_phase_8_0_stub() {
        let refresher = WebView2Refresher::new();
        let triggered = refresher.refresh_if_needed().await.expect("stub");
        assert!(!triggered, "stub must report no refresh triggered");
    }

    #[test]
    fn with_config_overrides_default() {
        let cfg = WebViewRefreshConfig {
            channel_change: true,
            watchdog_trigger: false,
            time_based: None,
        };
        let refresher = WebView2Refresher::with_config(cfg);
        assert!(refresher.config().channel_change);
        assert!(!refresher.config().watchdog_trigger);
        assert_eq!(refresher.config().time_based, None);
    }
}
