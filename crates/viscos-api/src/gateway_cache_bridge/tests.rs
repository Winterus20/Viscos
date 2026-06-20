//! `GatewayCacheBridge` test modulu - ayri dosyada tutulur (production
//! `gateway_cache_bridge.rs` 400 satir sinirinin altinda kalsin diye).
//!
//! Twilight 0.17 fixture'lari `serde_json::from_str` ile deserialize edilir
//! (gercek Discord baglantisi yok, audit `events_try_from.rs` pattern'i).
//! `moka::future::Cache` single-thread tokio runtime'da `yield + run_pending_tasks`
//! gerektirir (multi-thread runtime'da buna gerek yok).

use super::*;
use tempfile::TempDir;
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker};

/// Gecici bir DB + MessageCache + IPC sender uclusu kurar. Test sonunda
/// `tempdir` drop edilince DB dosyasi silinir.
fn make_fixture() -> (
    TempDir,
    Arc<Db>,
    Arc<MessageCache>,
    UnboundedSender<IpcEvent>,
    tokio::sync::mpsc::UnboundedReceiver<IpcEvent>,
) {
    let tmp = TempDir::new().expect("tempdir");
    let db = Arc::new(Db::open(tmp.path().join("cache.db")).expect("db open"));
    db.migrate().expect("migrate");
    let msg_cache = Arc::new(MessageCache::new(64));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<IpcEvent>();
    (tmp, db, msg_cache, tx, rx)
}

fn user_id() -> Id<UserMarker> {
    Id::new(111_111_111_111_111_111)
}
fn guild_id() -> Id<GuildMarker> {
    Id::new(222_222_222_222_222_222)
}
fn channel_id() -> Id<ChannelMarker> {
    Id::new(333_333_333_333_333_333)
}
fn message_id() -> Id<MessageMarker> {
    Id::new(444_444_444_444_444_444)
}
fn user_id_marker() -> u64 {
    user_id().get()
}
fn channel_id_marker() -> u64 {
    channel_id().get()
}
fn message_id_marker() -> u64 {
    message_id().get()
}

fn twilight_message(content: &str) -> twilight_model::channel::Message {
    let json = format!(
        r#"{{
            "id": "444444444444444444",
            "channel_id": "333333333333333333",
            "guild_id": "222222222222222222",
            "author": {{
                "id": "111111111111111111",
                "username": "alice",
                "discriminator": "1",
                "avatar": null,
                "bot": false,
                "system": null,
                "mfa_enabled": null,
                "verified": null,
                "email": null,
                "flags": null,
                "public_flags": null,
                "global_name": null
            }},
            "content": "{content}",
            "timestamp": "2026-06-19T12:00:00.000000+00:00",
            "edited_timestamp": null,
            "tts": false,
            "mention_everyone": false,
            "mentions": [],
            "mention_roles": [],
            "attachments": [],
            "embeds": [],
            "pinned": false,
            "type": 0
        }}"#
    );
    serde_json::from_str(&json).expect("Message deserialize")
}

fn twilight_current_user() -> CurrentUser {
    let json = r#"{
        "id": "111111111111111111",
        "username": "alice",
        "discriminator": "1",
        "avatar": null,
        "bot": false,
        "system": null,
        "mfa_enabled": false,
        "verified": null,
        "email": null,
        "flags": null,
        "public_flags": null,
        "global_name": null,
        "accent_color": null,
        "banner": null,
        "locale": "en-US",
        "premium_type": null
    }"#;
    serde_json::from_str(json).expect("CurrentUser deserialize")
}

fn twilight_guild() -> TwGuild {
    // Twilight 0.17'de Guild 51 alanli. Sadece zorunlu olanlari dolduruyoruz
    // (serde default'lar geri kalani doldurur).
    let json = r#"{
        "id": "222222222222222222",
        "name": "Test Guild",
        "icon": null,
        "owner_id": "111111111111111111",
        "afk_channel_id": null,
        "afk_timeout": 0,
        "verification_level": 0,
        "default_message_notifications": 0,
        "explicit_content_filter": 0,
        "features": [],
        "application_id": null,
        "banner": null,
        "description": null,
        "discovery_splash": null,
        "emojis": [],
        "guild_scheduled_events": [],
        "joined_at": "2026-06-19T12:00:00.000000+00:00",
        "large": false,
        "max_members": null,
        "max_presences": null,
        "members": [],
        "mfa_level": 0,
        "nsfw_level": 0,
        "owner": false,
        "permissions": null,
        "preferred_locale": "en-US",
        "premium_progress_bar_enabled": false,
        "premium_subscription_count": 0,
        "premium_tier": 0,
        "presences": [],
        "public_updates_channel_id": null,
        "roles": [],
        "rules_channel_id": null,
        "safety_alerts_channel_id": null,
        "splash": null,
        "stage_instances": [],
        "stickers": [],
        "system_channel_flags": 0,
        "system_channel_id": null,
        "threads": [],
        "unavailable": false,
        "vanity_url_code": null,
        "voice_states": [],
        "widget_channel_id": null,
        "widget_enabled": false,
        "max_stage_video_channel_users": null,
        "max_video_channel_users": null,
        "approximate_member_count": null,
        "approximate_presence_count": null,
        "channels": [
            {
                "id": "333333333333333333",
                "type": 0,
                "name": "general",
                "guild_id": "222222222222222222"
            }
        ]
    }"#;
    serde_json::from_str(json).expect("Guild deserialize")
}

#[tokio::test]
async fn on_message_create_writes_cache_and_emits_event() {
    let (_tmp, db, msg_cache, tx, mut rx) = make_fixture();

    let bridge = GatewayCacheBridge::new(db.clone(), msg_cache.clone(), tx);

    let msg = twilight_message("hello world");
    let msg_id_u64 = msg.id.get();
    let channel_id_u64 = msg.channel_id.get();
    bridge.on_message_create(msg).await.expect("handle");

    // moka future cache - `current_thread` runtime'da pending task drain
    // icin yield. Multi-thread runtime'da buna gerek yok.
    tokio::task::yield_now().await;
    msg_cache.run_pending_tasks().await;

    // 1) MessageCache hit
    let cached = msg_cache.get(msg_id_u64).await.expect("get").expect("hit");
    assert_eq!(cached.content, "hello world");
    assert_eq!(cached.channel_id, channel_id_u64);

    // 2) SQLite WAL row mevcut
    let conn = db.conn().expect("conn");
    let row: (String, u64, String) = conn
        .query_row(
            "SELECT id, channel_id, content FROM messages WHERE id = ?1",
            rusqlite::params![message_id().get().to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("row");
    assert_eq!(row.0, message_id().get().to_string());
    assert_eq!(row.1, channel_id().get());
    assert_eq!(row.2, "hello world");

    // 3) IPC event yakalandi
    let event = rx.recv().await.expect("event");
    assert!(
        matches!(event, IpcEvent::MessageCreated { channel_id, message_id }
            if channel_id == channel_id_marker() && message_id == message_id_marker()),
        "unexpected event: {event:?}"
    );
}

#[tokio::test]
async fn on_message_update_overwrites_content() {
    let (_tmp, db, msg_cache, tx, mut rx) = make_fixture();

    let bridge = GatewayCacheBridge::new(db.clone(), msg_cache.clone(), tx);

    bridge
        .on_message_create(twilight_message("first"))
        .await
        .expect("create");
    // Ilk event'i tuket
    let _ = rx.recv().await;

    bridge
        .on_message_update(twilight_message("edited"))
        .await
        .expect("update");

    // Cache guncel
    let cached = msg_cache
        .get(message_id().get())
        .await
        .expect("get")
        .expect("hit");
    assert_eq!(cached.content, "edited");

    // DB guncel
    let conn = db.conn().expect("conn");
    let content: String = conn
        .query_row(
            "SELECT content FROM messages WHERE id = ?1",
            rusqlite::params![message_id().get().to_string()],
            |r| r.get(0),
        )
        .expect("row");
    assert_eq!(content, "edited");

    // IPC MessageEdited push
    let event = rx.recv().await.expect("event");
    assert!(matches!(event, IpcEvent::MessageEdited { .. }));
}

#[tokio::test]
async fn on_guild_create_inserts_guild_and_channels() {
    let (_tmp, db, _msg_cache, tx, _rx) = make_fixture();
    let bridge = GatewayCacheBridge::new(db.clone(), Arc::new(MessageCache::new(8)), tx);

    bridge
        .on_guild_create(twilight_guild())
        .await
        .expect("guild create");

    let conn = db.conn().expect("conn");
    // Guild satiri
    let name: String = conn
        .query_row(
            "SELECT name FROM guilds WHERE id = ?1",
            rusqlite::params![guild_id().get()],
            |r| r.get(0),
        )
        .expect("guild row");
    assert_eq!(name, "Test Guild");

    // Kanal sayisi
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channels WHERE guild_id = ?1",
            rusqlite::params![guild_id().get()],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(count, 1, "1 kanal eklenmis olmali");
}

#[tokio::test]
async fn on_ready_pushes_login_success() {
    let (_tmp, db, _msg_cache, tx, mut rx) = make_fixture();

    let bridge = GatewayCacheBridge::new(db, Arc::new(MessageCache::new(8)), tx);
    let user = twilight_current_user();
    let guilds = vec![twilight_model::guild::UnavailableGuild {
        id: guild_id(),
        unavailable: false,
    }];

    bridge.on_ready(&user, &guilds).await.expect("ready");

    let event = rx.recv().await.expect("event");
    assert!(matches!(event, IpcEvent::LoginSuccess { user_id } if user_id == user_id_marker()));
}

#[tokio::test]
async fn handle_event_dispatches_message_create() {
    let (_tmp, db, msg_cache, tx, mut rx) = make_fixture();

    let bridge = GatewayCacheBridge::new(db.clone(), msg_cache.clone(), tx);

    let event = GatewayEvent::MessageCreate(Box::new(
        twilight_model::gateway::payload::incoming::MessageCreate(twilight_message("hi")),
    ));
    bridge.handle_event(event).await.expect("dispatch");

    // Cache hit
    let cached = msg_cache
        .get(message_id().get())
        .await
        .expect("get")
        .expect("hit");
    assert_eq!(cached.content, "hi");

    // IPC event yakalandi
    let event = rx.recv().await.expect("event");
    assert!(matches!(event, IpcEvent::MessageCreated { .. }));
}

#[tokio::test]
async fn handle_event_ignores_lifecycle_events() {
    let (_tmp, db, msg_cache, tx, _rx) = make_fixture();
    let bridge = GatewayCacheBridge::new(db, msg_cache, tx);

    let event = GatewayEvent::Resumed;
    let result = bridge.handle_event(event).await;
    assert!(result.is_ok(), "Resumed event should be a no-op");

    let event = GatewayEvent::Reconnect;
    let result = bridge.handle_event(event).await;
    assert!(result.is_ok(), "Reconnect event should be a no-op");
}

#[tokio::test]
async fn ipc_channel_closed_returns_bridge_error() {
    let (_tmp, db, msg_cache, _tx, _rx) = make_fixture();
    // Yeni bir sender kur ama receiver'i drop et.
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<IpcEvent>();
    drop(rx);
    let bridge = GatewayCacheBridge::new(db, msg_cache, tx);

    let result = bridge.on_message_create(twilight_message("test")).await;
    assert!(result.is_err(), "channel closed hata donmeli");
    match result {
        Err(BridgeError::Ipc(IpcEventError::ChannelClosed)) => {}
        Err(other) => panic!("expected ChannelClosed, got {other:?}"),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn bridge_error_wraps_serde_json_error() {
    let bad = serde_json::from_str::<serde_json::Value>("{not valid}");
    let serde_err = bad.expect_err("invalid JSON");
    let bridge_err: BridgeError = serde_err.into();
    assert!(matches!(bridge_err, BridgeError::Twilight(_)));
}

#[test]
fn bridge_error_converts_to_viscos_error() {
    let bridge_err = BridgeError::Twilight("test".to_string());
    let viscos_err: ViscosError = bridge_err.into();
    // Twilight hatasi `ViscosError::Media` uzerinden tasinir.
    assert!(matches!(viscos_err, ViscosError::Media(_)));
}
