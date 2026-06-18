//! `GatewayEvent` — twilight `Event` → viscos-owned event adaptörü.
//!
//! **Neden ayrı bir enum:** Twilight'ın `Event` enum'ı 70+ varyant içerir ve
//! Box'lı payload'lar taşır (`Box<MessageCreate>`). Viscos'un ihtiyacı olan
//! alt küme (mesaj, guild, typing, reaksiyon, ready, lifecycle). Bu adaptör:
//!
//! 1. Viscos'un dış yüzeyine bağımlılığı daraltır (twilight-major bump daha
//!    güvenli olur — sadece bu dosyada değişiklik gerekir).
//! 2. Box indirection'ı kaldırır — IPC katmanı (Faz 3.5+) flat `Message` payload
//!    ile çalışır.
//! 3. `non_exhaustive` sayesinde dış tüketiciler exhaustive match yapamaz; yeni
//!    varyant eklemek non-breaking.
//!
//! **Tasarım:** Payload alanları twilight typed struct'larıdır (clonable).
//! Faz 4'te kendi domain tipimize (`viscos-core::Message`) çevirirken yine bu
//! enum üzerinden geçeceğiz — şimdilik ileri dönüşüm noktası.

use twilight_gateway::Event as TwEvent;
use twilight_model::gateway::event::EventType;
use twilight_model::gateway::payload::incoming::{
    GuildCreate, MessageCreate, MessageDelete, MessageUpdate, ReactionAdd, Ready, TypingStart,
};

use crate::error::ApiError;

/// Lifecycle event — connection state machine'ini yansıtır.
///
/// Twilight'ın ham `Event::GatewayClose(_)`, `Event::Resumed`,
/// `Event::GatewayReconnect` varyantları burada anlamlı enum'lara dönüşür.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LifecycleEvent {
    /// `Event::GatewayClose` — WebSocket kapandı (graceful veya hata).
    GatewayClose {
        code: Option<u16>,
        reason: Option<String>,
    },
    /// `Event::Resumed` — başarılı session resume.
    Resumed,
    /// `Event::GatewayReconnect` — Discord reconnect istedi.
    Reconnect,
    /// `Event::GatewayInvalidateSession(resumable)` — session invalidated.
    SessionInvalidated { resumable: bool },
    /// `Event::GatewayHeartbeatAck` — heartbeat ACK (opsiyonel monitoring).
    HeartbeatAck,
    /// `Event::GatewayHello` — Hello alındı (heartbeat interval).
    Hello { heartbeat_interval_ms: u64 },
}

/// Viscos-owned gateway event.
///
/// `non_exhaustive` — dış tüketiciler `_ =>` dalı bulundurmak zorunda.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum GatewayEvent {
    /// Hello alındı — `Event::GatewayHello`. Discord handshake başlangıcı.
    Hello { heartbeat_interval_ms: u64 },
    /// Ready — `Event::Ready(Ready)`. Session kuruldu.
    Ready(Box<Ready>),
    /// `Event::Resumed` — başarılı session resume.
    Resumed,
    /// `Event::GatewayReconnect` — Discord reconnect istedi.
    Reconnect,
    /// `Event::GatewayInvalidateSession(bool)` — session geçersiz kılındı.
    SessionInvalidated { resumable: bool },
    /// `Event::GatewayHeartbeatAck` — heartbeat ACK.
    HeartbeatAck,
    /// `Event::GatewayClose` — WebSocket kapandı.
    GatewayClose {
        code: Option<u16>,
        reason: Option<String>,
    },
    /// `Event::MessageCreate(Box<MessageCreate>)`.
    MessageCreate(Box<MessageCreate>),
    /// `Event::MessageUpdate(Box<MessageUpdate>)`.
    MessageUpdate(Box<MessageUpdate>),
    /// `Event::MessageDelete(MessageDelete)` — Box indirection yok (küçük payload).
    MessageDelete(MessageDelete),
    /// `Event::GuildCreate(Box<GuildCreate>)` — büyük payload (üyelerle).
    GuildCreate(Box<GuildCreate>),
    /// `Event::ReactionAdd(Box<ReactionAdd>)`.
    ReactionAdd(Box<ReactionAdd>),
    /// `Event::TypingStart(Box<TypingStart>)`.
    TypingStart(Box<TypingStart>),
    /// Tanınmayan event — twilight yeni bir varyant eklediğinde buraya düşer.
    /// İleri sürümlerde yeni varyant eklenene kadar bilinçli olarak yutulur.
    Unknown(EventType),
}

impl GatewayEvent {
    /// Lifecycle event'lere ayrıştır. Diğerleri `None`.
    #[must_use]
    pub fn as_lifecycle(&self) -> Option<LifecycleEvent> {
        match self {
            Self::GatewayClose { code, reason } => Some(LifecycleEvent::GatewayClose {
                code: *code,
                reason: reason.clone(),
            }),
            Self::Resumed => Some(LifecycleEvent::Resumed),
            Self::Reconnect => Some(LifecycleEvent::Reconnect),
            Self::SessionInvalidated { resumable } => Some(LifecycleEvent::SessionInvalidated {
                resumable: *resumable,
            }),
            Self::HeartbeatAck => Some(LifecycleEvent::HeartbeatAck),
            Self::Hello {
                heartbeat_interval_ms,
            } => Some(LifecycleEvent::Hello {
                heartbeat_interval_ms: *heartbeat_interval_ms,
            }),
            _ => None,
        }
    }
}

/// `twilight_gateway::Event` → `viscos_api::GatewayEvent` adaptörü.
///
/// `# Errors` — şu an `TryFrom` infallible (bilinmeyen varyantlar `Unknown`'a
/// düşüyor), ama `ApiError` döndürme contract'ı API kararlılığı için korunuyor:
/// ileride gateway v=11+ parse hataları buraya eklenecek.
impl TryFrom<TwEvent> for GatewayEvent {
    type Error = ApiError;

    #[allow(clippy::too_many_lines)]
    fn try_from(event: TwEvent) -> Result<Self, Self::Error> {
        Ok(match event {
            TwEvent::GatewayHello(hello) => Self::Hello {
                heartbeat_interval_ms: hello.heartbeat_interval,
            },
            TwEvent::Ready(ready) => Self::Ready(Box::new(ready)),
            TwEvent::Resumed => Self::Resumed,
            TwEvent::GatewayReconnect => Self::Reconnect,
            TwEvent::GatewayInvalidateSession(resumable) => Self::SessionInvalidated { resumable },
            TwEvent::GatewayHeartbeatAck => Self::HeartbeatAck,
            TwEvent::GatewayClose(frame) => {
                let (code, reason) = frame
                    .map(|f| (Some(f.code), Some(f.reason.to_string())))
                    .unwrap_or((None, None));
                Self::GatewayClose { code, reason }
            }
            TwEvent::MessageCreate(m) => Self::MessageCreate(m),
            TwEvent::MessageUpdate(m) => Self::MessageUpdate(m),
            TwEvent::MessageDelete(m) => Self::MessageDelete(m),
            TwEvent::GuildCreate(g) => Self::GuildCreate(g),
            TwEvent::ReactionAdd(r) => Self::ReactionAdd(r),
            TwEvent::TypingStart(t) => Self::TypingStart(t),
            other => Self::Unknown(other.kind()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_bridge_for_known_lifecycle_events() {
        let ev = GatewayEvent::Resumed;
        assert_eq!(ev.as_lifecycle(), Some(LifecycleEvent::Resumed));

        let ev = GatewayEvent::Reconnect;
        assert_eq!(ev.as_lifecycle(), Some(LifecycleEvent::Reconnect));

        let ev = GatewayEvent::HeartbeatAck;
        assert_eq!(ev.as_lifecycle(), Some(LifecycleEvent::HeartbeatAck));

        let ev = GatewayEvent::Hello {
            heartbeat_interval_ms: 41_250,
        };
        assert_eq!(
            ev.as_lifecycle(),
            Some(LifecycleEvent::Hello {
                heartbeat_interval_ms: 41_250
            })
        );
    }

    #[test]
    fn lifecycle_bridge_is_none_for_payload_events() {
        let ev = GatewayEvent::MessageDelete(MessageDelete {
            channel_id: twilight_model::id::Id::new(1),
            guild_id: None,
            id: twilight_model::id::Id::new(2),
        });
        assert!(ev.as_lifecycle().is_none());
    }
}
