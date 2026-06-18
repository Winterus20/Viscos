//! CDN refresh worker — Dalga 2 stub.
//!
//! Tracks Discord signed-URL expiry and refreshes URLs approaching their
//! 24-hour TTL limit. Full implementation is deferred: this module exposes
//! only the configuration knobs and a `run_once` no-op.

use std::time::Duration;

use crate::cache::MediaError;

/// Configuration for the CDN refresh worker.
pub struct CdnRefreshWorker {
    /// Refresh URLs whose expiry lands in `(now + threshold, now + 24h)`.
    pub threshold: Duration,
    /// Max attachments per refresh batch (Discord rate-limit friendly).
    pub batch_size: usize,
}

impl CdnRefreshWorker {
    /// v1 defaults: refresh in the last hour of the 24h TTL window,
    /// batch 50 attachments per round-trip.
    pub fn default_v1() -> Self {
        Self {
            threshold: Duration::from_secs(23 * 3600),
            batch_size: 50,
        }
    }

    /// One refresh pass. Dalga 2 will iterate `MediaCache` URL metadata,
    /// filter by `(now + threshold) < expires_at < (now + 24h)`, batch,
    /// refetch signed URLs, and update metadata.
    ///
    /// v1 stub: returns `Ok(0)` (nothing refreshed).
    pub async fn run_once(&self) -> Result<usize, MediaError> {
        // TODO(Dalga 2): iterate MediaCache URL metadata, batch refresh.
        Ok(0)
    }
}
