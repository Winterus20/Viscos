//! `Cache` facade — unified entry point for cache operations (Faz 4.0 Dalga 1, MVP-2).
//!
//! Combines three repository implementations with a moka hot cache:
//! - `Arc<dyn MessageRepository>` — `SqliteMessageRepository` v1.
//! - `Arc<dyn GuildRepository>` + `Arc<dyn ChannelRepository>` — auxiliary.
//! - `moka::future::Cache<u64, Message>` — sıcak RAM tier (Discord message lookup path).
//!
//! ## Write-through
//!
//! `upsert_message_sync` + `upsert_message` both persist to SQLite **and**
//! invalidate the moka hot entry. Read path is moka-first, miss falls back to
//! SQLite. v1 read path bypasses moka (no async miss-fill) — full hit-rate
//! optimization is Dalga 2 (PR-5+).
//!
//! ## Threading
//!
//! `Cache` is `Send + Sync` — `Arc<...>` inner state + `moka::future::Cache`
//! inherent thread safety. Caller clones `Arc<Cache>` freely (matches
//! `main.rs` `Arc<Cache>` usage per audit §2.4).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache as MokaCache;
use serde_json::Value as JsonValue;

use viscos_config::CacheConfig;

use crate::cache::Message;
use crate::error::CacheError;
use crate::repository::{
    Channel, ChannelRepository, Guild, GuildRepository, MessageRepository, SqliteChannelRepository,
    SqliteGuildRepository, SqliteMessageRepository,
};

/// Crate-local error alias. Re-export of [`CacheError`] for callers who want
/// to spell the error type inline (`use viscos_cache::Result<T>`).
pub type Result<T> = std::result::Result<T, CacheError>;

/// Shared cache facade. Clone-via-`Arc::clone(&self)`.
pub struct Cache {
    messages: Arc<dyn MessageRepository>,
    guilds: Arc<dyn GuildRepository>,
    channels: Arc<dyn ChannelRepository>,
    /// Hot path — Discord message lookup (`message_id → Message`). moka
    /// async future cache; moka handles TinyLFU admission + TTL/TTI.
    hot: MokaCache<u64, Message>,
    /// Backing SQLite path (kept for `close()` graceful shutdown + tests).
    sqlite_path: PathBuf,
}

impl Cache {
    /// Open a [`Cache`] backed by SQLite + moka. Parent directory for the
    /// SQLite file is auto-created.
    ///
    /// # Arguments
    ///
    /// * `config` — [`CacheConfig`] from `viscos_config::Config::cache`. The
    ///   `max_size_mb` field drives moka's `max_capacity` (entry count, not
    ///   bytes — see audit §2.4).
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] on path / open / PRAGMA / schema failure.
    pub fn open(config: &CacheConfig) -> Result<Self> {
        Self::open_with_paths(
            &config.sqlite_path,
            &config.data_dir,
            config.max_size_mb.saturating_mul(1024 * 1024),
        )
    }

    /// Open a [`Cache`] with explicit path arguments. Used by tests that bypass
    /// the layered config loader. Capacity is in **bytes** (per
    /// [`CacheConfig::max_size_mb`], callers convert).
    pub fn open_with_paths(
        sqlite_path: &Path,
        cache_root: &Path,
        max_capacity_bytes: u64,
    ) -> Result<Self> {
        // Parent dir auto-create (SQLITE_CANTOPEN errno 14 fix).
        if let Some(parent) = sqlite_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(cache_root)?;

        let msg_repo = SqliteMessageRepository::open(sqlite_path)?;
        let guild_repo = SqliteGuildRepository::open(sqlite_path)?;
        let channel_repo = SqliteChannelRepository::open(sqlite_path)?;

        // Capacity: bytes → entry count heuristic (1 entry ≈ 1 KB average).
        let entry_capacity = (max_capacity_bytes / 1024).max(64);

        let hot = MokaCache::builder()
            .max_capacity(entry_capacity)
            .time_to_live(Duration::from_secs(3600))
            .time_to_idle(Duration::from_secs(300))
            .build();

        Ok(Self {
            messages: Arc::new(msg_repo),
            guilds: Arc::new(guild_repo),
            channels: Arc::new(channel_repo),
            hot,
            sqlite_path: sqlite_path.to_path_buf(),
        })
    }

    /// Persist a message write-through (SQLite upsert + moka invalidate).
    ///
    /// Sync variant — no `.await`. Use this when the caller is already on a
    /// blocking thread (CLI tool, test, `spawn_blocking`).
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] from the underlying repository.
    pub fn upsert_message_sync(&self, msg: Message) -> Result<()> {
        self.messages.upsert(&msg)?;
        // moka future cache: invalidate is async; fire-and-forget best-effort.
        // Hot path: next read repopulates from SQLite on miss.
        let hot = self.hot.clone();
        let id = msg.id;
        tokio::task::spawn(async move {
            hot.invalidate(&id).await;
        });
        Ok(())
    }

    /// Async write-through. Wraps the sync call in `spawn_blocking` so the
    /// caller (e.g. gateway event handler on the tokio runtime) doesn't block
    /// the executor on SQLite I/O.
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] propagated from the repository.
    pub async fn upsert_message(&self, msg: Message) -> Result<()> {
        let messages = Arc::clone(&self.messages);
        let hot = self.hot.clone();
        let id = msg.id;

        tokio::task::spawn_blocking(move || {
            messages.upsert(&msg)?;
            // moka invalidate is async; spawn a separate task for it.
            tokio::task::spawn(async move {
                hot.invalidate(&id).await;
            });
            Ok::<(), CacheError>(())
        })
        .await
        .map_err(|e| CacheError::Join(e.to_string()))?
    }

    /// Most recent `limit` messages for a channel, newest first. v1
    /// SQLite-backed (moka hot is per-message-id, not per-channel, so it
    /// doesn't accelerate channel-list queries).
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] on query failure.
    pub fn recent_messages(&self, channel_id: u64, limit: u32) -> Result<Vec<Message>> {
        self.messages.recent(channel_id, limit)
    }

    /// Single message lookup by composite key. v1 SQLite-backed (moka hot
    /// only accelerates Gateway → IPC push, not the cache_read path).
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] from the underlying query.
    pub fn get_message(&self, channel_id: u64, message_id: u64) -> Result<Option<Message>> {
        self.messages.get(channel_id, message_id)
    }

    /// Convert a Discord Gateway payload (`MESSAGE_CREATE`) into our cache
    /// [`Message`]. Returns [`CacheError::Json`] on shape mismatch.
    ///
    /// Expected payload fields (per Discord v10):
    /// - `id` (string snowflake)
    /// - `channel_id` (string snowflake)
    /// - `author.id` (optional string snowflake)
    /// - `content` (string)
    /// - `timestamp` (ISO-8601 string — we parse to unix seconds)
    ///
    /// # Errors
    ///
    /// [`CacheError::Json`] on missing/invalid fields.
    pub fn message_from_raw(&self, json: JsonValue) -> Result<Message> {
        let id = json
            .get("id")
            .and_then(JsonValue::as_str)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| CacheError::Json("missing 'id'".into()))?;
        let channel_id = json
            .get("channel_id")
            .and_then(JsonValue::as_str)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| CacheError::Json("missing 'channel_id'".into()))?;
        let author_id = json
            .get("author")
            .and_then(|a| a.get("id"))
            .and_then(JsonValue::as_str)
            .and_then(|s| s.parse::<u64>().ok());
        let content = json
            .get("content")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_string();
        let timestamp = json
            .get("timestamp")
            .and_then(JsonValue::as_str)
            .and_then(parse_iso8601_to_unix)
            .unwrap_or(0);

        Ok(Message {
            id,
            channel_id,
            author_id,
            content,
            timestamp,
        })
    }

    /// Upsert a guild.
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] from the underlying repository.
    pub fn upsert_guild(&self, guild: Guild) -> Result<()> {
        self.guilds.upsert(&guild)
    }

    /// List all cached guilds.
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] from the underlying repository.
    pub fn list_guilds(&self) -> Result<Vec<Guild>> {
        self.guilds.list()
    }

    /// Upsert a channel.
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] from the underlying repository.
    pub fn upsert_channel(&self, channel: Channel) -> Result<()> {
        self.channels.upsert(&channel)
    }

    /// List all cached channels.
    ///
    /// # Errors
    ///
    /// [`CacheError::Sqlite`] from the underlying repository.
    pub fn list_channels(&self) -> Result<Vec<Channel>> {
        self.channels.list()
    }

    /// Backing SQLite path (visible for diagnostics / tests).
    #[must_use]
    pub fn sqlite_path(&self) -> &Path {
        &self.sqlite_path
    }

    /// Trigger pending moka maintenance (eviction + expiration). Useful in
    /// tests before measuring `hot.entry_count()`.
    pub async fn run_pending_tasks(&self) {
        self.hot.run_pending_tasks().await;
    }

    /// Graceful close. Currently a no-op (moka async destructor + SQLite
    /// connection drop on inner Arc release handle it). Provided as the
    /// symmetry point for future cleanup (foyer flush, telemetry hook, vb.).
    ///
    /// # Errors
    ///
    /// Reserved for future variants — currently always `Ok(())`.
    pub fn close(self) -> Result<()> {
        drop(self.hot);
        Ok(())
    }
}

/// Parse a Discord ISO-8601 timestamp (`2024-01-02T03:04:05.678+00:00`)
/// into unix epoch seconds. Returns `None` on parse failure (caller falls
/// back to 0).
pub(crate) fn parse_iso8601_to_unix(s: &str) -> Option<i64> {
    // Minimal hand-rolled parser — Discord timestamps are RFC-3339 with
    // millisecond precision. Avoids pulling in `chrono` / `time` for v1.
    let mut iter = s.split('T');
    let date = iter.next()?;
    let rest = iter.next()?;

    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;

    let time = rest.split(['+', 'Z', '-']).next()?;
    let mut time_parts = time.split(':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;
    let second_part = time_parts.next()?;
    let second: u32 = second_part.split('.').next()?.parse().ok()?;

    let days = days_from_civil(year, month as i32, day as i32)?;
    let hms = (hour as i64) * 3600 + (minute as i64) * 60 + second as i64;
    days.checked_mul(86_400).and_then(|d| d.checked_add(hms))
}

/// Howard Hinnant days-from-civil algorithm (reversible, no leap-second
/// games). Returns days since unix epoch (1970-01-01).
pub(crate) fn days_from_civil(y: i32, m: i32, d: i32) -> Option<i64> {
    let y = if m <= 2 { y - 1 } else { y };
    let era = i64::from(y.div_euclid(400));
    let yoe = i64::from(y - era as i32 * 400); // [0, 399]
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + i64::from(doy); // [0, 146096]
    let days = era * 146_097 + doe - 719_468;
    Some(days)
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) use super::{days_from_civil, parse_iso8601_to_unix};
}
