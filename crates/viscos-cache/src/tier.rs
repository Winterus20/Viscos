//! Cache tier boyut yapılandırması + adaptive sizing stub (ADR-0010 §4.5).
//!
//! Faz 4.0 Dalga 1: **statik default'lar** (v1) — `CacheTiers::default_v1()`
//! üretir. Faz 1.5 telemetry backend hazır olunca `auto_tune()` adaptive
//! algorithm aktif olur (Dalga 3 — parent agent sonra atayacak).
//!
//! ## Tier sizing rationale (v1 defaults)
//!
//! - **moka RAM metadata**: 64 MB — sıcak mesaj + üye lookup (Discord OLTP pattern).
//! - **foyer memory tier**: 32 MB — sıcak blob chunk.
//! - **foyer disk tier**: 10 GB — tüm attachment blob (encrypted).
//!
//! Toplam ~101 MB cache overhead. Viscos shell + WebView + IPC ~150 MB →
//! toplam ~250 MB baseline, 300 MB hedefe sığar.

use serde::{Deserialize, Serialize};

/// Tier sizes in bytes. `Memory = moka RAM metadata`, `Disk = foyer hybrid`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CacheTiers {
    /// moka RAM tier (bytes).
    pub memory: u64,
    /// foyer disk tier (bytes).
    pub disk: u64,
}

impl CacheTiers {
    /// v1 default tier sizes (ADR-0010 §B tablosu).
    pub fn default_v1() -> Self {
        Self {
            memory: 64 * MB,
            disk: 10 * GB,
        }
    }

    /// Adaptive sizing stub — Dalga 3 (Faz 1.5 telemetry) implementasyonu.
    ///
    /// v1'de no-op (default'ı değiştirmez). Telemetry backend hazır olunca:
    /// 1. Son 1 saatlik hit ratio aggregate'i alınır.
    /// 2. moka hit ratio <%70 → memory 2× (max 256 MB); >%95 → ÷2 (min 32 MB).
    /// 3. foyer disk hit ratio <%40 → disk 2× (cap kullanıcı onayı gerektirir).
    ///
    /// **İnsan onayı:** Tier değişikliği >25 GB disk artışı için kullanıcı tray
    /// notification gerekir (ADR-0010 §4.5 Tier policy tablosu).
    pub fn auto_tune(&mut self, _telemetry: &TelemetryStats) {
        // TODO Dalga 3: hit ratio thresholds'a göre tier boyut tune.
        // Bugün: statik default'lar korunuyor (auto_tune opt-out default true v1'de).
        // Implementation note: parent agent sonra atayacak (Faz 1.5 telemetry entegrasyonu).
    }
}

/// Placeholder for telemetry aggregate (Dalga 3). Faz 1.5 telemetry backend
/// hazır olunca gerçek hit ratio + eviction count ile doldurulur.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryStats {
    /// Aggregation window unix timestamp (start).
    pub window_start: u64,
    /// Window length in seconds (default 3600 = 1 saat).
    pub window_secs: u32,

    /// moka hits in window.
    pub moka_hits: u64,
    /// moka misses in window.
    pub moka_misses: u64,
    /// moka evictions in window.
    pub moka_evictions: u64,

    /// foyer memory tier hits.
    pub foyer_memory_hits: u64,
    /// foyer memory tier misses.
    pub foyer_memory_misses: u64,

    /// foyer disk tier hits.
    pub foyer_disk_hits: u64,
    /// foyer disk tier misses.
    pub foyer_disk_misses: u64,
}

impl TelemetryStats {
    /// moka hit ratio (`hits / (hits + misses)`). 0.0 → 1.0.
    pub fn moka_hit_ratio(&self) -> f64 {
        let total = self.moka_hits + self.moka_misses;
        if total == 0 {
            0.0
        } else {
            self.moka_hits as f64 / total as f64
        }
    }

    /// foyer disk tier hit ratio.
    pub fn foyer_disk_hit_ratio(&self) -> f64 {
        let total = self.foyer_disk_hits + self.foyer_disk_misses;
        if total == 0 {
            0.0
        } else {
            self.foyer_disk_hits as f64 / total as f64
        }
    }
}

/// Size constant helpers (Bytes-based, byte unit).
const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_v1_matches_adr_0010_table() {
        let tiers = CacheTiers::default_v1();
        assert_eq!(tiers.memory, 64 * MB);
        assert_eq!(tiers.disk, 10 * GB);
    }

    #[test]
    fn auto_tune_is_noop_in_v1() {
        let mut tiers = CacheTiers::default_v1();
        let original_mem = tiers.memory;
        let original_disk = tiers.disk;
        let stats = TelemetryStats::default();
        tiers.auto_tune(&stats);
        assert_eq!(tiers.memory, original_mem);
        assert_eq!(tiers.disk, original_disk);
    }

    #[test]
    fn telemetry_hit_ratio_computes_correctly() {
        let stats = TelemetryStats {
            moka_hits: 70,
            moka_misses: 30,
            ..Default::default()
        };
        assert!((stats.moka_hit_ratio() - 0.7).abs() < 1e-9);

        let empty = TelemetryStats::default();
        assert_eq!(empty.moka_hit_ratio(), 0.0);
    }
}
