//! Integration tests for viscos-cache (open + migrate + MessageCache put/get).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use viscos_cache::{Db, MessageCache};

#[test]
fn db_open_and_migrate_idempotent() {
    let path = tempdir_path("db");
    let db = Db::open(&path).expect("open");
    db.migrate().expect("migrate");

    // Re-open to verify schema persists and migration is idempotent.
    let db2 = Db::open(&path).expect("reopen");
    db2.migrate().expect("migrate again (idempotent)");
}

#[tokio::test]
async fn message_cache_put_get_invalidate() {
    let cache: MessageCache = MessageCache::new(100);
    let msg = viscos_cache::cache::Message::new(1, 1, "hello");
    let _ = cache.put(1, msg).await;
    let got = cache.get(1).await.expect("get").expect("hit");
    assert_eq!(got.id, 1);
    assert_eq!(got.content, "hello");

    cache.invalidate(1).await;
    assert!(cache.get(1).await.expect("get").is_none());
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn tempdir_path(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    p.push(format!("viscos-cache-test-{prefix}-{pid}-{seq}"));
    std::fs::create_dir_all(&p).unwrap();
    p.push("cache.db");
    p
}
