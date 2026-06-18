---
name: Phase 3.0 — Gateway WebSocket + zstd
overview: Discord Gateway WebSocket bağlantısı, Hello→Identify→Ready handshake, jittered heartbeat, zstd streaming decompression, session resume, reconnect logic, event dispatch, intent filtering.
isProject: false
todos:
  - id: gateway-connection
    content: WebSocket bağlantısı (tokio-tungstenite)
    status: pending
  - id: handshake
    content: Hello → Identify → Ready handshake
    status: pending
  - id: heartbeat
    content: Jittered heartbeat (exponential backoff)
    status: pending
  - id: zstd-decompression
    content: zstd streaming decompression
    status: pending
  - id: session-resume
    content: Session resume (RESUMED opcode, RESUME payload)
    status: pending
  - id: reconnect
    content: Reconnect logic (exponential backoff, max retry)
    status: pending
  - id: event-dispatch
    content: Event dispatch (Message Create, Guild Create, vb.)
    status: pending
  - id: intent-filter
    content: Intent filtering (sadece gerekli event'ler)
    status: pending
  - id: sharding-prep
    content: Sharding altyapısı (ileride 2.5K+ sunucu için, v1'de tek shard)
    status: pending
---

# Phase 3.0 — Gateway WebSocket + zstd

> **Süre:** 2–3 hafta
> **Hedef:** Gerçek zamanlı Discord event'lerini almak: mesaj, typing, presence, ses kanalı.
> **Önceki faz:** [`phase-2.0-discord-api.md`](./phase-2.0-discord-api.md)
> **Sonraki faz:** [`phase-4.0-cache-media.md`](./phase-4.0-cache-media.md)

---

## 1. Mimari Genel Bakış

```
┌──────────────────────────────────────────────────────────┐
│ viscos-api                                               │
│  ├─ RestClient (Faz 2)                                   │
│  └─ GatewayClient (YENİ)                                 │
│     ├─ WebSocket (tokio-tungstenite)                    │
│     ├─ zstd decoder (Discord compression)                │
│     ├─ Heartbeat task                                    │
│     ├─ Reconnect state machine                           │
│     └─ Event dispatch (moka broadcast channel)           │
└────────────────────┬─────────────────────────────────────┘
                     │
                     ▼
            ┌─────────────────┐
            │ Discord Gateway │
            │ gateway.discord │
            └─────────────────┘
```

---

## 2. Workspace'e Dependencies

> **Mimari karar (Haziran 2026):** Gateway transport katmanı için sıfırdan `tokio-tungstenite` + manual zstd-stream + manual opcode state machine yerine **`twilight-gateway`** kullanılacak. Tam gerekçe: [`docs/DECISIONS.md` ADR-0008](../DECISIONS.md). Mevcut `phase-3.0-gateway.md` taslağında `read_payload` fonksiyonu zstd-stream framing bug'ı içeriyor (`zstd_buffer.clear()` her frame sonrası → partial frame'ler kaybolur); twilight-gateway production-tested zstd decoder ile gelir.

```toml
[workspace.dependencies]
# Discord Gateway — twilight-gateway (transport + session resume + zstd built-in)
twilight-gateway = { version = "0.17", default-features = false, features = [
    "twilight-http",          # create_recommended helper
    "rustls-platform-verifier",
    "zstd",                   # zstd-stream transport compression
    "simd-json",              # simd-accelerated JSON parsing
] }
twilight-model = { version = "0.17", default-features = false, features = ["tracing"] }
```

**Önceki planda yer alan ama artık gereksiz olan dependency'ler:**
- ❌ `tokio-tungstenite` 0.24 — twilight-gateway zaten tokio-websockets kullanıyor (varsayılan, daha modern)
- ❌ `tungstenite` 0.24 — twilight-gateway içinde
- ❌ `futures-util` 0.3 — twilight kendi `EventStream` tipini veriyor
- ❌ `zstd` 0.13 — twilight-gateway `zstd-safe` kullanıyor (built-in)
- ❌ `async-compression` 0.4 — twilight zstd-stream framing'i built-in

**Neden `tokio-websockets` (`tokio-tungstenite` değil):** Twilight 0.17 varsayılan transport olarak `tokio-websockets` (Gelbpunkt) kullanıyor. Autobahn-strict, SIMD-accelerated masking/UTF-8, performans olarak `fastwebsockets` ile aynı seviyede (ikisi de Deno ekibinin altyapısı). Mevcut plandaki `tokio-tungstenite` 0.24 seçimi demode.

---

## 3. Gateway Opcode'ları

Discord Gateway 14 opcode kullanır:

| Opcode | Name | Direction | Açıklama |
|--------|------|-----------|----------|
| 0 | Dispatch | S→C | Event dispatch |
| 1 | Heartbeat | B | Keepalive |
| 2 | Identify | C→S | İlk bağlantı |
| 3 | Presence Update | C→S | (opsiyonel) |
| 4 | Voice State Update | C→S | (Faz 7) |
| 6 | Resume | C→S | Yeniden bağlanma |
| 7 | Reconnect | S→C | Server restart |
| 8 | Invalid Session | S→C | Resume gerekli değil |
| 9 | Hello | S→C | İlk mesaj, heartbeat interval |
| 10 | Heartbeat ACK | S→C | Heartbeat cevabı |
| 11 | Guild Sync | C→S | (community) |
| 14 | Guild Members Chunk | S→C | |

---

## 4. `viscos-api/src/gateway.rs` (Ana Yapı — twilight-gateway adaptörü)

> **Tasarım notu:** `twilight_gateway::Shard` zaten Hello→Identify→Ready handshake, jittered heartbeat, session resume, zstd-stream decompression, reconnect logic, event dispatching, intent filtering — hepsini yapıyor. Viscos'un yaptığı: (a) twilight Shard'ı config etmek, (b) twilight'ın `Event` enum'unu `viscos-core::events::GatewayEvent`'e map etmek, (c) `EventStream`'i viscos-core event broadcast channel'ına bağlamak.

```rust
use std::sync::Arc;
use std::time::Duration;
use twilight_gateway::{Shard, ShardId, Config, ConfigBuilder, Event, EventType, EventTypeFlags};
use twilight_gateway::stream::EventStream;
use twilight_http::Client as TwilightHttpClient;
use twilight_model::gateway::Intents;
use twilight_model::gateway::presence::{Activity, ClientStatus, Status, UserOrId};
use futures_util::StreamExt;
use tracing::{info, warn, error, debug};
use viscos_core::events::GatewayEvent;
use viscos_error::Result;

const GATEWAY_URL: &str = "wss://gateway.discord.gg";

pub struct GatewayConfig {
    pub token: String,
    pub intents: Intents,
    pub large_threshold: u8,
    pub presence: Option<PresenceUpdate>,
}

#[derive(Debug, Clone)]
pub struct PresenceUpdate {
    pub status: Status,
    pub activities: Vec<Activity>,
}

pub struct GatewayClient {
    config: GatewayConfig,
    event_tx: tokio::sync::broadcast::Sender<GatewayEvent>,
    http: Arc<TwilightHttpClient>,
    shard: Option<Shard>,
}

impl GatewayClient {
    pub fn new(config: GatewayConfig, http: Arc<TwilightHttpClient>) -> Self {
        // Broadcast channel: 4096 buffered events, aşırı birikme olursa en eski kaybolur
        // (Lagging receiver'lar için) — moka Faz 4'te persistent queue olur
        let (event_tx, _) = tokio::sync::broadcast::channel(4096);
        Self {
            config,
            event_tx,
            http,
            shard: None,
        }
    }
    
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<GatewayEvent> {
        self.event_tx.subscribe()
    }
    
    /// Twilight Shard'ı başlatır ve event stream'ini viscos-core broadcast channel'ına bağlar.
    pub async fn connect(&mut self) -> Result<()> {
        let mut presence = None;
        if let Some(p) = &self.config.presence {
            presence = Some(
                twilight_model::gateway::presence::PresenceUpdate::new(
                    p.activities.clone(),
                    p.status.clone(),
                    false,
                )?
            );
        }
        
        let config = Config::builder(
            self.config.token.clone(),
            self.config.intents,
        )
        .large_threshold(self.config.large_threshold)
        .presence(presence)
        .build();
        
        info!("Gateway connecting (twilight-gateway)");
        // twilight gateway'in Shard::new'ı bağlantıyı başlatmaz, EventStream poll edildiğinde başlar.
        let (shard, events) = Shard::with_config(ShardId::ONE, config);
        self.shard = Some(shard);
        
        // Event stream'i task olarak spawn et
        let event_tx = self.event_tx.clone();
        let mut stream = events;
        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(event) => {
                        if let Some(vc_event) = map_event(event) {
                            let _ = event_tx.send(vc_event);
                        }
                    }
                    Err(err) => {
                        error!(?err, "twilight gateway error");
                    }
                }
            }
            warn!("twilight event stream ended");
        });
        
        Ok(())
    }
    
    /// Shard'ı durdur (graceful shutdown için).
    pub async fn shutdown(&mut self) {
        if let Some(shard) = self.shard.take() {
            shard.shutdown();
        }
    }
}

/// twilight Event → viscos_core GatewayEvent adaptörü.
fn map_event(event: Event) -> Option<GatewayEvent> {
    match event {
        Event::Ready(ready) => Some(GatewayEvent::Ready(ready.boxed().into())),
        Event::MessageCreate(msg) => Some(GatewayEvent::MessageCreate((*msg).into())),
        Event::MessageUpdate(msg) => Some(GatewayEvent::MessageUpdate((*msg).into())),
        Event::MessageDelete(delete) => Some(GatewayEvent::MessageDelete(delete.into())),
        Event::GuildCreate(guild) => Some(GatewayEvent::GuildCreate(guild.into())),
        Event::TypingStart(typing) => Some(GatewayEvent::TypingStart(typing.into())),
        Event::PresenceUpdate(presence) => Some(GatewayEvent::PresenceUpdate(presence.into())),
        Event::VoiceStateUpdate(voice) => Some(GatewayEvent::VoiceStateUpdate(voice.into())),
        Event::Resumed => {
            info!("Gateway session resumed");
            None
        }
        Event::Reconnect => {
            warn!("Server requested reconnect");
            None
        }
        Event::GatewayHeartbeatAck => {
            debug!("heartbeat ACK");
            None
        }
        Event::GatewayHeartbeat(_) => None,
        _ => {
            // Unhandled event — viscos-core event'lerine eklenene kadar yutulur
            debug!(?event, "unhandled gateway event");
            None
        }
    }
}
```

**Ne kazandık:**
- ✅ zstd-stream framing doğru (mevcut plandaki `zstd_buffer.clear()` bug'ı çözüldü)
- ✅ Session resume built-in (Discord breaking change'lerde twilight upstream fix yapar)
- ✅ Reconnect + exponential backoff built-in
- ✅ Heartbeat jittering built-in
- ✅ Intent filter built-in
- ✅ `tokio-websockets` (Autobahn-strict, SIMD-accelerated) bedava
- ✅ `EventStream` doğrudan `futures::Stream` → ergonomik
- ~500 satır custom gateway kodu → ~50 satır adaptör

---

## 5. Intent'ler (twilight-model::Intents)

Discord 32 intent sunar (bitfield). `twilight_model::gateway::Intents` typed bitflags ile geliyor — manuel `1 << 9` yerine `Intent::GuildMessages` yazılır, derleyici yanlış bit'i yakalar.

| Intent | Tip | Kullanım |
|--------|-----|----------|
| GUILDS | `Intent::Guilds` | Sunucu bilgisi (üye listesi için) |
| GUILD_MEMBERS | `Intent::GuildMembers` | Üye event'leri (privileged) |
| GUILD_MESSAGES | `Intent::GuildMessages` | Mesaj event'leri |
| DIRECT_MESSAGES | `Intent::DirectMessages` | DM |
| MESSAGE_CONTENT | `Intent::MessageContent` | Mesaj içeriği (privileged) |
| GUILD_MESSAGE_TYPING | `Intent::GuildMessageTyping` | Typing indicator |
| PRESENCES | `Intent::Presences` | Çevrimiçi durumu (privileged) |

**Privileged intent'ler** Discord Developer Portal'da onay gerektirir.

**Viscos v1 default intents (privacy):**
```rust
use twilight_model::gateway::Intents;

const DEFAULT_INTENTS: Intents = 
    Intents::GUILDS
    | Intents::GUILD_MESSAGES
    | Intents::DIRECT_MESSAGES
    | Intents::MESSAGE_CONTENT    // privileged, gerekli
    | Intents::GUILD_MESSAGE_TYPING;
```

---

## 6. Event Types (viscos-core)

```rust
// crates/viscos-core/src/events.rs
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub enum GatewayEvent {
    Ready(ReadyData),
    MessageCreate(Message),
    MessageUpdate(Message),
    MessageDelete(MessageDelete),
    GuildCreate(Guild),
    TypingStart(TypingEvent),
    PresenceUpdate(Presence),
    VoiceStateUpdate(VoiceState),
    // ...
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReadyData {
    pub v: u8,
    pub user: User,
    pub guilds: Vec<UnavailableGuild>,
    pub session_id: String,
    pub resume_gateway_url: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    pub bot: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UnavailableGuild {
    pub id: String,
    pub unavailable: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub author: User,
    pub content: String,
    pub timestamp: String,
    pub edited_timestamp: Option<String>,
    // ...
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MessageDelete {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
}

// ... Guild, TypingEvent, Presence, VoiceState
```

---

## 7. Test Stratejisi (Faz 3.0)

| Test | Tip | Kabul |
|------|-----|-------|
| Hello parse | Unit | heartbeat_interval doğru |
| Identify serialize | Unit | JSON valid |
| Heartbeat timing | Integration (mock) | Interval doğru, jitter var |
| Resume payload | Unit | session_id + seq doğru |
| Reconnect backoff | Unit | Exponential artış, max aşılmaz |
| Event dispatch | Unit | Doğru enum variant |
| zstd decode | Integration | Binary frame → JSON |

**Mock server:** `tokio-tungstenite` + local server. CI'da tam gateway flow simülasyonu.

**Not:** Gerçek Discord Gateway'e CI'da bağlanma (hesap banı + değişken ortam).

---

## 8. Kabul Kriterleri (Definition of Done)

- [ ] WebSocket bağlantısı kuruluyor (gerçek Discord)
- [ ] Hello alınıyor, heartbeat interval doğru
- [ ] Identify gönderiliyor, Ready alınıyor
- [ ] Message Create event'i alınıyor
- [ ] Guild Create event'i alınıyor (sunucu listesi)
- [ ] Typing event'i alınıyor
- [ ] Heartbeat çalışıyor, ACK alınıyor
- [ ] Sunucu kapatılınca Reconnect tetikleniyor
- [ ] Resume payload doğru gönderiliyor
- [ ] Reconnect backoff exponential
- [ ] Intent filter çalışıyor (privileged intent olmadan bile bağlantı)
- [ ] `cargo clippy -- -D warnings` temiz
- [ ] Tüm gateway test'leri geçer (mock)
- [ ] 5 dakika canlı bağlantıda crash yok (lokal acceptance)

---

## 9. Karar Noktası (Faz 3.0 Sonu)

> 🔵 **İNSAN:** Hangi intent'ler varsayılan açık olsun?
> - Privacy-first: sadece mesaj + guild (privileged olmadan), üye listesi yok
> - Feature-rich: üye listesi + presence için privileged intent (Discord onayı gerekir)
> - Config-time: settings'ten aç/kapa
> - Trade-off: üye listesi feature'ı vs Discord application onay süreci

> 🔵 **İNSAN:** Reconnect backoff agresifliği?
> - Hızlı: 1s, 2s, 4s, 8s, 16s, 32s, 60s (max)
> - Yavaş: 5s, 10s, 30s, 60s (daha nazik)
> - Trade-off: reconnect hızı vs sunucu yükü

> 🔵 **İNSAN:** Session resume ne kadar agresif?
> - Her bağlantı kopmasında resume dene (10 deneme)
> - Sadece network glitch'te resume, geri kalanında fresh identify
> - Trade-off: hız vs correctness

---

## 10. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| zstd decode hatası | Bağlantı kopar | twilight-gateway production-tested zstd decoder (built-in, manual implementasyon yok) |
| Heartbeat kaçırılırsa | Server disconnect | twilight-gateway jittered heartbeat built-in |
| Sequence number kayıp | Event sırası bozulur | twilight-gateway internal `AtomicU64`; viscos-core event dispatch'te ekstra ordering guard (Faz 4 cache'inde) |
| Gateway version değişir | Kırılma | twilight-gateway v=10 sabit, breaking change'de upstream patch; biz sadece `cargo update` yaparız |
| Privileged intent reddedilir | Üye listesi yok | Privacy-first default, kullanıcı opt-in |
| Reconnect storm | Sunucu yükü | twilight-gateway exponential backoff + jitter built-in |
| twilight-gateway breaking change (0.17 → 0.18) | API drift | Pin `0.17.x`; major bump → 1 hafta geçiş + yeni ADR |

---

## 11. Çıkış → Faz 4.0

Bu faz tamamlandığında:
- Gerçek zamanlı mesaj event'leri akıyor
- Sunucu listesi, üye event'leri alınıyor
- Reconnect/resume çalışıyor
- zstd decompression sağlam

Faz 4.0 → SQLite + moka + foyer cache (event'leri persist et, mesaj geçmişini offline oku).
