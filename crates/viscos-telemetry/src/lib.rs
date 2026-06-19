//! `viscos-telemetry` — SQLite-backed time-series telemetry.
//!
//! MVP-3 (Faz 1.5 Polish) kapsamı:
//! - [`TelemetryStore`]: GDI sample + restart event logger.
//! - [`CefRecommendation`]: 7 günlük peak GDI değerine göre CEF backend önerisi.
//! - [`TelemetrySink`] trait: watchdog/observability callback kontratı.
//! - [`TelemetryStoreSink`]: `TelemetryStore`'u `TelemetrySink`'e adapte eder.
//!
//! Cross-references:
//! - [`viscos_watchdog`] — watchdog task `TelemetrySink::on_sample` callback'i.
//! - ADR-0012 §4 — telemetry-driven backend kararı.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub mod sink;
pub mod store;

pub use error::{Result, TelemetryError};
pub use sink::{TelemetrySink, TelemetryStoreSink};
pub use store::{CEF_REQUIRED_PEAK_THRESHOLD, CefRecommendation, TelemetryStore};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug, Default)]
    struct CountingSink {
        samples: AtomicU32,
        restarts: AtomicU32,
    }

    impl TelemetrySink for CountingSink {
        fn on_sample(&self, _count: u32) {
            self.samples.fetch_add(1, Ordering::SeqCst);
        }
        fn on_restart(&self, _reason: &str) {
            self.restarts.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn crate_version_exports_are_in_sync() {
        let _f: fn(&std::path::Path) -> Result<TelemetryStore> = TelemetryStore::open;
    }

    #[test]
    fn counting_sink_records_calls() {
        let sink = CountingSink::default();
        sink.on_sample(5000);
        sink.on_sample(7000);
        sink.on_restart("GdiLeakCritical");
        assert_eq!(sink.samples.load(Ordering::SeqCst), 2);
        assert_eq!(sink.restarts.load(Ordering::SeqCst), 1);
    }
}
