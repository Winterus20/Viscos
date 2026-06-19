//! `Watchdog` — periyodik GDI sample + threshold tetikleme.
//!
//! Faz 1.0: stub implementation — sample alır, loglar, restart signal emit
//! eder. Gerçek WebView dispose+recreate Faz 1.6'da.
//!
//! MVP-3 (Polish): watchdog opsiyonel olarak `TelemetrySink` alır — her
//! sample'da `on_sample` callback'i tetiklenir, SQLite time-series
//! doldurulur. Hook `None` ise MVP-2 davranışı (sadece log) korunur.
//!
//! Cross-reference: [`webview2-hardening.md` Katman 1](../../.cursor/plans/webview2-hardening.md#katman-1-watchdog-faz-1--viscos-watchdog-crate).

use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::autosave::DraftAutosave;
use crate::gdi::GdiCounter;
use crate::restart::{RestartReason, RestartSignal};

use crate::{DEFAULT_GDI_CRITICAL, DEFAULT_GDI_WARNING};

/// Watchdog konfigürasyonu.
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// GDI warning threshold (default: 7000).
    pub gdi_warning: u32,
    /// GDI critical threshold (default: 9000) — restart tetikleyici.
    pub gdi_critical: u32,
    /// Sample alma aralığı (default: 30s).
    pub sample_interval: Duration,
    /// İlk kaç sample warmup olarak skip edilsin (default: 2).
    /// İlk sample'lar process başlangıcında doğal olarak yüksek; baseline
    /// stabilize olması için bekle.
    pub warmup_samples: u32,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            gdi_warning: DEFAULT_GDI_WARNING,
            gdi_critical: DEFAULT_GDI_CRITICAL,
            sample_interval: Duration::from_secs(30),
            warmup_samples: 2,
        }
    }
}

/// MVP-3 — Telemetry callback kontratı (viscos-telemetry ile uyumlu).
///
/// `Watchdog` her GDI sample aldığında bu trait'in `on_sample` method'unu
/// çağırır. `viscos_telemetry::TelemetrySink` (ve onun `TelemetryStoreSink`
/// adaptörü) implementasyonu SQLite'a yazar; başka implementasyonlar
/// (in-memory aggregator, vs.) da mümkün.
///
/// **`Send + Sync`:** Watchdog task tek bir thread'de çalışır ancak bu
/// constraint ilerideki multi-thread sample stratejileri için gerekli.
///
/// **Reason format:** `RestartReason::as_str()` (örn. `"GdiLeakCritical"`,
/// `"IpcBufferCritical"`). `viscos-telemetry` watchdog'a bağımlılık yaratmaz
/// (dependency cycle); reason string olarak taşınır.
pub trait TelemetrySink: Send + Sync + std::fmt::Debug {
    /// Sample kaydedildi.
    fn on_sample(&self, count: u32);
    /// Restart olayı kaydedildi (`reason` = `RestartReason::as_str()`).
    fn on_restart(&self, reason: &str);
}

/// Watchdog — periodik GDI sayacı + restart emitter.
pub struct Watchdog {
    config: WatchdogConfig,
    counter: GdiCounter,
    restart: RestartSignal,
    autosave: Arc<dyn DraftAutosave>,
    telemetry: Option<Arc<dyn TelemetrySink>>,
}

impl std::fmt::Debug for Watchdog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watchdog")
            .field("config", &self.config)
            .field("restart_subscribers", &self.restart.subscriber_count())
            .field("telemetry_attached", &self.telemetry.is_some())
            .finish_non_exhaustive()
    }
}

impl Watchdog {
    /// Yeni watchdog oluştur (telemetry olmadan — MVP-2 davranışı).
    #[must_use]
    pub fn new(
        config: WatchdogConfig,
        restart: RestartSignal,
        autosave: Arc<dyn DraftAutosave>,
    ) -> Self {
        Self {
            config,
            counter: GdiCounter::new(),
            restart,
            autosave,
            telemetry: None,
        }
    }

    /// Yeni watchdog oluştur ve opsiyonel telemetry sink bağla (MVP-3).
    ///
    /// `telemetry = Some(sink)` → her sample'da `sink.on_sample(count)` çağrılır.
    #[must_use]
    pub fn with_telemetry(
        config: WatchdogConfig,
        restart: RestartSignal,
        autosave: Arc<dyn DraftAutosave>,
        telemetry: Arc<dyn TelemetrySink>,
    ) -> Self {
        Self {
            config,
            counter: GdiCounter::new(),
            restart,
            autosave,
            telemetry: Some(telemetry),
        }
    }

    /// Background task olarak başlat — `tokio::spawn` ile çalıştırılmalı.
    ///
    /// Faz 1.0 stub: periyodik sample + log + restart emit. Gerçek WebView
    /// dispose+recreate Faz 1.6'da (`viscos-webview::WebViewWindow` üzerinden).
    ///
    /// İptal etmek için `tokio::task::JoinHandle::abort()` çağrılabilir.
    pub fn spawn(mut self) {
        let config = self.config.clone();
        tokio::spawn(async move {
            let mut ticker = interval(config.sample_interval);
            let mut warmup_remaining = config.warmup_samples;

            info!(
                warning = config.gdi_warning,
                critical = config.gdi_critical,
                interval_secs = config.sample_interval.as_secs(),
                telemetry_attached = self.telemetry.is_some(),
                "Watchdog started"
            );

            loop {
                ticker.tick().await;

                if warmup_remaining > 0 {
                    warmup_remaining -= 1;
                    debug!(remaining = warmup_remaining, "Watchdog warmup");
                    self.counter.reset();
                    continue;
                }

                let sample = self.counter.sample();
                let count = sample.count;
                let delta = sample.delta;

                // MVP-3: Telemetry hook varsa her sample'i kaydet.
                if let Some(hook) = self.telemetry.as_ref() {
                    hook.on_sample(count);
                }

                if count >= config.gdi_critical {
                    error!(count, delta, "GDI CRITICAL — restart tetikleniyor");

                    // Pre-restart: draft autosave (mesaj kaybı 0 garantisi).
                    let drafts = match self.autosave.snapshot_open_composers() {
                        Ok(n) => {
                            info!(drafts = n, "Draft autosave OK");
                            n
                        }
                        Err(e) => {
                            error!(?e, "Draft autosave başarısız");
                            0
                        }
                    };

                    let reason = RestartReason::GdiLeakCritical;
                    self.restart.emit(reason);
                    if let Some(hook) = self.telemetry.as_ref() {
                        hook.on_restart(reason.as_str());
                    }
                    info!(
                        drafts,
                        "Restart signal emitted — shell WebView'i yeniden oluşturmalı"
                    );
                    self.counter.reset();
                } else if count >= config.gdi_warning {
                    warn!(count, delta, "GDI WARNING");
                } else {
                    debug!(count, delta, "GDI OK");
                }
            }
        });
    }

    /// Watchdog konfigürasyonu (read-only).
    #[must_use]
    pub const fn config(&self) -> &WatchdogConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autosave::StubAutosave;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Test telemetry sink — sample/restart çağrılarını sayar.
    #[derive(Debug, Default)]
    struct CountingSink {
        sample_count: AtomicU32,
        restart_count: AtomicU32,
    }

    impl TelemetrySink for CountingSink {
        fn on_sample(&self, _count: u32) {
            self.sample_count.fetch_add(1, Ordering::SeqCst);
        }
        fn on_restart(&self, _reason: &str) {
            self.restart_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn default_config_matches_phase_1_0_thresholds() {
        let cfg = WatchdogConfig::default();
        assert_eq!(cfg.gdi_warning, 7000);
        assert_eq!(cfg.gdi_critical, 9000);
        assert_eq!(cfg.sample_interval, Duration::from_secs(30));
        assert_eq!(cfg.warmup_samples, 2);
    }

    #[test]
    fn watchdog_constructs_with_components() {
        let restart = RestartSignal::default();
        let autosave: Arc<dyn DraftAutosave> = Arc::new(StubAutosave::new());
        let wd = Watchdog::new(WatchdogConfig::default(), restart, autosave);
        assert_eq!(wd.config().gdi_critical, 9000);
        assert!(
            wd.telemetry.is_none(),
            "default constructor must not have telemetry"
        );
    }

    #[test]
    fn watchdog_with_telemetry_attaches_hook() {
        let restart = RestartSignal::default();
        let autosave: Arc<dyn DraftAutosave> = Arc::new(StubAutosave::new());
        let sink = Arc::new(CountingSink::default());
        let wd =
            Watchdog::with_telemetry(WatchdogConfig::default(), restart, autosave, sink.clone());
        assert!(wd.telemetry.is_some(), "telemetry must be attached");
    }

    #[tokio::test]
    async fn watchdog_constructs_and_subscribes_to_restart_signal() {
        let restart = RestartSignal::default();
        let mut rx = restart.subscribe();
        let autosave: Arc<dyn DraftAutosave> = Arc::new(StubAutosave::new());
        let wd = Watchdog::new(WatchdogConfig::default(), restart.clone(), autosave);

        // Doğrudan restart signal emit et (Watchdog task'i başlatmadan).
        restart.emit(RestartReason::GdiLeakCritical);

        // Subscriber mesajı aldı mı?
        match rx.try_recv() {
            Ok(RestartReason::GdiLeakCritical) => {}
            other => panic!("expected GdiLeakCritical, got {other:?}"),
        }

        // Watchdog instance oluşturuldu mu?
        assert_eq!(wd.config().gdi_critical, 9000);
    }

    #[tokio::test]
    async fn watchdog_restart_signal_emit_succeeds() {
        // Gerçek Watchdog::spawn() sonsuz loop → CI'da timeout olur.
        // Bu yüzden signal path'i izole test ediyoruz.
        let restart = RestartSignal::default();
        let mut rx = restart.subscribe();

        restart.emit(RestartReason::IpcBufferCritical);
        match rx.try_recv() {
            Ok(RestartReason::IpcBufferCritical) => {}
            other => panic!("expected IpcBufferCritical, got {other:?}"),
        }
    }

    #[test]
    fn counting_sink_records_calls() {
        let sink = CountingSink::default();
        sink.on_sample(100);
        sink.on_sample(200);
        sink.on_restart("GdiLeakCritical");
        assert_eq!(sink.sample_count.load(Ordering::SeqCst), 2);
        assert_eq!(sink.restart_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn restart_reason_as_str_is_stable() {
        // Restart reason string contract'ı public API'de belgelenmiştir;
        // telemetry crate bunun üzerinden restart reason loglar.
        assert_eq!(RestartReason::GdiLeakCritical.as_str(), "GdiLeakCritical");
        assert_eq!(
            RestartReason::IpcBufferCritical.as_str(),
            "IpcBufferCritical"
        );
    }
}
