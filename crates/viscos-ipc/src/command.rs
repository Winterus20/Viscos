//! JS → Rust pull-based command enum.
//!
//! Her yeni command variant **breaking change** sayılmaz (`#[non_exhaustive]` +
//! Rust tarafı default'a düşer). Ancak Rust tarafı yeni variant handle etmek
//! zorundadır — `DefaultIpcRouter` default olarak
//! [`IpcCommandError::Unimplemented`] döner.
//!
//! # Protocol
//!
//! Serde tagged enum:
//! ```json
//! {"type": "GetUnreadCount", "data": {"guild_id": null}}
//! ```
//!
//! **Tüm command'lar async** (handler `async fn`). JS tarafı her zaman
//! `await viscos.invoke(cmd)` ile çağırır.

use serde::{Deserialize, Serialize};

use crate::types::IpcCommandError;

/// Frontend → Backend pull-based komut.
///
/// `#[non_exhaustive]` — yeni varyant eklemek non-breaking. Tüketici kodu
/// `_ =>` kolu bulundurmalı. Detaylar için [`crate::types`] modülüne bak.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[non_exhaustive]
pub enum IpcCommand {
    // -----------------------------------------------------------------------
    // Auth (Faz 2.0 C3 — token paste + MFA TOTP)
    // -----------------------------------------------------------------------
    /// Kullanıcının Discord'dan kopyaladığı token'ı yapıştır.
    ///
    /// C3 (token paste fallback) akışı — `/auth/login` kullanılmaz (ToS risk,
    /// undocumented). Token format validate edilir, keyring'e yazılır.
    /// `GET /users/@me` doğrulaması shell handler'da yapılır.
    AuthPasteToken {
        /// Ham Discord auth token (kullanıcı clipboard'dan yapıştırır).
        token: String,
    },
    /// MFA TOTP challenge — base32 TOTP secret'tan 6 haneli kod üret.
    ///
    /// Üretilen kod `POST /api/v10/auth/mfa/totp`'a gönderilir (documented,
    /// ToS-safe endpoint). Secret keyring'e yazılmaz — sadece kod üretilir.
    AuthValidateMfa {
        /// Base32 kodlu TOTP secret (kullanıcının authenticator uygulamasından).
        totp_secret: String,
    },
    /// Oturumu kapat — token'ı keyring'den sil, gateway disconnect tetikle.
    AuthLogout,
    /// Mevcut auth durumunu sorgula (token var mı, user_id nedir).
    ///
    /// Cevap: `{"authenticated": bool, "user_id": Option<String>}`.
    AuthGetStatus,

    /// Login başlat — token'ı validate et ve gateway connect tetikle.
    ///
    /// Geriye dönük uyumluluk: Faz 2.0 1b'den miras kalan komut.
    LoginRequest {
        /// Keyring'den okunacak mevcut kullanıcı token'ı (keyring boşsa
        /// explicit token geçilebilir).
        token: Option<String>,
    },
    /// Oturumu kapat — token'ı keyring'den sil, gateway disconnect.
    ///
    /// Geriye dönük uyumluluk: `AuthLogout` tercih edilir.
    Logout {},

    // -----------------------------------------------------------------------
    // Guild + channel metadata (Faz 3.0)
    // -----------------------------------------------------------------------
    /// Kullanıcının guild listesi (REST + cache merge).
    GetGuildList {},
    /// Belirli bir guild'in kanal listesi.
    GetChannelList {
        /// Discord guild snowflake.
        guild_id: u64,
    },

    // -----------------------------------------------------------------------
    // Messages (Faz 3.0)
    // -----------------------------------------------------------------------
    /// Bir kanaldaki son mesajları getir (cache öncelikli, REST fallback).
    GetMessages {
        /// Discord channel snowflake.
        channel_id: u64,
        /// Max mesaj sayısı (1..=100).
        limit: u16,
    },
    /// Yeni mesaj gönder (REST POST /channels/{id}/messages).
    SendMessage {
        /// Discord channel snowflake.
        channel_id: u64,
        /// Mesaj içeriği (markdown).
        content: String,
    },
    /// Kanalda yazıyor göstergesi tetikle (REST POST /channels/{id}/typing).
    TriggerTyping {
        /// Discord channel snowflake.
        channel_id: u64,
    },
    /// Mesaj draft'ını autosave et (periyodik, watchdog tetiklemeli).
    SaveMessageDraft {
        /// Discord channel snowflake.
        channel_id: u64,
        /// Taslak içerik.
        content: String,
    },
    /// Kaydedilmiş draft'ı iptal et.
    CancelMessageDraft {
        /// Discord channel snowflake.
        channel_id: u64,
    },
    /// Kanalı okundu olarak işaretle (mention badge sıfırla).
    MarkChannelRead {
        /// Discord channel snowflake.
        channel_id: u64,
    },

    // -----------------------------------------------------------------------
    // Phase-1 iskelet komutları (geriye uyumluluk)
    // -----------------------------------------------------------------------
    /// Belirli guild veya tümü için okunmamış mesaj sayısı.
    ///
    /// `guild_id = None` → tüm guild'ler. `Some(id)` → yalnızca o guild.
    GetUnreadCount {
        /// Discord guild snowflake. `None` = aggregate.
        guild_id: Option<u64>,
    },
    /// Frontend'i belirli bir URL'e yönlendir.
    Navigate {
        /// Hedef URL (örn. Discord kanal deep-link).
        url: String,
    },
    /// Tema değiştir (`"dark"` | `"light"`).
    SetTheme {
        /// Tema adı.
        theme: String,
    },
}

/// Komut için handler trait.
///
/// Async — JS tarafı `await` ile bekler. Default implementasyon
/// `IpcCommandError::Unimplemented` döner; bu sayede yeni command eklendiğinde
/// implementasyon gecikse bile router compile olur.
#[async_trait::async_trait]
pub trait IpcHandler: Send + Sync {
    /// Command'ı handle et.
    ///
    /// # Errors
    ///
    /// Default implementasyon `IpcCommandError::Unimplemented(...)` döner.
    /// Faz 2+'da Discord API hataları, rate-limit, payload decode hataları
    /// typed olarak yüzeye çıkar.
    async fn handle(&self, cmd: IpcCommand) -> Result<serde_json::Value, IpcCommandError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_serde_tag_is_type() {
        let cmd = IpcCommand::Navigate {
            url: "https://discord.com/channels/1/2".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"type\":\"Navigate\""));
        assert!(json.contains("\"url\""));
    }

    #[test]
    fn command_serde_set_theme() {
        let cmd = IpcCommand::SetTheme {
            theme: "light".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: IpcCommand = serde_json::from_str(&json).unwrap();
        match back {
            IpcCommand::SetTheme { theme } => assert_eq!(theme, "light"),
            _ => panic!("variant mismatch"),
        }
    }

    #[test]
    fn command_is_non_exhaustive_compile_time() {
        // `#[non_exhaustive]` olduğu için dışarıdan exhaustive match derlenmez.
        // Burada sadece oluşturma testi.
        let _ = IpcCommand::GetUnreadCount { guild_id: Some(42) };
    }

    #[test]
    fn new_commands_round_trip() {
        // Faz 3.0 + Faz 5.0 eklenen yeni command varyantları.
        let send = IpcCommand::SendMessage {
            channel_id: 123,
            content: "hello".into(),
        };
        let json = serde_json::to_string(&send).unwrap();
        let back: IpcCommand = serde_json::from_str(&json).unwrap();
        match back {
            IpcCommand::SendMessage {
                channel_id,
                content,
            } => {
                assert_eq!(channel_id, 123);
                assert_eq!(content, "hello");
            }
            _ => panic!("SendMessage round-trip failed"),
        }

        let cancel = IpcCommand::CancelMessageDraft { channel_id: 7 };
        let json = serde_json::to_string(&cancel).unwrap();
        let back: IpcCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            IpcCommand::CancelMessageDraft { channel_id: 7 }
        ));

        let mark = IpcCommand::MarkChannelRead { channel_id: 9 };
        let json = serde_json::to_string(&mark).unwrap();
        let back: IpcCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            IpcCommand::MarkChannelRead { channel_id: 9 }
        ));
    }

    #[test]
    fn login_request_optional_token_round_trip() {
        let cmd = IpcCommand::LoginRequest { token: None };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: IpcCommand = serde_json::from_str(&json).unwrap();
        match back {
            IpcCommand::LoginRequest { token } => assert!(token.is_none()),
            _ => panic!("LoginRequest round-trip failed"),
        }

        let cmd = IpcCommand::LoginRequest {
            token: Some("xyz".into()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: IpcCommand = serde_json::from_str(&json).unwrap();
        match back {
            IpcCommand::LoginRequest { token } => assert_eq!(token.as_deref(), Some("xyz")),
            _ => panic!("LoginRequest round-trip failed"),
        }
    }

    #[test]
    fn auth_paste_token_round_trip() {
        let cmd = IpcCommand::AuthPasteToken {
            token: "NTkw.abc.def".into(),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(json.contains("\"type\":\"AuthPasteToken\""));
        let back: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        match back {
            IpcCommand::AuthPasteToken { token } => assert_eq!(token, "NTkw.abc.def"),
            _ => panic!("AuthPasteToken round-trip failed"),
        }
    }

    #[test]
    fn auth_validate_mfa_round_trip() {
        let cmd = IpcCommand::AuthValidateMfa {
            totp_secret: "GEZDGNBVGY3TQOJQ".into(),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(json.contains("\"type\":\"AuthValidateMfa\""));
        let back: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        match back {
            IpcCommand::AuthValidateMfa { totp_secret } => {
                assert_eq!(totp_secret, "GEZDGNBVGY3TQOJQ")
            }
            _ => panic!("AuthValidateMfa round-trip failed"),
        }
    }

    #[test]
    fn auth_logout_round_trip() {
        let cmd = IpcCommand::AuthLogout;
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(json.contains("\"type\":\"AuthLogout\""));
        let back: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(back, IpcCommand::AuthLogout));
    }

    #[test]
    fn auth_get_status_round_trip() {
        let cmd = IpcCommand::AuthGetStatus;
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(json.contains("\"type\":\"AuthGetStatus\""));
        let back: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(back, IpcCommand::AuthGetStatus));
    }
}
