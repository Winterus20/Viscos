//! Discord REST adaptörü — `twilight_http::Client`'i saran ince katman.
//!
//! **Tasarım notu:** Twilight zaten production-tested bir REST istemcisi —
//! rate-limit (`X-RateLimit-*`, global 429), brotli/gzip decompression, SIMD
//! JSON parsing, rustls TLS dahil. Viscos'un eklediği sadece:
//!
//! 1. `SecretString` ile sarılmış token (memory dump baseline savunma).
//! 2. twilight `Error` → `ApiError` typed adaptörü (kütüphane boundary).
//! 3. 401 alındığında `viscos-auth` ile koordinasyon hook'u (Faz 2.x, şimdi stub).
//!
//! **Scope guard:** Faz 2.0'da sadece core endpoint'ler — auth, mesaj, kanal,
//! guild, typing. Voice, interactions, slash commands, gateway event'leri →
//! sonraki fazlar.
//!
//! **Notlar:**
//! - Twilight 0.17 user-token DM listeleme endpoint'ini (`GET /users/@me/channels`)
//!   tip-safe wrapper olarak **sağlamıyor** (bot-token varsayımı). DM listesi
//!   Faz 3.0 gateway event'lerinde `CHANNEL_CREATE` takibiyle dolaylı olarak
//!   handle edilecek.
//! - Reaksiyon endpoint'leri Faz 2.0 follow-up: twilight 0.17'de
//!   `RequestReactionType` API refactor edildi, tam bağlantı Faz 2.0+ takip iş'inde.

use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use tracing::{debug, instrument, warn};
use twilight_http::Client as TwilightClient;
use twilight_model::{
    channel::{Channel, Message},
    id::{
        Id,
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
    },
    user::{CurrentUser, CurrentUserGuild},
};

// `rustls` crypto provider'ı test/build sırasında kurulmalı (sadece tip import).
#[cfg(test)]
use rustls;

use crate::error::ApiError;

/// Default REST timeout (twilight default'u 10s; 30s daha güvenli mobile network için).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Viscos REST istemcisi. `twilight_http::Client`'in ince sarmalayıcısı.
///
/// `SecretString` ile sarılmış token → drop anında zeroize.
///
/// **Kullanım:** [`ViscosHttp::new`] ile bir kere kur, sonra `Arc<ViscosHttp>`
/// ile birden çok task'ta paylaş.
pub struct ViscosHttp {
    client: TwilightClient,
    token: SecretString,
}

/// Builder pattern — twilight'ın `ClientBuilder`'ını extend eder.
///
/// Faz 2.0'da sadece `token` + `timeout` expose ediliyor. Faz 4'te
/// `proxy_url`, `default_allowed_mentions` eklenebilir.
pub struct ViscosHttpBuilder {
    token: SecretString,
    timeout: Duration,
}

impl ViscosHttpBuilder {
    /// Yeni builder oluştur (token zorunlu).
    pub fn new(token: SecretString) -> Self {
        Self {
            token,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Request timeout override.
    #[must_use]
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Builder'ı consume edip `ViscosHttp` üret.
    #[instrument(skip(self), fields(timeout_secs = self.timeout.as_secs()))]
    pub fn build(self) -> Result<ViscosHttp, ApiError> {
        let client = TwilightClient::builder()
            .token(self.token.expose_secret().to_string())
            .timeout(self.timeout)
            .build();

        debug!("viscos-http client built");
        Ok(ViscosHttp {
            client,
            token: self.token,
        })
    }
}

impl ViscosHttp {
    /// Builder üzerinden explicit konfigürasyonla kurulum.
    pub fn builder(token: SecretString) -> ViscosHttpBuilder {
        ViscosHttpBuilder::new(token)
    }

    /// Default konfigürasyonla (30s timeout) kısa yol kurulum.
    ///
    /// **Yan etki:** `twilight_http::Client::builder()` rustls crypto provider
    /// kurulumunu uygulamadan (process başına bir kez) bekler. Çağıran taraf
    /// `viscos` binary'sinde `main` başlangıcında `rustls::crypto::ring::default_provider().install_default().unwrap()`
    /// çağırmalı. Aksi halde TLS handshake panic eder.
    pub fn new(token: SecretString) -> Result<Self, ApiError> {
        Self::builder(token).build()
    }

    /// Aktif token'ın audit-güvenli expose'u (call-site'ta review edilebilir).
    #[must_use]
    pub fn expose_token(&self) -> &str {
        self.token.expose_secret()
    }

    // -----------------------------------------------------------------------
    // Core / auth
    // -----------------------------------------------------------------------

    /// `GET /users/@me` — token validation + mevcut kullanıcı bilgisi.
    ///
    /// **Not:** Twilight `current_user` endpoint'i `CurrentUser` döndürür (User'ın
    /// özelleştirilmiş hali — email, mfa_enabled, verified alanları dahil).
    /// Genel User objesi gerektiğinde caller `CurrentUser → User` dönüşümünü
    /// kendisi yapar (Faz 2.0'da ihtiyaç yok).
    #[instrument(skip(self))]
    pub async fn current_user(&self) -> Result<CurrentUser, ApiError> {
        let resp = self.client.current_user().await?;
        let user = resp.model().await?;
        debug!(user_id = user.id.get(), "current_user ok");
        Ok(user)
    }

    // -----------------------------------------------------------------------
    // Guilds
    // -----------------------------------------------------------------------

    /// `GET /users/@me/guilds` — kullanıcının bulunduğu sunucular (paginated).
    ///
    /// **Faz 2.0 sınırı:** İlk sayfa (200 guild) döner. Pagination Faz 4 cache
    /// katmanında `From<Id>` ile handle edilecek.
    ///
    /// **Not:** Twilight `current_user_guilds` endpoint'i `CurrentUserGuild`
    /// döndürür (partial guild: id + name + icon + permissions + features).
    /// Tam `Guild` objesi gerektiğinde `client.guild(id)` ile ayrı çağrı yapılır.
    #[instrument(skip(self))]
    pub async fn guilds(&self) -> Result<Vec<CurrentUserGuild>, ApiError> {
        let resp = self.client.current_user_guilds().limit(200u16).await?;
        let guilds: Vec<CurrentUserGuild> = resp.model().await?;
        debug!(count = guilds.len(), "current_user_guilds ok");
        Ok(guilds)
    }

    /// `GET /guilds/{id}/channels` — sunucu kanal listesi.
    #[instrument(skip(self))]
    pub async fn guild_channels(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> Result<Vec<Channel>, ApiError> {
        let resp = self.client.guild_channels(guild_id).await?;
        let channels: Vec<Channel> = resp.model().await?;
        debug!(
            guild_id = guild_id.get(),
            count = channels.len(),
            "guild_channels ok"
        );
        Ok(channels)
    }

    /// `GET /guilds/{id}/members` — sunucu üye listesi (limit 1..=1000).
    ///
    /// **Not:** Twilight `limit` parametresi `u16`. Clamp u16 max'ına kadar.
    #[instrument(skip(self))]
    pub async fn guild_members(
        &self,
        guild_id: Id<GuildMarker>,
        limit: u16,
    ) -> Result<Vec<twilight_model::guild::Member>, ApiError> {
        let limit = limit.clamp(1, 1000);
        let resp = self.client.guild_members(guild_id).limit(limit).await?;
        let members: Vec<twilight_model::guild::Member> = resp.model().await?;
        debug!(
            guild_id = guild_id.get(),
            count = members.len(),
            "guild_members ok"
        );
        Ok(members)
    }

    // -----------------------------------------------------------------------
    // Channels (DM)
    // -----------------------------------------------------------------------

    /// `POST /users/@me/channels` — kullanıcıyla DM aç (idempotent).
    #[instrument(skip(self))]
    pub async fn create_dm(&self, recipient_id: Id<UserMarker>) -> Result<Channel, ApiError> {
        let resp = self.client.create_private_channel(recipient_id).await?;
        let channel = resp.model().await?;
        debug!(channel_id = channel.id.get(), "create_dm ok");
        Ok(channel)
    }

    // -----------------------------------------------------------------------
    // Messages
    // -----------------------------------------------------------------------

    /// `POST /channels/{id}/messages` — kanalda mesaj gönder.
    ///
    /// **Shadow mode:** Faz 1.5'teki 24h shadow mode aktifken çağıran taraf
    /// (`viscos-shell`) bu method'u çağırmadan önce `ShadowMode::allows_write()`
    /// kontrol etmeli. Bu method bilinçli olarak shadow kontrolü yapmaz — policy
    /// üst katmandadır (ADR-0012 §3.B).
    #[instrument(skip(self, content))]
    pub async fn create_message(
        &self,
        channel_id: Id<ChannelMarker>,
        content: &str,
    ) -> Result<Message, ApiError> {
        let resp = self
            .client
            .create_message(channel_id)
            .content(content)
            .await?;
        let message = resp.model().await?;
        debug!(
            channel_id = channel_id.get(),
            message_id = message.id.get(),
            "create_message ok"
        );
        Ok(message)
    }

    /// `GET /channels/{id}/messages` — mesaj geçmişi (limit 1..=100).
    #[instrument(skip(self))]
    pub async fn channel_messages(
        &self,
        channel_id: Id<ChannelMarker>,
        limit: u16,
    ) -> Result<Vec<Message>, ApiError> {
        let limit = limit.clamp(1, 100);
        let resp = self
            .client
            .channel_messages(channel_id)
            .limit(limit)
            .await?;
        let messages: Vec<Message> = resp.model().await?;
        debug!(
            channel_id = channel_id.get(),
            count = messages.len(),
            "channel_messages ok"
        );
        Ok(messages)
    }

    /// `PATCH /channels/{id}/messages/{id}` — mesaj düzenle (kendi mesajını).
    #[instrument(skip(self, content))]
    pub async fn update_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        content: &str,
    ) -> Result<Message, ApiError> {
        let resp = self
            .client
            .update_message(channel_id, message_id)
            .content(Some(content))
            .await?;
        let message = resp.model().await?;
        debug!(message_id = message_id.get(), "update_message ok");
        Ok(message)
    }

    /// `DELETE /channels/{id}/messages/{id}` — mesaj sil.
    #[instrument(skip(self))]
    pub async fn delete_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> Result<(), ApiError> {
        self.client.delete_message(channel_id, message_id).await?;
        debug!(message_id = message_id.get(), "delete_message ok");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Typing indicator
    // -----------------------------------------------------------------------

    /// `POST /channels/{id}/typing` — typing indicator tetikle (10s timeout).
    #[instrument(skip(self))]
    pub async fn trigger_typing(&self, channel_id: Id<ChannelMarker>) -> Result<(), ApiError> {
        self.client.create_typing_trigger(channel_id).await?;
        debug!(channel_id = channel_id.get(), "trigger_typing ok");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Reactions
    // -----------------------------------------------------------------------

    /// `PUT /channels/{id}/messages/{id}/reactions/{emoji}/@me` — reaksiyon ekle.
    ///
    /// **Faz 2.0 stub:** `RequestReactionType` API'sı twilight 0.17'de refactor
    /// edildi (path template). Tam implementasyon Faz 2.0 follow-up.
    pub async fn create_reaction(
        &self,
        _channel_id: Id<ChannelMarker>,
        _message_id: Id<MessageMarker>,
        _emoji: &str,
    ) -> Result<(), ApiError> {
        Err(ApiError::Other(
            "create_reaction: stub — Faz 2.0 follow-up (twilight 0.17 ReactionType API)"
                .to_string(),
        ))
    }

    /// `DELETE /channels/{id}/messages/{id}/reactions/{emoji}/@me` — kendi reaksiyonunu sil.
    pub async fn delete_own_reaction(
        &self,
        _channel_id: Id<ChannelMarker>,
        _message_id: Id<MessageMarker>,
        _emoji: &str,
    ) -> Result<(), ApiError> {
        Err(ApiError::Other(
            "delete_own_reaction: stub — Faz 2.0 follow-up".to_string(),
        ))
    }

    // -----------------------------------------------------------------------
    // Health
    // -----------------------------------------------------------------------

    /// 401 kontrolü: hızlı bir `current_user` çağrısı, sonuç sadece başarılıysa token canlı.
    /// Faz 3.0'da `viscos-auth::handle_401` ile koordinasyon eklenir.
    #[instrument(skip(self))]
    pub async fn validate_token(&self) -> Result<u64, ApiError> {
        match self.current_user().await {
            Ok(user) => Ok(user.id.get()),
            Err(ApiError::Unauthorized) => {
                warn!("validate_token: 401 received, token is invalid");
                Err(ApiError::Unauthorized)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_default_timeout_is_30s() {
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn builder_accepts_timeout_override() {
        let builder =
            ViscosHttpBuilder::new(SecretString::new("dummy".to_string().into_boxed_str()))
                .timeout(Duration::from_secs(5));
        assert_eq!(builder.timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn token_expose_is_callable() {
        // Audit noktası: expose_secret() call-site review'da grep'lenebilir.
        // twilight_http::Client::builder() artık tokio runtime + rustls
        // crypto provider bekliyor. Test ortamında ikisini de kur.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let vh = ViscosHttp {
            client: TwilightClient::builder().token("test".to_string()).build(),
            token: SecretString::new("test-token".to_string().into_boxed_str()),
        };
        assert_eq!(vh.expose_token(), "test-token");
    }
}
