//! Integration tests for `refresh_batch` — public API only (the `tests/`
//! directory is excluded from the file-size scanner per
//! `.github/workflows/ai-task-validate.yml`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use viscos_media::refresh::{
    RefreshConfig, RefreshStatus, RetryPolicy, StaticHeadFetcher, UrlHead, UrlHeadFetcher,
    refresh_batch,
};

// ----- mock fetchers -----

/// Fetcher that always succeeds with a fixed expiry. Used by the URL
/// classification tests.
struct OkFetcher;

#[async_trait]
impl UrlHeadFetcher for OkFetcher {
    async fn head(&self, url: &str) -> Result<UrlHead, viscos_media::MediaError> {
        Ok(UrlHead {
            final_url: url.to_string(),
            expires_at_unix: 1_700_000_000,
        })
    }
}

/// Fetcher that records in-flight call counts and tracks the maximum
/// concurrency observed. Used by the semaphore-bounds test.
struct ConcurrencyProbe {
    in_flight: Arc<AtomicUsize>,
    max_observed: Arc<AtomicUsize>,
}

#[async_trait]
impl UrlHeadFetcher for ConcurrencyProbe {
    async fn head(&self, url: &str) -> Result<UrlHead, viscos_media::MediaError> {
        let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_observed.fetch_max(current, Ordering::SeqCst);
        // Hold the permit long enough for sibling tasks to pile up.
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(UrlHead {
            final_url: url.to_string(),
            expires_at_unix: 1_700_000_000,
        })
    }
}

/// Fetcher that fails the first `fail_until - 1` attempts and succeeds
/// on attempt `fail_until`. Used by the retry test.
struct FlakyFetcher {
    attempts: Arc<AtomicU32>,
    fail_until: u32,
}

#[async_trait]
impl UrlHeadFetcher for FlakyFetcher {
    async fn head(&self, url: &str) -> Result<UrlHead, viscos_media::MediaError> {
        let n = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if n < self.fail_until {
            Err(viscos_media::MediaError::Cdn(format!(
                "transient failure {n}"
            )))
        } else {
            Ok(UrlHead {
                final_url: url.to_string(),
                expires_at_unix: 1_700_000_000,
            })
        }
    }
}

/// Fetcher that always fails. Used to verify the failure path produces
/// `RefreshStatus::Failed` after exhausting retries.
struct AlwaysFailFetcher;

#[async_trait]
impl UrlHeadFetcher for AlwaysFailFetcher {
    async fn head(&self, _url: &str) -> Result<UrlHead, viscos_media::MediaError> {
        Err(viscos_media::MediaError::Cdn("always fails".to_string()))
    }
}

/// Test backoff schedule (1 ms / 2 ms) so the retry test stays
/// under a few milliseconds wall-clock per attempt.
const TEST_BACKOFFS: [Duration; 2] = [Duration::from_millis(1), Duration::from_millis(2)];

/// Test config with millisecond-scale backoffs so the retry test stays
/// under a few milliseconds wall-clock per attempt.
fn fast_config(concurrency: usize) -> RefreshConfig {
    RefreshConfig {
        concurrency,
        retry_policy: RetryPolicy {
            max_attempts: 3,
            backoffs: &TEST_BACKOFFS,
        },
    }
}

// ----- classification + parse tests -----

#[tokio::test]
async fn refresh_batch_parses_urls_correctly() {
    let fetcher = Arc::new(OkFetcher);
    let urls: &[&str] = &[
        "https://cdn.discordapp.com/attachments/1/2/foo.png?ex=100",
        "http://example.com/asset",
        "ftp://nope.example/file",    // → Skipped unsupported-scheme
        "data:image/png;base64,abcd", // → Skipped missing-scheme
        "not-a-url",                  // → Skipped missing-scheme
        "https://",                   // → Skipped missing-host
    ];
    let report = refresh_batch(urls, fast_config(4), fetcher)
        .await
        .expect("batch ok");

    assert_eq!(report.total, 6);
    assert_eq!(report.refreshed, 2, "two http(s) URLs should refresh");
    assert_eq!(report.skipped, 4, "four invalid URLs should skip");
    assert_eq!(report.failed, 0);
    assert!(report.all_succeeded());

    // Order preservation: outcomes[i] corresponds to urls[i].
    for (i, outcome) in report.outcomes.iter().enumerate() {
        assert_eq!(outcome.url, urls[i]);
    }

    let reasons: Vec<&'static str> = report
        .outcomes
        .iter()
        .filter_map(|o| match o.status {
            RefreshStatus::Skipped { reason } => Some(reason),
            _ => None,
        })
        .collect();
    assert!(reasons.contains(&"unsupported-scheme"));
    assert!(reasons.contains(&"missing-scheme"));
    assert!(reasons.contains(&"missing-host"));
}

#[tokio::test]
async fn refresh_batch_handles_empty_input() {
    let fetcher = Arc::new(OkFetcher);
    let urls: &[&str] = &[];
    let report = refresh_batch(urls, fast_config(4), fetcher)
        .await
        .expect("empty batch ok");
    assert_eq!(report.total, 0);
    assert_eq!(report.refreshed, 0);
    assert_eq!(report.skipped, 0);
    assert_eq!(report.failed, 0);
    assert!(report.outcomes.is_empty());
    assert!(report.all_succeeded());
}

#[tokio::test]
async fn refresh_batch_respects_concurrency_limit() {
    // 20 URLs, concurrency = 4, each fetch holds the in-flight counter
    // for 20 ms. The semaphore must cap concurrency at exactly 4.
    let fetcher = Arc::new(ConcurrencyProbe {
        in_flight: Arc::new(AtomicUsize::new(0)),
        max_observed: Arc::new(AtomicUsize::new(0)),
    });
    let urls: Vec<String> = (0..20)
        .map(|i| format!("https://cdn.discordapp.com/{i}"))
        .collect();
    let url_refs: Vec<&str> = urls.iter().map(String::as_str).collect();

    let report = refresh_batch(&url_refs, fast_config(4), fetcher.clone())
        .await
        .expect("batch ok");
    assert_eq!(report.total, 20);
    assert_eq!(report.refreshed, 20);

    let max = fetcher.max_observed.load(Ordering::SeqCst);
    assert!(
        max <= 4,
        "max in-flight ({max}) must respect concurrency limit (4)"
    );
    assert!(
        max >= 2,
        "max in-flight ({max}) should actually exercise concurrency (>1)"
    );
}

#[tokio::test]
async fn refresh_batch_retries_on_transient_error() {
    // fail_until = 3 → fetcher fails twice (attempts 1 and 2), succeeds
    // on attempt 3. With backoffs [1ms, 2ms] the total wait is ~3 ms.
    let fetcher = Arc::new(FlakyFetcher {
        attempts: Arc::new(AtomicU32::new(0)),
        fail_until: 3,
    });
    let urls: &[&str] = &["https://cdn.discordapp.com/attachments/x/y"];
    let report = refresh_batch(urls, fast_config(1), fetcher.clone())
        .await
        .expect("batch ok");
    assert_eq!(report.refreshed, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(
        fetcher.attempts.load(Ordering::SeqCst),
        3,
        "fetcher must be invoked exactly max_attempts (3) times"
    );
}

#[tokio::test]
async fn refresh_batch_marks_failed_after_exhausting_retries() {
    let fetcher = Arc::new(AlwaysFailFetcher);
    let urls: &[&str] = &["https://cdn.discordapp.com/attachments/x/y"];
    let report = refresh_batch(urls, fast_config(1), fetcher)
        .await
        .expect("batch ok");
    assert_eq!(report.failed, 1);
    assert_eq!(report.refreshed, 0);
    match &report.outcomes[0].status {
        RefreshStatus::Failed { attempts, reason } => {
            assert_eq!(*attempts, 3, "all 3 attempts must be reported");
            assert!(reason.contains("always fails"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
    assert!(!report.all_succeeded());
}

#[tokio::test]
async fn static_head_fetcher_returns_input_with_24h_expiry() {
    let fetcher = StaticHeadFetcher;
    let head = fetcher
        .head("https://cdn.discordapp.com/x")
        .await
        .expect("static head ok");
    assert_eq!(head.final_url, "https://cdn.discordapp.com/x");
    // Expiry should be roughly now + 24h. Tolerate small clock skew.
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let one_day = 24 * 3600;
    assert!(
        head.expires_at_unix >= now_secs + one_day - 5,
        "expires_at_unix ({}) should be >= now+24h-5s ({})",
        head.expires_at_unix,
        now_secs + one_day - 5
    );
    assert!(
        head.expires_at_unix <= now_secs + one_day + 5,
        "expires_at_unix ({}) should be <= now+24h+5s ({})",
        head.expires_at_unix,
        now_secs + one_day + 5
    );
}
