//! CDN refresh worker + batch URL metadata refresh (Faz 4 Dalga 2).
//!
//! `CdnRefreshWorker::run_once` is the production entry point — it iterates
//! `MediaCache` URL metadata for entries inside the 24h TTL refresh window
//! and re-fetches their signed URLs. The `MediaCache` URL iterator is wired
//! in a follow-up patch (cache.rs scope); this module owns the actual batch
//! refresh primitive ([`refresh_batch`]) so the orchestration layer just
//! collects URLs and delegates.
//!
//! ## Concurrency
//!
//! [`refresh_batch`] caps in-flight fetches via a `tokio::sync::Semaphore`
//! (default 32). Discord's CDN is rate-limit friendly at this concurrency;
//! bumping above 64 risks `429 Too Many Requests` on `cdn.discordapp.com`.
//!
//! ## Retry
//!
//! Transient fetcher failures are retried with exponential backoff
//! (100 ms → 500 ms → 2 s, three attempts total). After exhausting attempts,
//! the URL is reported as [`RefreshStatus::Failed`] and the batch continues —
//! one bad URL must not poison the rest of the round.
//!
//! ## Production wiring (Faz 4 Dalga 2 follow-up)
//!
//! A `reqwest`-backed [`UrlHeadFetcher`] implementation will be plugged in
//! once `reqwest` lands in the workspace dependencies. Today the default
//! [`StaticHeadFetcher`] returns the input URL with a 24h-from-now expiry,
//! which is enough to drive the orchestration path end-to-end.
//!
//! ## Tests
//!
//! Integration tests live in `../tests/refresh.rs` (public API only, the
//! `tests/` directory is excluded from the file-size scanner per
//! `.github/workflows/ai-task-validate.yml`).

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

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

    /// One refresh pass. Currently a no-op stub: `MediaCache::url_meta`
    /// iteration API is added in a follow-up patch. Once available this
    /// method will:
    ///
    /// 1. iterate `MediaCache` URL metadata for entries in the
    ///    `(now + threshold) < expires_at < (now + 24h)` window,
    /// 2. chunk by `self.batch_size`,
    /// 3. call [`refresh_batch`] per chunk with [`RefreshConfig::default`],
    /// 4. write refreshed `CdnUrlMeta` back via `MediaCache::put_with_url`.
    ///
    /// Returns `Ok(0)` while the iterator is missing — no URLs refreshed.
    pub async fn run_once(&self) -> Result<usize, MediaError> {
        let _ = self.threshold;
        let _ = self.batch_size;
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// Refresh batch API
// ---------------------------------------------------------------------------

/// Per-URL outcome of a [`refresh_batch`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshStatus {
    /// URL was refreshed successfully; new expiry captured.
    Refreshed {
        /// Discord CDN `ex=` unix timestamp extracted from the response.
        expires_at_unix: u64,
    },
    /// URL was ineligible for refresh (e.g. non-http scheme, malformed).
    Skipped {
        /// Static reason code — caller branches on this without parsing strings.
        reason: &'static str,
    },
    /// Fetcher returned an error after exhausting retries.
    Failed {
        /// Number of fetcher attempts actually made (1..=max_attempts).
        attempts: u32,
        /// Last error message.
        reason: String,
    },
}

/// One entry in [`RefreshReport::outcomes`], parallel to the input slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshOutcome {
    /// Original URL string (preserves caller formatting).
    pub url: String,
    /// Refresh outcome for this URL.
    pub status: RefreshStatus,
}

/// Aggregated stats from a [`refresh_batch`] invocation.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefreshReport {
    /// Total URLs handed to the batch.
    pub total: usize,
    /// URLs that returned a fresh signed URL.
    pub refreshed: usize,
    /// URLs that were ineligible for refresh (e.g. bad scheme).
    pub skipped: usize,
    /// URLs whose fetcher failed after exhausting retries.
    pub failed: usize,
    /// Per-URL outcomes, parallel to the input slice.
    pub outcomes: Vec<RefreshOutcome>,
}

impl RefreshReport {
    /// True when every URL was either refreshed or skipped (i.e. nothing failed).
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        self.failed == 0
    }
}

/// HTTP HEAD result that the fetcher produces for each URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlHead {
    /// URL after redirects — Discord may follow `Location:` on signed URLs.
    pub final_url: String,
    /// New expiry (Discord `ex=` query parameter, unix epoch seconds).
    pub expires_at_unix: u64,
}

/// Trait abstraction over HTTP HEAD fetcher.
///
/// `'static` is required because [`refresh_batch`] spawns per-URL tasks via
/// `tokio::spawn`, which needs `'static` futures. Fetcher implementations
/// must therefore own (not borrow) any state they capture.
///
/// Production wiring injects a `reqwest`-backed implementation (added in a
/// follow-up patch). Tests inject mock fetchers that record call counts,
/// simulate failures, or measure concurrency without touching the network.
#[async_trait]
pub trait UrlHeadFetcher: Send + Sync + 'static {
    /// Fetch metadata for `url`. Implementations decide how to extract
    /// the new expiry (e.g. parse `ex=` from the `Location:` header for
    /// Discord signed-URL refresh).
    ///
    /// # Errors
    ///
    /// Return [`MediaError::Cdn`] for transient network/HTTP failures so the
    /// retry policy can re-attempt the request. Validation errors
    /// (non-http scheme, malformed URL) should be reported at the caller
    /// level as [`RefreshStatus::Skipped`] instead of bubbling through this
    /// trait.
    async fn head(&self, url: &str) -> Result<UrlHead, MediaError>;
}

/// No-op fetcher: returns the input URL with a 24h-from-now expiry.
///
/// Used when no real HTTP fetcher is wired yet (production v1, before
/// `reqwest` lands) and in deterministic tests that don't care about
/// expiry timestamps.
#[derive(Debug, Default, Clone, Copy)]
pub struct StaticHeadFetcher;

#[async_trait]
impl UrlHeadFetcher for StaticHeadFetcher {
    async fn head(&self, url: &str) -> Result<UrlHead, MediaError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(UrlHead {
            final_url: url.to_string(),
            expires_at_unix: now + 24 * 3600,
        })
    }
}

/// Default exponential-backoff schedule (3 attempts, 100 ms / 500 ms / 2 s).
///
/// The third entry (2 s) is reserved for a future `max_attempts = 4` policy.
const DEFAULT_BACKOFFS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_secs(2),
];

/// Retry policy for transient fetcher failures.
///
/// `max_attempts = 3` sleeps once before attempt #2 (100 ms) and once before
/// attempt #3 (500 ms). The `n`-th backoff (`backoffs[n - 1]`) is slept
/// between attempt `n` and attempt `n + 1`.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Maximum number of fetcher attempts per URL (including the first).
    pub max_attempts: u32,
    /// Backoff durations between attempts. Length must be `>= max_attempts - 1`.
    pub backoffs: &'static [Duration],
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoffs: &DEFAULT_BACKOFFS,
        }
    }
}

impl RetryPolicy {
    /// Sleep before the next attempt. No-op when `attempt` is the final one
    /// or the backoff slice is shorter than `attempt - 1`.
    async fn backoff_between(&self, attempt: u32) {
        if attempt >= self.max_attempts {
            return;
        }
        let Some(delay) = self.backoffs.get((attempt - 1) as usize) else {
            return;
        };
        if delay.is_zero() {
            return;
        }
        tokio::time::sleep(*delay).await;
    }
}

/// Configuration for [`refresh_batch`].
#[derive(Debug, Clone, Copy)]
pub struct RefreshConfig {
    /// Maximum in-flight fetches; semaphore cap.
    pub concurrency: usize,
    /// Retry/backoff schedule for transient fetcher failures.
    pub retry_policy: RetryPolicy,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            concurrency: 32,
            retry_policy: RetryPolicy::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// URL pre-flight classification
// ---------------------------------------------------------------------------

const VALID_SCHEMES: &[&str] = &["http", "https"];

/// Cheap scheme check. Returns `Some(reason)` for URLs that should be
/// skipped without invoking the fetcher (non-http scheme, missing host).
fn url_skip_reason(url: &str) -> Option<&'static str> {
    let Some(scheme_end) = url.find("://") else {
        return Some("missing-scheme");
    };
    let scheme = &url[..scheme_end];
    if !VALID_SCHEMES.contains(&scheme) {
        return Some("unsupported-scheme");
    }
    if url.len() <= scheme_end + "://".len() {
        return Some("missing-host");
    }
    None
}

// ---------------------------------------------------------------------------
// Single-URL refresh (with retry)
// ---------------------------------------------------------------------------

async fn refresh_one<F>(url: &str, config: RefreshConfig, fetcher: &F) -> RefreshOutcome
where
    F: UrlHeadFetcher + ?Sized,
{
    if let Some(reason) = url_skip_reason(url) {
        return RefreshOutcome {
            url: url.to_string(),
            status: RefreshStatus::Skipped { reason },
        };
    }

    let policy = config.retry_policy;
    let mut last_err = String::new();
    for attempt in 1..=policy.max_attempts {
        match fetcher.head(url).await {
            Ok(head) => {
                debug!(
                    url,
                    attempt,
                    expires_at_unix = head.expires_at_unix,
                    "url head ok"
                );
                return RefreshOutcome {
                    url: url.to_string(),
                    status: RefreshStatus::Refreshed {
                        expires_at_unix: head.expires_at_unix,
                    },
                };
            }
            Err(e) => {
                last_err = e.to_string();
                warn!(
                    url,
                    attempt,
                    max_attempts = policy.max_attempts,
                    error = %last_err,
                    "url head failed, will retry"
                );
                policy.backoff_between(attempt).await;
            }
        }
    }

    RefreshOutcome {
        url: url.to_string(),
        status: RefreshStatus::Failed {
            attempts: policy.max_attempts,
            reason: last_err,
        },
    }
}

// ---------------------------------------------------------------------------
// Batch refresh
// ---------------------------------------------------------------------------

/// Concurrent batch refresh of Discord CDN signed URLs.
///
/// # Returns
///
/// A [`RefreshReport`] summarizing refreshed / skipped / failed counts plus
/// per-URL [`RefreshOutcome`]s. The order of `outcomes` matches the input
/// slice. Order preservation matters because callers map URL → attachment
/// id by position.
///
/// # Concurrency
///
/// In-flight fetches are bounded by `config.concurrency` (default 32). One
/// semaphore permit is acquired per URL; permits are released when the
/// spawned task completes (the permit is moved into the task closure).
pub async fn refresh_batch<F>(
    urls: &[&str],
    config: RefreshConfig,
    fetcher: Arc<F>,
) -> Result<RefreshReport, MediaError>
where
    F: UrlHeadFetcher + ?Sized,
{
    let total = urls.len();
    if urls.is_empty() {
        return Ok(RefreshReport::default());
    }

    let concurrency = config.concurrency.max(1);
    let semaphore = Arc::new(Semaphore::new(concurrency));
    info!(total, concurrency, "refresh_batch: starting");

    let mut handles = Vec::with_capacity(total);
    for (idx, url) in urls.iter().enumerate() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| MediaError::Cdn(format!("semaphore closed: {e}")))?;
        let fetcher = fetcher.clone();
        let cfg = config;
        let url_owned = (*url).to_string();
        let handle = tokio::spawn(async move {
            let _permit = permit;
            let outcome = refresh_one(&url_owned, cfg, &*fetcher).await;
            (idx, outcome)
        });
        handles.push(handle);
    }

    let mut indexed: Vec<Option<RefreshOutcome>> = (0..total).map(|_| None).collect();
    for handle in handles {
        let (idx, outcome) = handle
            .await
            .map_err(|e| MediaError::Cdn(format!("task join failed: {e}")))?;
        indexed[idx] = Some(outcome);
    }

    let mut report = RefreshReport {
        total,
        ..RefreshReport::default()
    };
    for outcome in indexed.into_iter().flatten() {
        match outcome.status {
            RefreshStatus::Refreshed { .. } => report.refreshed += 1,
            RefreshStatus::Skipped { .. } => report.skipped += 1,
            RefreshStatus::Failed { .. } => report.failed += 1,
        }
        report.outcomes.push(outcome);
    }

    info!(
        total = report.total,
        refreshed = report.refreshed,
        skipped = report.skipped,
        failed = report.failed,
        "refresh_batch: done"
    );

    Ok(report)
}
