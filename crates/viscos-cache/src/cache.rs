//! moka in-memory message cache (ADR-0010 §3.3).
//!
//! Faz 4.0 Dalga 1: `moka::future::Cache<u64, Arc<Message>>` — sıcak kanal scroll
//! pattern'i için. TinyLFU admission policy (moka default), per-entry TTL/TTI.
//!
//! **Not:** `viscos-core::types::Message` henüz minimal — Faz 2'de `twilight-model`
//! typed Message ile doldurulacak. v1'de `Message` placeholder'ı yeterli; cache
//! katmanı generic tip kullandığı için API değişmez.

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;

use crate::error::Result;

/// Placeholder `Message` type. Faz 2'de `twilight_model::message::Message`'a evolve eder.
/// `Arc<Message>` ile sarmalama: aynı mesaj birden fazla subscriber'a paylaştırılırken
/// allocation yok.
#[derive(Debug, Clone, Default)]
pub struct Message {
    /// Discord message snowflake (u64).
    pub id: u64,
    /// Channel snowflake.
    pub channel_id: u64,
    /// Author user snowflake.
    pub author_id: Option<u64>,
    /// Raw markdown content.
    pub content: String,
    /// Unix epoch seconds.
    pub timestamp: i64,
}

impl Message {
    /// Construct a new `Message` from primitive fields. Used by tests + future
    /// Discord Gateway deserialization.
    pub fn new(id: u64, channel_id: u64, content: impl Into<String>) -> Self {
        Self {
            id,
            channel_id,
            author_id: None,
            content: content.into(),
            timestamp: 0,
        }
    }
}

/// moka-backed message cache. Capacity measured in entry count (not bytes — moka
/// future API'sinde weighted size ileride eklenebilir).
///
/// **Defaults:**
/// - TTL: 1 saat (3600s) — sıcak scroll dışına çıkan mesajlar evict olur
/// - TTI: 5 dakika (300s) — idle window sonrası evict
pub struct MessageCache {
    inner: Cache<u64, Arc<Message>>,
}

impl MessageCache {
    /// Build a new `MessageCache` with the given entry capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` — Maximum number of entries. moka TinyLFU admission policy
    ///   sıcak erişim pattern'inde hit ratio maximize eder.
    pub fn new(capacity: u64) -> Self {
        let inner = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(Duration::from_secs(3600))
            .time_to_idle(Duration::from_secs(300))
            .build();
        Self { inner }
    }

    /// Look up a message by id. `Some(Arc<Message>)` on hit, `None` on miss.
    pub async fn get(&self, id: u64) -> Result<Option<Message>> {
        Ok(self.inner.get(&id).await.map(|arc| (*arc).clone()))
    }

    /// Insert or update a message. Returns the inserted message for chaining.
    pub async fn put(&self, id: u64, msg: Message) -> Result<()> {
        self.inner.insert(id, Arc::new(msg)).await;
        Ok(())
    }

    /// Explicitly invalidate (remove) a single entry.
    pub async fn invalidate(&self, id: u64) {
        self.inner.invalidate(&id).await;
    }

    /// Current entry count (approximate; moka async maintenance).
    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }

    /// Trigger pending maintenance tasks (eviction, expiration). Useful before
    /// `entry_count` measurement in tests.
    pub async fn run_pending_tasks(&self) {
        self.inner.run_pending_tasks().await;
    }
}

impl std::fmt::Debug for MessageCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageCache")
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_then_get_returns_message() {
        let cache = MessageCache::new(16);
        let msg = Message::new(42, 7, "hello");
        cache.put(42, msg.clone()).await.expect("put");

        let got = cache.get(42).await.expect("get").expect("hit");
        assert_eq!(got.id, 42);
        assert_eq!(got.content, "hello");
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let cache = MessageCache::new(16);
        let got = cache.get(999).await.expect("get");
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn invalidate_removes_entry() {
        let cache = MessageCache::new(16);
        cache.put(1, Message::new(1, 1, "a")).await.expect("put");
        assert!(cache.get(1).await.expect("get").is_some());
        cache.invalidate(1).await;
        assert!(cache.get(1).await.expect("get").is_none());
    }

    #[tokio::test]
    async fn capacity_overflow_triggers_lru_eviction() {
        let cache = MessageCache::new(4);
        // 8 entry ekle → capacity 4, eski 4 evict olmalı.
        for i in 0..8u64 {
            cache.put(i, Message::new(i, 1, "x")).await.expect("put");
        }
        cache.run_pending_tasks().await;
        assert!(cache.entry_count() <= 4, "capacity exceeded");
    }
}
