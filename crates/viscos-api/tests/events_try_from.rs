//! `GatewayEvent::try_from(twilight::Event)` coverage — her twilight varyantı
//! için dönüşümün doğru Viscos enum koluna düştüğünü doğrular.
//!
//! Twilight büyük payload'ları `Box<T>` ile sarar (`MessageCreate`,
//! `MessageUpdate`, `GuildCreate`, `ReactionAdd`, `TypingStart`, `Ready`).
//! Burada `TwilightEvent`'i doğrudan constructor ile üretip variant eşleşmesini
//! kontrol ediyoruz — gerçek Discord bağlantısı yok.
//!
//! **Strateji:** Bazı twilight tipleri (`Ready`, `MessageCreate`...) karmaşık
//! alanlar içeriyor ve serde-deserialize olmadan struct literal kurmak zahmetli.
//! Lifecycle event'ler için (Ready, Resumed, Reconnect, vs.) direkt variant
//! kuruyoruz; mesaj event'leri için `Default::default()` ile tüm alanları
//! sıfırlayıp sadece ID'leri set ediyoruz — `Default` derive edilmediyse
//! `twilight_model`'in test fixture'larına güvenmek yerine sadece ID round-trip
//! kontrolü yapıyoruz.

use twilight_gateway::Event;
use twilight_model::gateway::event::EventType;
use twilight_model::id::Id;
use viscos_api::GatewayEvent;

fn user_id() -> Id<twilight_model::id::marker::UserMarker> {
    Id::new(111_111_111_111_111_111)
}

fn guild_id() -> Id<twilight_model::id::marker::GuildMarker> {
    Id::new(222_222_222_222_222_222)
}

fn channel_id() -> Id<twilight_model::id::marker::ChannelMarker> {
    Id::new(333_333_333_333_333_333)
}

fn message_id() -> Id<twilight_model::id::marker::MessageMarker> {
    Id::new(444_444_444_444_444_444)
}

// -----------------------------------------------------------------------------
// Lifecycle event'ler — basit, payload-free, direkt variant kurulumu.
// -----------------------------------------------------------------------------

#[test]
fn ready_event_maps_to_ready_variant() {
    // `Ready`'i serde-deserialize ile kuruyoruz çünkü 51 alanlı `CurrentUser`
    // ve `Guild` zincirleri literal construction'ı pratik dışı bırakıyor.
    // `application.id` non-zero u64; `shard: [0, 2]` (number=0, total=2).
    let json = r#"{
        "v": 10,
        "user": {
            "id": "111111111111111111",
            "username": "tester",
            "discriminator": "1",
            "avatar": null,
            "bot": false,
            "system": null,
            "mfa_enabled": false,
            "verified": null,
            "email": null,
            "flags": null,
            "public_flags": null,
            "global_name": null
        },
        "guilds": [],
        "session_id": "session-abc",
        "resume_gateway_url": "wss://gateway.resume.example",
        "application": { "id": "1", "flags": 0 },
        "shard": [0, 2]
    }"#;
    let ready: twilight_model::gateway::payload::incoming::Ready =
        serde_json::from_str(json).expect("Ready deserialize");
    let event: Event = Event::Ready(ready);
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::Ready(ready) => {
            assert_eq!(ready.session_id, "session-abc");
            assert_eq!(ready.resume_gateway_url, "wss://gateway.resume.example");
        }
        other => panic!("expected GatewayEvent::Ready, got {other:?}"),
    }
}

#[test]
fn message_create_event_maps_to_message_create_variant() {
    let json = r#"{
        "id": "444444444444444444",
        "channel_id": "333333333333333333",
        "guild_id": "222222222222222222",
        "author": {
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
        },
        "content": "hello world",
        "timestamp": "2026-06-18T12:00:00.000000+00:00",
        "edited_timestamp": null,
        "tts": false,
        "mention_everyone": false,
        "mentions": [],
        "mention_roles": [],
        "attachments": [],
        "embeds": [],
        "pinned": false,
        "type": 0
    }"#;
    let mc: twilight_model::gateway::payload::incoming::MessageCreate =
        serde_json::from_str(json).expect("MessageCreate deserialize");
    let event: Event = Event::MessageCreate(Box::new(mc));
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::MessageCreate(m) => {
            assert_eq!(m.content, "hello world");
            assert_eq!(m.id, message_id());
        }
        other => panic!("expected GatewayEvent::MessageCreate, got {other:?}"),
    }
}

#[test]
fn message_update_event_maps_to_message_update_variant() {
    // `MessageUpdate(pub Message)` — full Message payload bekleniyor
    // (Discord 0.17'de bu breaking change yapıldı).
    let json = r#"{
        "id": "444444444444444444",
        "channel_id": "333333333333333333",
        "guild_id": "222222222222222222",
        "author": {
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
        },
        "content": "edited body",
        "timestamp": "2026-06-18T12:00:00.000000+00:00",
        "edited_timestamp": "2026-06-18T12:05:00.000000+00:00",
        "tts": false,
        "mention_everyone": false,
        "mentions": [],
        "mention_roles": [],
        "attachments": [],
        "embeds": [],
        "pinned": false,
        "type": 0
    }"#;
    let mu: twilight_model::gateway::payload::incoming::MessageUpdate =
        serde_json::from_str(json).expect("MessageUpdate deserialize");
    let event: Event = Event::MessageUpdate(Box::new(mu));
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::MessageUpdate(m) => {
            // `MessageUpdate(pub Message)` newtype — deref ile content erişimi.
            assert_eq!(m.id, message_id());
            assert_eq!(m.content, "edited body");
        }
        other => panic!("expected GatewayEvent::MessageUpdate, got {other:?}"),
    }
}

#[test]
fn message_delete_event_maps_to_message_delete_variant() {
    let payload = twilight_model::gateway::payload::incoming::MessageDelete {
        channel_id: channel_id(),
        guild_id: Some(guild_id()),
        id: message_id(),
    };
    let event: Event = Event::MessageDelete(payload);
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::MessageDelete(d) => {
            assert_eq!(d.id, message_id());
            assert_eq!(d.channel_id, channel_id());
        }
        other => panic!("expected GatewayEvent::MessageDelete, got {other:?}"),
    }
}

#[test]
fn guild_create_event_maps_to_guild_create_variant() {
    // `GuildCreate::Unavailable(UnavailableGuild)` — `guild::UnavailableGuild`
    // { id, unavailable: bool } (Option değil).
    let unavailable = twilight_model::guild::UnavailableGuild {
        id: guild_id(),
        unavailable: true,
    };
    let event: Event = Event::GuildCreate(Box::new(
        twilight_model::gateway::payload::incoming::GuildCreate::Unavailable(unavailable),
    ));
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::GuildCreate(g) => {
            // Ortak `id()` methodu hem Available hem Unavailable'da çalışır.
            assert_eq!(g.id(), guild_id());
        }
        other => panic!("expected GatewayEvent::GuildCreate, got {other:?}"),
    }
}

#[test]
fn reaction_add_event_maps_to_reaction_add_variant() {
    // `ReactionAdd(pub GatewayReaction)` newtype. `GatewayReaction` kompleks
    // değil — direkt literal kurulabilir.
    let gateway_reaction = twilight_model::gateway::GatewayReaction {
        burst: false,
        burst_colors: Vec::new(),
        channel_id: channel_id(),
        emoji: twilight_model::channel::message::EmojiReactionType::Unicode {
            name: "👍".to_string(),
        },
        guild_id: Some(guild_id()),
        member: None,
        message_author_id: None,
        message_id: message_id(),
        user_id: user_id(),
    };
    let event: Event = Event::ReactionAdd(Box::new(
        twilight_model::gateway::payload::incoming::ReactionAdd(gateway_reaction),
    ));
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::ReactionAdd(r) => {
            assert_eq!(r.user_id, user_id());
            assert_eq!(r.message_id, message_id());
        }
        other => panic!("expected GatewayEvent::ReactionAdd, got {other:?}"),
    }
}

#[test]
fn typing_start_event_maps_to_typing_start_variant() {
    // `TypingStart` flat struct (newtype değil): channel_id, guild_id,
    // member, timestamp, user_id.
    let payload = twilight_model::gateway::payload::incoming::TypingStart {
        channel_id: channel_id(),
        guild_id: Some(guild_id()),
        user_id: user_id(),
        timestamp: 1_716_000_000,
        member: None,
    };
    let event: Event = Event::TypingStart(Box::new(payload));
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::TypingStart(t) => {
            assert_eq!(t.user_id, user_id());
            assert_eq!(t.channel_id, channel_id());
            assert_eq!(t.timestamp, 1_716_000_000);
        }
        other => panic!("expected GatewayEvent::TypingStart, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// Lifecycle event'ler — payload-free, doğrudan variant kurulumu.
// -----------------------------------------------------------------------------

#[test]
fn resumed_event_maps_to_resumed_variant() {
    let event: Event = Event::Resumed;
    assert!(matches!(
        GatewayEvent::try_from(event).expect("conversion ok"),
        GatewayEvent::Resumed
    ));
}

#[test]
fn gateway_reconnect_maps_to_reconnect_variant() {
    let event: Event = Event::GatewayReconnect;
    assert!(matches!(
        GatewayEvent::try_from(event).expect("conversion ok"),
        GatewayEvent::Reconnect
    ));
}

#[test]
fn gateway_invalidate_session_maps_to_session_invalidated() {
    let event: Event = Event::GatewayInvalidateSession(true);
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::SessionInvalidated { resumable } => assert!(resumable),
        other => panic!("expected SessionInvalidated, got {other:?}"),
    }
}

#[test]
fn gateway_heartbeat_ack_maps_to_heartbeat_ack() {
    let event: Event = Event::GatewayHeartbeatAck;
    assert!(matches!(
        GatewayEvent::try_from(event).expect("conversion ok"),
        GatewayEvent::HeartbeatAck
    ));
}

#[test]
fn gateway_hello_maps_to_hello_variant() {
    let payload = twilight_model::gateway::payload::incoming::Hello {
        heartbeat_interval: 41_250,
    };
    let event: Event = Event::GatewayHello(payload);
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::Hello {
            heartbeat_interval_ms,
        } => {
            assert_eq!(heartbeat_interval_ms, 41_250);
        }
        other => panic!("expected Hello, got {other:?}"),
    }
}

#[test]
fn unknown_variant_falls_through_to_unknown() {
    // Event::RateLimited — bilinçli olarak dönüştürmediğimiz bir varyant,
    // Unknown'a düşmesini bekliyoruz.
    let event: Event =
        Event::RateLimited(twilight_model::gateway::payload::incoming::RateLimited {
            opcode: twilight_model::gateway::OpCode::RequestGuildMembers,
            retry_after: 1.0,
            meta:
                twilight_model::gateway::payload::incoming::rate_limited::RateLimitMetadata::RequestGuildMembers {
                    guild_id: Id::new(987_654_321),
                    nonce: None,
                },
        });
    match GatewayEvent::try_from(event).expect("conversion ok") {
        GatewayEvent::Unknown(et) => assert_eq!(et, EventType::RateLimited),
        other => panic!("expected Unknown variant, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// L ifecycleEvent round-trip — Viscos L ifecycleEvent köprüsü.
// -----------------------------------------------------------------------------

#[test]
fn lifecycle_bridge_extracts_known_lifecycle_events() {
    use viscos_api::LifecycleEvent;

    let hello = GatewayEvent::Hello {
        heartbeat_interval_ms: 42_000,
    };
    assert_eq!(
        hello.as_lifecycle(),
        Some(LifecycleEvent::Hello {
            heartbeat_interval_ms: 42_000
        })
    );

    let resumed = GatewayEvent::Resumed;
    assert_eq!(resumed.as_lifecycle(), Some(LifecycleEvent::Resumed));

    let reconnect = GatewayEvent::Reconnect;
    assert_eq!(reconnect.as_lifecycle(), Some(LifecycleEvent::Reconnect));

    let invalidated = GatewayEvent::SessionInvalidated { resumable: false };
    assert_eq!(
        invalidated.as_lifecycle(),
        Some(LifecycleEvent::SessionInvalidated { resumable: false })
    );

    let close = GatewayEvent::GatewayClose {
        code: Some(4004),
        reason: Some("Authentication failed".to_string()),
    };
    assert_eq!(
        close.as_lifecycle(),
        Some(LifecycleEvent::GatewayClose {
            code: Some(4004),
            reason: Some("Authentication failed".to_string()),
        })
    );
}

#[test]
fn lifecycle_bridge_is_none_for_payload_events() {
    use twilight_model::gateway::payload::incoming::MessageDelete;

    let ev = GatewayEvent::MessageDelete(MessageDelete {
        channel_id: Id::new(1),
        guild_id: None,
        id: Id::new(2),
    });
    assert!(ev.as_lifecycle().is_none());
}
