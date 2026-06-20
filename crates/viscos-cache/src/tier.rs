//! Cache tier boyut yapılandırması + adaptive sizing (ADR-0010 §B "Adaptive
//! Tier Sizing").
//!
//! Faz 4.0 Dalga 1: **statik default'lar** (v1) — `CacheTiers::default_v1()`
//! üretir. Faz 1.5 telemetry backend hazır olunca `auto_tune()` adaptive
//! algorithm aktif olur (Dalga 3).
//!
//! ## Tier sizing rationale (v1 defaults)
//!
//! - **moka RAM metadata**: 64 MB — sıcak mesaj + üye lookup (Discord OLTP pattern).
//! - **foyer memory tier**: 32 MB — sıcak blob chunk.
//! - **foyer disk tier**: 10 GB — tüm attachment blob (encrypted).
//!
//! Toplam ~101 MB cache overhead. Viscos shell + WebView + IPC ~150 MB →
//! toplam ~250 MB baseline, 300 MB hedefe sığar.
//!
//! ## Adaptive algorithm (Dalga 3)
//!
//! Hit ratio thresholds (telemetry aggregate from `TelemetryStats`):
//!
//! | Hit ratio (volume-weighted) | Action |
//! |---|---|
//! | `>= 0.85` | memory × 2 (cap 256 MB), disk + 50 % (cap 25 GB) |
//! | `0.5 .. 0.85` | no-op (healthy band) |
//! | `< 0.5` | memory ÷ 2 (floor 32 MB); disk unchanged |
//!
//! **Safety bounds** (ADR-0010 §B Tier policy):
//! - Memory tier hard cap at 256 MB (`u64::saturating_mul`) — RAM geri kazanımı.
//! - Disk tier hard cap at 25 GB — `>25 GB` requires tray notification per
//!   §4.5 Tier policy; v1 stays below the threshold and never requests approval.
//! - Empty telemetry (cold start / watchdog disabled) → no-op: better to
//!   keep defaults than to shrink on a spurious 0 % hit ratio.
//!
//! **İnsan onayı:** Disk tier büyümesi >25 GB kullanıcı tray notification
//! gerektirir (ADR-0010 §4.5). Bu v1 implementation cap ile sınırlı; Faz 4
//! Dalga 2'de tray integration eklenecek.

use serde::{Deserialize, Serialize};
use tracing::info;

/// Hit ratio above which tiers are expanded. Below this but above
/// [`LOW_HIT_RATIO_THRESHOLD`] is the no-op healthy band.
const HIGH_HIT_RATIO_THRESHOLD: f64 = 0.85;

/// Hit ratio below which the hot (memory) tier is shrunk. Disk is left alone
/// because cold attachments still need storage regardless of churn.
const LOW_HIT_RATIO_THRESHOLD: f64 = 0.5;

/// Hard cap on the moka RAM tier (ADR-0010 §B Tier policy).
const MAX_MEMORY_TIER: u64 = 256 * MB;

/// Floor for the moka RAM tier — smaller values produce no observable gain
/// because of moka's own overhead per entry.
const MIN_MEMORY_TIER: u64 = 32 * MB;

/// Hard cap on the foyer disk tier. Beyond 25 GB a tray notification is
/// required (ADR-0010 §4.5); v1 stays below the threshold.
const MAX_DISK_TIER: u64 = 25 * GB;

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

    /// Adaptive sizing — hit ratio thresholds → tier size tune.
    ///
    /// Volume-weighted hit ratio is computed across the two observable
    /// telemetry sources (moka RAM + foyer disk). The combined ratio drives
    /// both tiers because cache effectiveness is a global property: if the
    /// working set fits the cache, both layers benefit; if not, the hot tier
    /// needs more headroom while cold storage is left to age out.
    ///
    /// Empty telemetry bails out (no signal) rather than acting on a
    /// spurious 0.0 hit ratio from a cold start.
    pub fn auto_tune(&mut self, telemetry: &TelemetryStats) {
        let moka_total = telemetry.moka_hits + telemetry.moka_misses;
        let foyer_total = telemetry.foyer_disk_hits + telemetry.foyer_disk_misses;
        let total_samples = moka_total + foyer_total;
        if total_samples == 0 {
            return;
        }

        // Volume-weighted hit ratio. When only one tier has data we use that
        // tier's ratio directly — avoids zero-weighting in the average.
        let hit_ratio = if moka_total > 0 && foyer_total > 0 {
            let moka_ratio = telemetry.moka_hits as f64 / moka_total as f64;
            let foyer_ratio = telemetry.foyer_disk_hits as f64 / foyer_total as f64;
            (moka_ratio * moka_total as f64 + foyer_ratio * foyer_total as f64)
                / total_samples as f64
        } else if moka_total > 0 {
            telemetry.moka_hits as f64 / moka_total as f64
        } else {
            telemetry.foyer_disk_hits as f64 / foyer_total as f64
        };

        if hit_ratio >= HIGH_HIT_RATIO_THRESHOLD {
            let new_memory = self.memory.saturating_mul(2).min(MAX_MEMORY_TIER);
            let new_disk = self.disk.saturating_add(self.disk / 2).min(MAX_DISK_TIER);
            info!(
                hit_ratio,
                memory_before = self.memory,
                memory_after = new_memory,
                disk_before = self.disk,
                disk_after = new_disk,
                "cache tier auto_tune: expanding tiers on high hit ratio",
            );
            self.memory = new_memory;
            self.disk = new_disk;
        } else if hit_ratio < LOW_HIT_RATIO_THRESHOLD {
            let new_memory = (self.memory / 2).max(MIN_MEMORY_TIER);
            info!(
                hit_ratio,
                memory_before = self.memory,
                memory_after = new_memory,
                disk_before = self.disk,
                "cache tier auto_tune: shrinking hot tier on low hit ratio",
            );
            self.memory = new_memory;
        }
        // 0.5 <= hit_ratio < 0.85: healthy band, no-op.
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
    fn auto_tune_no_signal_keeps_defaults() {
        // Empty telemetry → no signal (cold start / watchdog disabled).
        // Better to keep defaults than to shrink on a spurious 0 % hit ratio.
        let mut tiers = CacheTiers::default_v1();
        let original_mem = tiers.memory;
        let original_disk = tiers.disk;
        let stats = TelemetryStats::default();
        tiers.auto_tune(&stats);
        assert_eq!(tiers.memory, original_mem);
        assert_eq!(tiers.disk, original_disk);
    }

    #[test]
    fn auto_tune_increases_tiers_on_high_hit_ratio() {
        // 90 % moka + 85 % foyer disk → combined ≥ 0.85 → expand both tiers.
        let mut tiers = CacheTiers {
            memory: 64 * MB,
            disk: 10 * GB,
        };
        let stats = TelemetryStats {
            moka_hits: 90,
            moka_misses: 10,
            foyer_disk_hits: 85,
            foyer_disk_misses: 15,
            ..Default::default()
        };
        tiers.auto_tune(&stats);
        assert_eq!(tiers.memory, 128 * MB);
        assert_eq!(tiers.disk, 15 * GB);
    }

    #[test]
    fn auto_tune_decreases_hot_tier_on_low_hit_ratio() {
        // 10 % moka + 5 % foyer disk → combined < 0.5 → shrink memory, keep disk.
        let mut tiers = CacheTiers {
            memory: 128 * MB,
            disk: 10 * GB,
        };
        let stats = TelemetryStats {
            moka_hits: 10,
            moka_misses: 90,
            foyer_disk_hits: 5,
            foyer_disk_misses: 95,
            ..Default::default()
        };
        tiers.auto_tune(&stats);
        assert_eq!(tiers.memory, 64 * MB);
        assert_eq!(tiers.disk, 10 * GB);
    }

    #[test]
    fn auto_tune_no_op_on_moderate_hit_ratio() {
        // 70 % moka + 60 % foyer disk → combined ≈ 0.66 → no-op.
        let mut tiers = CacheTiers {
            memory: 64 * MB,
            disk: 10 * GB,
        };
        let stats = TelemetryStats {
            moka_hits: 70,
            moka_misses: 30,
            foyer_disk_hits: 60,
            foyer_disk_misses: 40,
            ..Default::default()
        };
        tiers.auto_tune(&stats);
        assert_eq!(tiers.memory, 64 * MB);
        assert_eq!(tiers.disk, 10 * GB);
    }

    #[test]
    fn auto_tune_respects_safety_bounds_on_expand() {
        // Memory at the 256 MB cap should not double; disk at 25 GB cap should
        // not grow further (ADR-0010 §4.5 tray notification policy).
        let mut tiers = CacheTiers {
            memory: MAX_MEMORY_TIER,
            disk: MAX_DISK_TIER,
        };
        let stats = TelemetryStats {
            moka_hits: 95,
            moka_misses: 5,
            foyer_disk_hits: 90,
            foyer_disk_misses: 10,
            ..Default::default()
        };
        tiers.auto_tune(&stats);
        assert_eq!(tiers.memory, MAX_MEMORY_TIER);
        assert_eq!(tiers.disk, MAX_DISK_TIER);
    }

    #[test]
    fn auto_tune_respects_safety_floor_on_shrink() {
        // Memory already at the 32 MB floor should not shrink below it.
        let mut tiers = CacheTiers {
            memory: MIN_MEMORY_TIER,
            disk: 10 * GB,
        };
        let stats = TelemetryStats {
            moka_hits: 5,
            moka_misses: 95,
            foyer_disk_hits: 0,
            foyer_disk_misses: 100,
            ..Default::default()
        };
        tiers.auto_tune(&stats);
        assert_eq!(tiers.memory, MIN_MEMORY_TIER);
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
