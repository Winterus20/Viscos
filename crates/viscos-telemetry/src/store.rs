//! `TelemetryStore` — SQLite-backed GDI time-series + restart event log.
//!
//! MVP-3 (Polish) Faz 1.5 telemetry'sinin basit hali:
//!
//! - **`gdi_samples` tablosu:** Periyodik GDI object count örnekleri
//!   (`ts` Unix epoch seconds, `count` GDI object number). Watchdog her
//!   sample aldığında `record_gdi_sample` çağırır.
//! - **`restart_events` tablosu:** WebView soft-restart olayları
//!   (`ts`, `reason`). Faz 1.6 telemetry analyser için zemin.
//! - **`recommend_cef()`:** Son 7 günlük peak GDI değerine bakıp
//!   CEF backend önerisi döner (`Required` / `Optional` / `Unknown`).
//!
//! ## Schema migration
//!
//! MVP-3'te `CREATE TABLE IF NOT EXISTS` ile idempotent migration kullanılır
//! (Refinery Faz 1.5'te). İlk açılışta tablolar yaratılır; sonraki açılışlarda
//! `IF NOT EXISTS` no-op.
//!
//! ## Threading
//!
//! `rusqlite::Connection` `!Sync` (interior `RefCell`); MVP-3'te telemetry
//! store `parking_lot::Mutex<Connection>` ile sarılır, `Arc<TelemetryStore>`
//! `Send + Sync` olur. Watchdog task + main thread aynı store'a erişir.
//!
//! Cross-references:
//! - [`viscos_watchdog::Watchdog`] — sample callback.
//! - ADR-0012 §4 — CEF default rollout telemetry-driven kararı.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::Connection;
use tracing::{info, warn};

use crate::error::{Result, TelemetryError};

/// Default schema migration (idempotent).
///
/// MVP-3: raw `CREATE TABLE IF NOT EXISTS` ile basit bootstrap. Faz 1.5'te
/// `refinery::embed_migrations!` ile versiyonlu migration'a migrate edilecek.
const SCHEMA_SQL: &str = r"
CREATE TABLE IF NOT EXISTS gdi_samples (
    ts INTEGER NOT NULL,
    count INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS gdi_samples_ts_idx ON gdi_samples(ts);
CREATE TABLE IF NOT EXISTS restart_events (
    ts INTEGER NOT NULL,
    reason TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS restart_events_ts_idx ON restart_events(ts);
";

/// CEF backend önerisi (telemetry-driven karar, ADR-0012 §4).
///
/// MVP-3'te sadece tip + determination logic var; gerçek rollout Faz 1.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefRecommendation {
    /// Veri yok (cold start veya telemetry henüz toplanmamış).
    Unknown,
    /// GDI sayısı stabil; WebView2 yeterli.
    Optional,
    /// GDI leak tepe değeri kritik; CEF zorunlu (Faz 1.6 rollout).
    Required,
}

impl CefRecommendation {
    /// İnsan-okunabilir etiket.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Optional => "optional",
            Self::Required => "required",
        }
    }
}

/// 7 günlük GDI peak değerine göre `Required` kararı için eşik.
pub const CEF_REQUIRED_PEAK_THRESHOLD: u32 = 8500;

/// Telemetry store — SQLite-backed GDI time-series.
pub struct TelemetryStore {
    db: Mutex<Connection>,
}

impl std::fmt::Debug for TelemetryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryStore").finish_non_exhaustive()
    }
}

impl TelemetryStore {
    /// Yeni telemetry store aç. Parent dizin `create_dir_all` ile garanti
    /// edilir (SQLITE_CANTOPEN errno 14'ü önler).
    ///
    /// # Errors
    ///
    /// [`TelemetryError::Io`] — parent dizin oluşturulamadı.
    /// [`TelemetryError::Sqlite`] — DB açılamadı veya schema migration başarısız.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA_SQL)?;
        info!(path = %path.display(), "telemetry store opened");
        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    /// In-memory telemetry store (test'ler için).
    ///
    /// # Errors
    ///
    /// [`TelemetryError::Sqlite`] — `Connection::open_in_memory` başarısız.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    /// Tek bir GDI sample kaydet.
    ///
    /// # Errors
    ///
    /// [`TelemetryError::Sqlite`] — INSERT başarısız.
    pub fn record_gdi_sample(&self, count: u32) -> Result<()> {
        let now = current_unix_seconds();
        self.db.lock().execute(
            "INSERT INTO gdi_samples (ts, count) VALUES (?1, ?2)",
            rusqlite::params![now, count],
        )?;
        Ok(())
    }

    /// Restart olayı kaydet (reason serbest metin).
    ///
    /// # Errors
    ///
    /// [`TelemetryError::Sqlite`] — INSERT başarısız.
    pub fn record_restart(&self, reason: &str) -> Result<()> {
        let now = current_unix_seconds();
        self.db.lock().execute(
            "INSERT INTO restart_events (ts, reason) VALUES (?1, ?2)",
            rusqlite::params![now, reason],
        )?;
        Ok(())
    }

    /// Son `lookback_secs` saniye içindeki peak GDI değerini oku.
    ///
    /// # Errors
    ///
    /// [`TelemetryError::Sqlite`] — SELECT başarısız.
    pub fn peak_gdi_last(&self, lookback_secs: i64) -> Result<Option<u32>> {
        let cutoff = current_unix_seconds() - lookback_secs;
        let conn = self.db.lock();
        let mut stmt = conn.prepare("SELECT MAX(count) FROM gdi_samples WHERE ts >= ?1")?;
        let result: Option<u64> = stmt.query_row(rusqlite::params![cutoff], |row| {
            row.get::<_, Option<u64>>(0)
        })?;
        Ok(result.and_then(|v| u32::try_from(v).ok()))
    }

    /// CEF backend önerisi (`Unknown` | `Optional` | `Required`).
    ///
    /// 7 günlük pencere içindeki peak GDI sayısına bakar:
    /// - `>= CEF_REQUIRED_PEAK_THRESHOLD` → `Required`.
    /// - Veri var ama threshold altında → `Optional`.
    /// - Veri yok → `Unknown`.
    pub fn recommend_cef(&self) -> CefRecommendation {
        let seven_days = 7 * 24 * 60 * 60;
        match self.peak_gdi_last(seven_days) {
            Ok(Some(peak)) if peak >= CEF_REQUIRED_PEAK_THRESHOLD => CefRecommendation::Required,
            Ok(Some(_)) => CefRecommendation::Optional,
            Ok(None) => CefRecommendation::Unknown,
            Err(e) => {
                warn!(error = %e, "recommend_cef: query failed; returning Unknown");
                CefRecommendation::Unknown
            }
        }
    }

    /// Tablodaki toplam sample sayısı (test + smoke için).
    ///
    /// # Errors
    ///
    /// [`TelemetryError::Sqlite`] — SELECT başarısız.
    pub fn gdi_sample_count(&self) -> Result<u64> {
        let count: i64 =
            self.db
                .lock()
                .query_row("SELECT COUNT(*) FROM gdi_samples", [], |row| row.get(0))?;
        u64::try_from(count).map_err(|e| {
            TelemetryError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::other(format!("count overflow: {e}")),
            )))
        })
    }

    /// Tablodaki toplam restart event sayısı (test + smoke için).
    ///
    /// # Errors
    ///
    /// [`TelemetryError::Sqlite`] — SELECT başarısız.
    pub fn restart_event_count(&self) -> Result<u64> {
        let count: i64 =
            self.db
                .lock()
                .query_row("SELECT COUNT(*) FROM restart_events", [], |row| row.get(0))?;
        u64::try_from(count).map_err(|e| {
            TelemetryError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::other(format!("count overflow: {e}")),
            )))
        })
    }

    /// Altta yatan SQLite connection'a raw erişim (test için).
    #[cfg(test)]
    pub(crate) fn raw_db(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.db.lock()
    }

    /// Watchdog telemetry sink adapter'ı oluştur.
    ///
    /// Bu method `Arc<TelemetryStore>` üzerinden `Arc<TelemetryStoreSink>`
    /// üretir; main.rs'de `Arc<dyn viscos_watchdog::TelemetrySink>`'e
    /// adapt edilir.
    pub fn sink(self: &Arc<Self>) -> Arc<crate::sink::TelemetryStoreSink> {
        Arc::new(crate::sink::TelemetryStoreSink::new(Arc::clone(self)))
    }
}

/// `std::time::SystemTime` epoch seconds (cross-platform).
fn current_unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn open_creates_parent_directory() {
        let dir = TempDir::new().expect("tempdir");
        let nested = dir.path().join("a/b/c/telemetry.db");
        let store = TelemetryStore::open(&nested).expect("open must create parent");
        assert!(nested.exists(), "db file must be created");
        assert_eq!(store.gdi_sample_count().unwrap(), 0);
    }

    #[test]
    fn record_gdi_sample_increments_count() {
        let store = TelemetryStore::open_in_memory().expect("open");
        assert_eq!(store.gdi_sample_count().unwrap(), 0);
        store.record_gdi_sample(5000).expect("record");
        store.record_gdi_sample(7000).expect("record");
        store.record_gdi_sample(8500).expect("record");
        assert_eq!(store.gdi_sample_count().unwrap(), 3);
    }

    #[test]
    fn peak_gdi_last_returns_max() {
        let store = TelemetryStore::open_in_memory().expect("open");
        store.record_gdi_sample(3000).expect("record");
        store.record_gdi_sample(8500).expect("record");
        store.record_gdi_sample(6500).expect("record");
        // Tüm sample'lar "şimdi" timestamp'i ile yazildi → 24h pencere peak 8500.
        let peak = store.peak_gdi_last(24 * 60 * 60).expect("peak");
        assert_eq!(peak, Some(8500));
    }

    #[test]
    fn peak_gdi_last_returns_none_for_empty() {
        let store = TelemetryStore::open_in_memory().expect("open");
        let peak = store.peak_gdi_last(24 * 60 * 60).expect("peak");
        assert_eq!(peak, None);
    }

    #[test]
    fn record_restart_increments_count() {
        let store = TelemetryStore::open_in_memory().expect("open");
        store.record_restart("GdiLeakCritical").expect("record");
        store.record_restart("IpcBufferCritical").expect("record");
        assert_eq!(store.restart_event_count().unwrap(), 2);
    }

    #[test]
    fn recommend_cef_returns_unknown_for_empty() {
        let store = TelemetryStore::open_in_memory().expect("open");
        assert_eq!(store.recommend_cef(), CefRecommendation::Unknown);
    }

    #[test]
    fn recommend_cef_returns_optional_for_low_peak() {
        let store = TelemetryStore::open_in_memory().expect("open");
        store.record_gdi_sample(5000).expect("record");
        assert_eq!(store.recommend_cef(), CefRecommendation::Optional);
    }

    #[test]
    fn recommend_cef_returns_required_for_high_peak() {
        let store = TelemetryStore::open_in_memory().expect("open");
        store
            .record_gdi_sample(CEF_REQUIRED_PEAK_THRESHOLD)
            .expect("record");
        assert_eq!(store.recommend_cef(), CefRecommendation::Required);
    }

    #[test]
    fn recommend_cef_returns_required_above_threshold() {
        let store = TelemetryStore::open_in_memory().expect("open");
        store.record_gdi_sample(9500).expect("record");
        assert_eq!(store.recommend_cef(), CefRecommendation::Required);
    }

    #[test]
    fn cef_recommendation_as_str_matches_enum_variant() {
        assert_eq!(CefRecommendation::Unknown.as_str(), "unknown");
        assert_eq!(CefRecommendation::Optional.as_str(), "optional");
        assert_eq!(CefRecommendation::Required.as_str(), "required");
    }

    #[test]
    fn schema_is_idempotent() {
        // İki kez schema uygulamak hata vermemeli.
        let store = TelemetryStore::open_in_memory().expect("open");
        store
            .raw_db()
            .execute_batch(SCHEMA_SQL)
            .expect("idempotent schema");
    }
}
