//! `TelemetrySink` trait + `TelemetryStoreSink` adapter.
//!
//! MVP-3 telemetry sink kontratı.
//!
//! `viscos-telemetry` crate'i `viscos-watchdog`'a bağımlılık yaratmaz
//! (dependency cycle riski). Bunun yerine generic bir `TelemetrySink`
//! trait'i tanımlar; watchdog kendi `TelemetrySink` trait'ini tutar
//! (aynı isim, aynı imza, ayrı crate) ve `viscos` binary main'inde
//! iki trait'i `TelemetrySinkAdapter` ile bağlarız.

use std::sync::Arc;

use crate::TelemetryStore;

/// MVP-3 telemetry sink kontratı.
///
/// `viscos-telemetry` crate'i `viscos-watchdog`'a bağımlılık yaratmaz
/// (dependency cycle riski). Bunun yerine generic bir `TelemetrySink`
/// trait'i tanımlar; watchdog `Arc<dyn TelemetrySink>` olarak kabul eder.
///
/// `RestartReason` enum'u `viscos-watchdog`'da olduğundan, sink `&str`
/// alır — watchdog kendi enum'unu string'e map eder. Bu sayede telemetry
/// crate bağımsız test edilebilir.
pub trait TelemetrySink: Send + Sync + std::fmt::Debug {
    /// GDI örneği kaydedildi.
    fn on_sample(&self, count: u32);
    /// Restart olayı kaydedildi (`reason` watchdog restart_reason
    /// variant'ının Display impl'ından gelir).
    fn on_restart(&self, reason: &str);
}

/// `TelemetryStore`'u `TelemetrySink`'e adapte eden wrapper.
///
/// `viscos` binary main'inde `Watchdog::with_telemetry(store.sink())`
/// çağrılır. Her sample SQLite'a yazılır; hata loglanır ama panic
/// edilmez (push exception: telemetry sample kaybı tolere edilir).
pub struct TelemetryStoreSink {
    store: Arc<TelemetryStore>,
}

impl std::fmt::Debug for TelemetryStoreSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryStoreSink").finish_non_exhaustive()
    }
}

impl TelemetryStoreSink {
    /// Yeni sink oluştur (store Arc capture).
    #[must_use]
    pub fn new(store: Arc<TelemetryStore>) -> Self {
        Self { store }
    }
}

impl TelemetrySink for TelemetryStoreSink {
    fn on_sample(&self, count: u32) {
        if let Err(e) = self.store.record_gdi_sample(count) {
            tracing::warn!(error = %e, count, "TelemetryStoreSink: record_gdi_sample failed");
        }
    }

    fn on_restart(&self, reason: &str) {
        if let Err(e) = self.store.record_restart(reason) {
            tracing::warn!(error = %e, reason, "TelemetryStoreSink: record_restart failed");
        }
    }
}
