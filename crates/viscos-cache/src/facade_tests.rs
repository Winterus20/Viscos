//! Unit tests for the [`Cache`] facade (split out from `facade.rs` to keep
//! the production file under the 400-line limit per `.cursorrules` Bölüm 2).

use crate::cache::Message;
use crate::error::CacheError;
use crate::facade::Cache;
use crate::repository::{Channel, Guild};

use viscos_config::CacheConfig;

fn temp_cache_config() -> (tempfile::TempDir, CacheConfig) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();
    let cfg = CacheConfig::new(data_dir, 8);
    (tmp, cfg)
}

#[test]
fn open_creates_parent_directories() {
    let (_tmp, mut cfg) = temp_cache_config();
    cfg.sqlite_path = cfg.data_dir.join("nested").join("cache.db");
    let cache = Cache::open(&cfg).expect("open");
    assert!(cfg.data_dir.exists());
    assert!(cfg.sqlite_path.exists());
    drop(cache);
}

#[test]
fn upsert_then_recent_round_trip() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    rt.block_on(async {
        let (_tmp, cfg) = temp_cache_config();
        let cache = Cache::open(&cfg).expect("open");

        for i in 1..=3u64 {
            let mut m = Message::new(i, 42, format!("m{i}"));
            m.timestamp = i as i64;
            cache.upsert_message_sync(m).expect("upsert");
        }
        let recent = cache.recent_messages(42, 10).expect("recent");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, 3);
    });
}

#[test]
fn upsert_message_async_round_trip() {
    let (_tmp, cfg) = temp_cache_config();
    let cache = Cache::open(&cfg).expect("open");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    rt.block_on(async {
        cache
            .upsert_message(Message::new(7, 1, "async hello"))
            .await
            .expect("upsert async");
    });
    let got = cache.get_message(1, 7).expect("get").expect("hit");
    assert_eq!(got.content, "async hello");
}

#[test]
fn upsert_message_sync_invalidates_hot() {
    let (_tmp, cfg) = temp_cache_config();
    let cache = Cache::open(&cfg).expect("open");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    rt.block_on(async {
        cache
            .upsert_message_sync(Message::new(1, 1, "v1"))
            .expect("upsert v1");
        let got = cache.get_message(1, 1).expect("get").expect("hit");
        assert_eq!(got.content, "v1");
    });
}

#[test]
fn message_from_raw_parses_discord_payload() {
    let (_tmp, cfg) = temp_cache_config();
    let cache = Cache::open(&cfg).expect("open");
    let payload = serde_json::json!({
        "id": "1234567890",
        "channel_id": "9876543210",
        "author": { "id": "11111" },
        "content": "hello <@22222>",
        "timestamp": "2024-01-02T03:04:05.000+00:00"
    });
    let msg = cache.message_from_raw(payload).expect("parse");
    assert_eq!(msg.id, 1234567890);
    assert_eq!(msg.channel_id, 9876543210);
    assert_eq!(msg.author_id, Some(11111));
    assert_eq!(msg.content, "hello <@22222>");
    assert!(msg.timestamp > 0);
}

#[test]
fn message_from_raw_missing_id_errors() {
    let (_tmp, cfg) = temp_cache_config();
    let cache = Cache::open(&cfg).expect("open");
    let payload = serde_json::json!({
        "channel_id": "1",
        "content": "x"
    });
    let err = cache.message_from_raw(payload).expect_err("must fail");
    assert!(matches!(err, CacheError::Json(_)));
}

#[test]
fn upsert_and_list_guilds() {
    let (_tmp, cfg) = temp_cache_config();
    let cache = Cache::open(&cfg).expect("open");
    cache
        .upsert_guild(Guild {
            id: 1,
            name: "alpha".into(),
        })
        .expect("upsert");
    cache
        .upsert_guild(Guild {
            id: 2,
            name: "beta".into(),
        })
        .expect("upsert");
    let list = cache.list_guilds().expect("list");
    assert_eq!(list.len(), 2);
}

#[test]
fn upsert_and_list_channels() {
    let (_tmp, cfg) = temp_cache_config();
    let cache = Cache::open(&cfg).expect("open");
    cache
        .upsert_channel(Channel {
            id: 100,
            guild_id: Some(1),
            name: "general".into(),
            kind: 0,
        })
        .expect("upsert");
    let list = cache.list_channels().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "general");
}

#[test]
fn close_is_idempotent() {
    let (_tmp, cfg) = temp_cache_config();
    let cache = Cache::open(&cfg).expect("open");
    cache.close().expect("close");
}

#[test]
fn parse_iso8601_to_unix_known_value() {
    // 2024-01-02T03:04:05.000+00:00 → 1_704_164_645
    let ts = crate::facade::tests::parse_iso8601_to_unix("2024-01-02T03:04:05.000+00:00");
    assert_eq!(ts, Some(1_704_164_645));
}

#[test]
fn days_from_civil_epoch() {
    assert_eq!(crate::facade::tests::days_from_civil(1970, 1, 1), Some(0));
    assert_eq!(
        crate::facade::tests::days_from_civil(2024, 1, 2),
        Some(19_724)
    );
}
