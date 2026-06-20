//! `GatewayCacheBridge` — Gateway → Cache + IPC fan-out (Faz 3.0 MVP-2).
//!
//! **Kontrat:** Discord Gateway'den gelen [`GatewayEvent`]'leri
//! 1. [`viscos_cache`] üzerinde kalıcı hale getirir (SQLite WAL + moka RAM).
//! 2. [`viscos_ipc`] üzerinden frontend'e küçük push olayları yayar.
//!
//! Bu köprü, MVP-2'nin (Time-to-Read/Write) kalbi: gateway event'leri
//! cache'e yazılmadan UI'ya iletilirse her kanal değişiminde REST round-trip
//! gerekir; cache'e yazıldığında ilk kanal açılışı <500ms'de döner
//! (audit §2.4 + §2.10).
//!
//! **Tasarım notları:**
//!
//! - **Pull-based IPC default'u bozulmaz:** Bridge sadece küçük push olayları
//!   (`MessageCreated`, `MessageEdited`, `LoginSuccess`) yayar. Tam message
//!   payload frontend tarafından [`viscos_ipc::IpcCommand::GetMessages`]
//!   ile çekilir (ADR-0012 §3).
//! - **`Arc<Db>` + `Arc<MessageCache>`:** Bridge cache'i sahiplenmez, paylaşır.
//!   Aynı cache'i REST handler'lar ve IPC komutları da kullanır (PR-6 sonrası).
//! - **`UnboundedSender<IpcEvent>`:** Frontend bağlı değilse event'ler yutulur
//!   (`ChannelClosed` warning). Backpressure uygulanmaz — channel drop'unda
//!   event kaybı kabul edilebilir (state zaten cache'e yazılmış).
//!
//! **Scope guard:** Bridge sadece MVP-2 kapsamındaki event'leri handle eder
//! (READY, GUILD_CREATE, MESSAGE_CREATE, MESSAGE_UPDATE).
//! Voice, presence, slash interaction, thread events → sonraki fazlar.
//!
//! **MVP-2 testing:** Test'lerde twilight payload'ları `serde_json` ile
//! deserialize edilir (audit'in `events_try_from.rs` pattern'i). Gerçek
//! Discord bağlantısı yoktur; sadece köprü davranışı doğrulanır.

use std::sync::Arc;

use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, instrument, warn};
use twilight_model::channel::Channel as TwChannel;
use twilight_model::guild::Guild as TwGuild;
use twilight_model::user::CurrentUser;
use viscos_cache::cache::Message;
use viscos_cache::{Db, MessageCache};
use viscos_error::ViscosError;
use viscos_ipc::{IpcEvent, IpcEventError};

use crate::error::ApiError;
use crate::events::GatewayEvent;

/// Köprü katmanı hata modeli.
///
/// `#[non_exhaustive]` — yeni varyant eklemek non-breaking. Tüketici taraf
/// `_ =>` kolu ile yakalanmalı.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BridgeError {
    /// `viscos_cache` alt sistem hatası (SQLite, moka, vb.).
    #[error("cache error: {0}")]
    Cache(#[from] viscos_cache::CacheError),

    /// `viscos_ipc` event yayılım hatası (serialize, channel closed).
    #[error("IPC event error: {0}")]
    Ipc(#[from] IpcEventError),

    /// `viscos_api` katmanı hatası (twilight deserialization vb.).
    #[error("api error: {0}")]
    Api(#[from] ApiError),

    /// Twilight payload parse / serialize hatası.
    #[error("twilight payload error: {0}")]
    Twilight(String),

    /// Alt sistem hatası (viscos-error kök hata tipi).
    #[error("viscos error: {0}")]
    Viscos(#[from] ViscosError),
}

impl From<serde_json::Error> for BridgeError {
    fn from(err: serde_json::Error) -> Self {
        Self::Twilight(err.to_string())
    }
}

impl From<rusqlite::Error> for BridgeError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Cache(viscos_cache::CacheError::Sqlite(err))
    }
}

/// `BridgeError` → `ViscosError` adaptörü (üst katman uyumluluğu).
impl From<BridgeError> for ViscosError {
    fn from(err: BridgeError) -> Self {
        match err {
            BridgeError::Viscos(inner) => inner,
            // Diğer tüm varyantlar context ile sarılır (kaynak bilgisi korunur).
            other => ViscosError::Media(other.to_string()),
        }
    }
}

/// Gateway → Cache + IPC event fan-out köprüsü.
///
/// `Arc<Db>` + `Arc<MessageCache>` + IPC event sender'ı tutar. Event'leri
/// dispatch eder; her event tipi kendi handler'ına düşer (aşağıdaki
/// `on_*` method'ları).
///
/// **Thread safety:** `Send + Sync` — `Arc` field'lar ve `UnboundedSender`
/// zaten thread-safe; mutable state yok.
///
/// **Lifetime:** `Arc` referanslarıyla sınırlı; bridge'in kendisi
/// `Clone` implement etmez (sender clone edilebilir, ama state paylaşımı
/// için bridge'i `Arc<GatewayCacheBridge>` ile sarmalayın).
pub struct GatewayCacheBridge {
    db: Arc<Db>,
    message_cache: Arc<MessageCache>,
    ipc_tx: UnboundedSender<IpcEvent>,
}

impl std::fmt::Debug for GatewayCacheBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayCacheBridge")
            .field("db", &"Arc<Db>")
            .field("message_cache", &"Arc<MessageCache>")
            .field("ipc_tx", &"UnboundedSender<IpcEvent>")
            .finish()
    }
}

impl GatewayCacheBridge {
    /// Yeni bir bridge instance oluştur.
    ///
    /// # Arguments
    ///
    /// * `db` — paylaşılan SQLite WAL bağlantı havuzu (guild, channel,
    ///   message metadata).
    /// * `message_cache` — paylaşılan moka message cache (sıcak mesaj lookup).
    /// * `ipc_tx` — frontend event kanalı. Drop edilirse `ChannelClosed`
    ///   warning loglanır; event'ler yutulur (state yine de cache'e yazılır).
    #[must_use]
    pub fn new(
        db: Arc<Db>,
        message_cache: Arc<MessageCache>,
        ipc_tx: UnboundedSender<IpcEvent>,
    ) -> Self {
        Self {
            db,
            message_cache,
            ipc_tx,
        }
    }

    /// `Arc<Db>` clone (multi-handler use-case).
    #[must_use]
    pub fn db(&self) -> Arc<Db> {
        Arc::clone(&self.db)
    }

    /// `Arc<MessageCache>` clone.
    #[must_use]
    pub fn message_cache(&self) -> Arc<MessageCache> {
        Arc::clone(&self.message_cache)
    }

    /// `UnboundedSender` clone (örn. ikinci bir task'a fan-out).
    #[must_use]
    pub fn ipc_sender(&self) -> UnboundedSender<IpcEvent> {
        self.ipc_tx.clone()
    }

    /// [`GatewayEvent`] → cache write + IPC push dispatch.
    ///
    /// Bilinmeyen event varyantları (`Unknown`, lifecycle event'ler) NoOp.
    /// Hata durumunda `BridgeError` döner; call-site log + reconnect.
    #[instrument(skip(self), fields(event = ?event_label(&event)))]
    pub async fn handle_event(&self, event: GatewayEvent) -> Result<(), BridgeError> {
        match event {
            GatewayEvent::MessageCreate(m) => self.on_message_create(m.0).await,
            GatewayEvent::MessageUpdate(m) => self.on_message_update(m.0).await,
            GatewayEvent::GuildCreate(g) => self.on_guild_create_enum(*g).await,
            GatewayEvent::Ready(ready) => self.on_ready(&ready.user, &ready.guilds).await,
            // Diğer varyantlar (Resumed, Reconnect, ReactionAdd, TypingStart, …)
            // MVP-2 kapsamı dışında; no-op log.
            _ => {
                debug!("handle_event: no-op for non-mvp2 event");
                Ok(())
            }
        }
    }

    /// `GUILD_CREATE` dispatch — `GuildCreate` enum (`Available(Guild)` veya
    /// `Unavailable(UnavailableGuild)`). Sadece `Available` cache'e yazılır.
    async fn on_guild_create_enum(
        &self,
        payload: twilight_model::gateway::payload::incoming::GuildCreate,
    ) -> Result<(), BridgeError> {
        match payload {
            twilight_model::gateway::payload::incoming::GuildCreate::Available(guild) => {
                self.on_guild_create(guild).await
            }
            twilight_model::gateway::payload::incoming::GuildCreate::Unavailable(unavailable) => {
                self.on_guild_unavailable(&unavailable).await
            }
        }
    }

    /// `GUILD_CREATE` Unavailable handler — sadece guild id placeholder row.
    async fn on_guild_unavailable(
        &self,
        unavailable: &twilight_model::guild::UnavailableGuild,
    ) -> Result<(), BridgeError> {
        let conn = self.db.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO guilds (id, name, owner_id, icon_hash, raw)
             VALUES (?1, ?2, NULL, NULL, ?3)",
            rusqlite::params![unavailable.id.get(), "", "{}"],
        )?;
        debug!("on_guild_unavailable: placeholder row yazıldı");
        Ok(())
    }

    /// `MESSAGE_CREATE` handler — mesajı cache'e yaz ve frontend'e push.
    ///
    /// Twilight `MessageCreate(Box<Message>)` payload'ından channel_id,
    /// id, content, author_id çıkarılır; hem `MessageCache`'e (moka, sıcak
    /// lookup) hem `Db`'ye (SQLite WAL, kalıcı) yazılır.
    #[instrument(skip(self, message), fields(message_id = message.id.get(), channel_id = message.channel_id.get()))]
    pub async fn on_message_create(
        &self,
        message: twilight_model::channel::Message,
    ) -> Result<(), BridgeError> {
        let cache_message = Message::new(
            message.id.get(),
            message.channel_id.get(),
            message.content.clone(),
        );

        // 1) moka RAM cache — sıcak kanal scroll pattern'i.
        self.message_cache
            .put(message.id.get(), cache_message)
            .await?;

        // 2) SQLite WAL — kalıcı metadata. `raw` kolonu NOT NULL; twilight
        // payload JSON'unu saklıyoruz (Faz 4'te indexed query için source of
        // truth).
        let conn = self.db.conn()?;
        let raw = serde_json::to_string(&message)?;
        // `timestamp` RFC3339 string; şimdilik 0 (epoch) — PR-6'da
        // `parse_rfc3339` helper'ı eklenecek.
        conn.execute(
            "INSERT OR REPLACE INTO messages (id, channel_id, author_id, content, timestamp, raw)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                message.id.get().to_string(),
                message.channel_id.get(),
                message.author.id.get(),
                message.content,
                0i64,
                raw,
            ],
        )?;

        // 3) IPC push — frontend badge / scroll anchor güncellemesi.
        self.push_event(IpcEvent::MessageCreated {
            channel_id: message.channel_id.get(),
            message_id: message.id.get(),
        })?;

        debug!("on_message_create: cache + ipc push ok");
        Ok(())
    }

    /// `MESSAGE_UPDATE` handler — düzenlenen mesajı cache'te güncelle.
    #[instrument(skip(self, message), fields(message_id = message.id.get(), channel_id = message.channel_id.get()))]
    pub async fn on_message_update(
        &self,
        message: twilight_model::channel::Message,
    ) -> Result<(), BridgeError> {
        let cache_message = Message::new(
            message.id.get(),
            message.channel_id.get(),
            message.content.clone(),
        );

        self.message_cache
            .put(message.id.get(), cache_message)
            .await?;

        let conn = self.db.conn()?;
        conn.execute(
            "UPDATE messages SET content = ?1 WHERE id = ?2",
            rusqlite::params![message.content, message.id.get().to_string()],
        )?;

        self.push_event(IpcEvent::MessageEdited {
            channel_id: message.channel_id.get(),
            message_id: message.id.get(),
        })?;

        debug!("on_message_update: cache + ipc push ok");
        Ok(())
    }

    /// `GUILD_CREATE` handler — guild + ilgili kanalları cache'e yaz.
    ///
    /// Twilight 0.17'de `Guild` direkt typed struct (önceki sürümlerde
    /// `Available | Unavailable` enum'dı). `channels` field'ı guild payload'ı
    /// içinde gömülü.
    #[instrument(skip(self, guild), fields(guild_id = guild.id.get(), channel_count = guild.channels.len()))]
    pub async fn on_guild_create(&self, guild: TwGuild) -> Result<(), BridgeError> {
        let conn = self.db.conn()?;
        let raw = serde_json::to_string(&guild)?;
        conn.execute(
            "INSERT OR REPLACE INTO guilds (id, name, owner_id, icon_hash, raw)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                guild.id.get(),
                guild.name,
                guild.owner_id.get(),
                guild.icon.as_ref().map(|h| h.to_string()),
                raw,
            ],
        )?;

        for channel in &guild.channels {
            self.upsert_channel_inner(&conn, channel)?;
        }

        debug!("on_guild_create: guild + channels written to cache");
        Ok(())
    }

    /// `CHANNEL_CREATE` handler — tek kanalı cache'e yaz.
    ///
    /// Faz 3.0'da channel event'leri `GUILD_CREATE` payload'ı içinde gelir;
    /// bu method ayrı `CHANNEL_CREATE` varyantı eklendiğinde dispatcher'dan
    /// çağrılır (Faz 3.5+).
    #[instrument(skip(self, channel), fields(channel_id = channel.id.get(), guild_id = ?channel.guild_id.map(|g| g.get())))]
    pub async fn on_channel_create(&self, channel: TwChannel) -> Result<(), BridgeError> {
        let conn = self.db.conn()?;
        self.upsert_channel_inner(&conn, &channel)?;
        debug!("on_channel_create: channel written to cache");
        Ok(())
    }

    /// `READY` handler — current user + initial guild snapshot.
    ///
    /// `Ready` payload'ı `user` (`CurrentUser`) ve `guilds`
    /// (`Vec<UnavailableGuild>`) içerir. User bilgisi cache'e sığmaz (Faz 1+);
    /// guild'ler placeholder row olarak yazılır (UI'ın "loading..." state'i
    /// için).
    #[instrument(skip(self, user, guilds), fields(user_id = user.id.get(), guild_count = guilds.len()))]
    pub async fn on_ready(
        &self,
        user: &CurrentUser,
        guilds: &[twilight_model::guild::UnavailableGuild],
    ) -> Result<(), BridgeError> {
        let conn = self.db.conn()?;
        for g in guilds {
            conn.execute(
                "INSERT OR REPLACE INTO guilds (id, name, owner_id, icon_hash, raw)
                 VALUES (?1, ?2, NULL, NULL, ?3)",
                rusqlite::params![g.id.get(), "", "{}"],
            )?;
        }

        self.push_event(IpcEvent::LoginSuccess {
            user_id: user.id.get(),
        })?;

        debug!("on_ready: user + initial guild snapshot processed");
        Ok(())
    }

    /// Internal: `TwChannel` → SQLite `channels` row.
    ///
    /// Twilight 0.17'de `Channel` flat struct. `kind: ChannelType`,
    /// `guild_id: Option<Id<GuildMarker>>`, `name: Option<String>`.
    fn upsert_channel_inner(
        &self,
        conn: &rusqlite::Connection,
        channel: &TwChannel,
    ) -> Result<(), BridgeError> {
        let guild_id = channel.guild_id.map_or(0, twilight_model::id::Id::get);
        let channel_type = u8::from(channel.kind) as i64;
        let name = channel.name.clone();
        let raw = serde_json::to_string(channel)?;
        conn.execute(
            "INSERT OR REPLACE INTO channels (id, guild_id, name, kind, raw)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![channel.id.get(), guild_id, name, channel_type, raw],
        )?;
        Ok(())
    }

    /// Internal: `IpcEvent` push (sender hata toleransı).
    fn push_event(&self, event: IpcEvent) -> Result<(), BridgeError> {
        if let Err(e) = self.ipc_tx.send(event) {
            warn!(error = %e, "IPC event send failed (channel closed?)");
            return Err(IpcEventError::ChannelClosed.into());
        }
        Ok(())
    }
}

/// `GatewayEvent` etiketleme (debug log için; pattern match yerine hızlı isim).
fn event_label(event: &GatewayEvent) -> &'static str {
    match event {
        GatewayEvent::Hello { .. } => "Hello",
        GatewayEvent::Ready(_) => "Ready",
        GatewayEvent::Resumed => "Resumed",
        GatewayEvent::Reconnect => "Reconnect",
        GatewayEvent::SessionInvalidated { .. } => "SessionInvalidated",
        GatewayEvent::HeartbeatAck => "HeartbeatAck",
        GatewayEvent::GatewayClose { .. } => "GatewayClose",
        GatewayEvent::MessageCreate(_) => "MessageCreate",
        GatewayEvent::MessageUpdate(_) => "MessageUpdate",
        GatewayEvent::MessageDelete(_) => "MessageDelete",
        GatewayEvent::GuildCreate(_) => "GuildCreate",
        GatewayEvent::ReactionAdd(_) => "ReactionAdd",
        GatewayEvent::TypingStart(_) => "TypingStart",
        GatewayEvent::Unknown(_) => "Unknown",
    }
}

#[cfg(test)]
mod tests;
