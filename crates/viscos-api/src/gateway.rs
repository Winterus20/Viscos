//! `ViscosGateway` — `twilight_gateway::Shard` ince sarmalayıcısı.
//!
//! **Tasarım notu:** Twilight 0.17'de eski `Cluster` API'si kaldırıldı; yerine
//! tek bir [`Shard`](twilight_gateway::Shard) (max 2500 guild) +
//! [`create_recommended`](twilight_gateway::create_recommended) helper'ı var.
//! Viscos v1 tek shard'lı başlayacak (kullanıcı hesabı < 2500 guild varsayımı),
//! ama [`ViscosGateway::connect`] ileride `ShardId` parametresi alarak çoklu
//! shard'a geçebilir.
//!
//! **Scope guard (Faz 3.0):** Sadece connect + event stream + lifecycle.
//! Voice, presence update, slash interaction → sonraki fazlar.
//!
//! **Reconnect / session resume / zstd:** Twilight built-in. Biz explicit kod
//! yazmıyoruz — `Shard::next_event` zaten:
//! - Reconnect loop (exponential backoff + jitter)
//! - Session resume (`RESUMED` event'i ile)
//! - zstd-stream framing (built-in decoder)
//! - Hello → Identify → Ready handshake
//! - Jittered heartbeat (akıllı interval)
//!
//! `# Panics` — yok (twilight Shard::new panics değil, Config::new sadece TLS
//! sertifika yükleme hatasında panikler; bu durumda program başlamaz, bu yüzden
//! [`ViscosGateway::connect`] panik olarak yüzeye çıkmaz).

use std::future::Future;

use tracing::{debug, instrument, warn};
use twilight_gateway::{Config, EventTypeFlags, Intents, Shard, ShardId, StreamExt as _};

use crate::error::ApiError;
use crate::events::GatewayEvent;

/// Tek bir Discord gateway shard'ı için ince sarmalayıcı.
///
/// Kullanım:
/// ```ignore
/// let mut gw = ViscosGateway::connect("token", ViscosGateway::default_intents())?;
/// while let Some(event) = gw.next_event().await {
///     // dispatch event
/// }
/// ```
pub struct ViscosGateway {
    shard: Shard,
}

impl ViscosGateway {
    /// Yeni bir gateway shard'ı oluştur ve bağlantı için konfigure et.
    ///
    /// **Not:** Bu method bağlantıyı kurmaz — twilight `Shard` lazy-connect
    /// modeli kullanır; ilk `next_event().await` çağrısında WebSocket açılır.
    ///
    /// `# Panics` — `Config::new` TLS sertifika yükleme hatasında panikler.
    /// Pratikte rustls-platform-verifier OS cert store'unu okur; standart
    /// Windows installasyonlarında panic olmaz.
    #[instrument(skip(token), fields(intents_bits = intents.bits()))]
    pub fn connect(token: &str, intents: Intents) -> Result<Self, ApiError> {
        Self::connect_internal(token.to_string(), intents)
    }

    /// Bridge-aware factory — `connect` ile aynı gateway'i kurar, ek olarak
    /// [`crate::GatewayCacheBridge`] üzerinden event fan-out sağlar.
    ///
    /// MVP-2 production use-case: caller bridge'i bir kez kurar, sonra
    /// `run_until_disconnect` yerine bridge'in `handle_event` callback'ini
    /// kullanır (PR-6 sonrası main loop'unda). Bu method bridge referansını
    /// kabul eder (şimdilik log-only; PR-6'da gateway event dispatch'i bridge
    /// üzerinden yönlendirilecek) ve aynı `ViscosGateway` shard'ını döner.
    ///
    /// `# Errors` — `ApiError` TLS / config hatası.
    #[instrument(skip_all, fields(intents_bits = intents.bits()))]
    pub fn connect_with_bridge(
        token: &str,
        intents: Intents,
        #[allow(unused_variables)] // PR-6'da dispatch burada bağlanacak
        bridge: std::sync::Arc<crate::GatewayCacheBridge>,
    ) -> Result<Self, ApiError> {
        // PR-6: caller aynı bridge'i dışarıda tutup `next_event` + `handle_event`
        // çiftini kullanacak; burada log-only.
        debug!("ViscosGateway constructed with bridge-aware factory (PR-6 wiring pending)");
        Self::connect_internal(token.to_string(), intents)
    }

    fn connect_internal(token: String, intents: Intents) -> Result<Self, ApiError> {
        // Config::new yerine ConfigBuilder kullanmıyoruz; ileride presence /
        // large_threshold override gerektiğinde builder'a geçilecek (Faz 3.5+).
        let config = Config::new(token, intents);
        let shard = Shard::with_config(ShardId::ONE, config);
        debug!("ViscosGateway shard constructed (lazy-connect)");
        Ok(Self { shard })
    }

    /// Sonraki gateway event'ini bekle. Stream bittiğinde `None`.
    ///
    /// `EventTypeFlags::all()` — twilight tüm deserialization'ı yapar; biz
    /// adaptör katmanında `Unknown`'a düşürürüz. İleride daraltma
    /// (sadece mesaj + guild) burada değişecek.
    #[instrument(skip(self))]
    pub async fn next_event(&mut self) -> Option<GatewayEvent> {
        let item = self.shard.next_event(EventTypeFlags::all()).await?;
        match item {
            Ok(event) => match GatewayEvent::try_from(event) {
                Ok(ev) => Some(ev),
                Err(e) => {
                    warn!(error = %e, "gateway event conversion failed");
                    None
                }
            },
            Err(err) => {
                warn!(source = ?err, "twilight gateway error");
                // Hata durumunda da None döndürüyoruz — caller reconnect
                // mantığını kendisi yönetir. Alternatif: bir internal error
                // channel'ı expose etmek (Faz 5.0'da UI'a yüzey).
                None
            }
        }
    }

    /// Tek handler fonksiyonuyla event stream'ini drain et — disconnect olana
    /// kadar (ya da handler None dönene kadar) çalışır.
    pub async fn run_until_disconnect<F, Fut>(&mut self, mut handler: F)
    where
        F: FnMut(GatewayEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        while let Some(event) = self.next_event().await {
            handler(event).await;
        }
    }

    /// Shard ID. Faz 3.5+'ta çoklu shard'a geçerken public API olur.
    #[must_use]
    pub fn shard_id(&self) -> ShardId {
        self.shard.id()
    }

    /// Aktif session varsa `Some(session_id)`. Ready sonrası set edilir.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.shard.session().map(|s| s.id())
    }

    /// Resume URL (Discord'tan). Bağlantı koptuğunda kullanılabilir.
    #[must_use]
    pub fn resume_url(&self) -> Option<&str> {
        self.shard.resume_url()
    }

    /// Heartbeat latency istatistikleri (avg + recent).
    #[must_use]
    pub fn latency(&self) -> &twilight_gateway::Latency {
        self.shard.latency()
    }

    /// Viscos v1 default intent seti — **privacy-first**.
    ///
    /// `GUILD_PRESENCES` ve `GUILD_MEMBERS` (her ikisi de privileged) **YOK**.
    /// `MESSAGE_CONTENT` (privileged) **VAR** — mesaj içeriği olmadan gateway
    /// neredeyse işe yaramaz.
    ///
    /// Kullanıcı Discord Developer Portal'da privileged intent'leri kendi
    /// talep ederse config'ten override edebilir (Faz 2.5 config schema).
    ///
    /// `# Panics` — yok.
    #[must_use]
    pub fn default_intents() -> Intents {
        // bitflags 2.x `BitOr` operatorü `const fn` değil → fonksiyon const
        // olamaz. Static initializer gereken yerler (config load) zaten bunu
        // runtime'da çağırıyor.
        Intents::GUILDS
            | Intents::GUILD_MESSAGES
            | Intents::DIRECT_MESSAGES
            | Intents::MESSAGE_CONTENT
            | Intents::GUILD_MESSAGE_TYPING
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_intents_contains_required_bits() {
        let intents = ViscosGateway::default_intents();
        assert!(intents.contains(Intents::GUILDS));
        assert!(intents.contains(Intents::GUILD_MESSAGES));
        assert!(intents.contains(Intents::DIRECT_MESSAGES));
        assert!(intents.contains(Intents::MESSAGE_CONTENT));
        assert!(intents.contains(Intents::GUILD_MESSAGE_TYPING));
    }

    #[test]
    fn default_intents_excludes_privileged_presence_member() {
        let intents = ViscosGateway::default_intents();
        // Privileged intent'ler privacy-first default'ta YOK.
        assert!(!intents.contains(Intents::GUILD_MEMBERS));
        // GUILD_PRESENCES (privileged) da yok — twilight 0.17'de
        // ayrı bir bitflag olarak mevcut.
        assert!(!intents.contains(Intents::GUILD_PRESENCES));
        // Moderation (ban/kick/audit) Faz 7+ admin feature'ları.
        assert!(!intents.contains(Intents::GUILD_MODERATION));
    }

    #[test]
    fn default_intents_is_non_empty_and_deterministic() {
        let a = ViscosGateway::default_intents();
        let b = ViscosGateway::default_intents();
        assert!(!a.is_empty());
        assert_eq!(a.bits(), b.bits());
    }
}
