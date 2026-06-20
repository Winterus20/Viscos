//! Rust → JS küçük olay enum'ları (push exception).
//!
//! Push-based event'ler sadece **küçük ve gerçek zamanlı** olaylar için:
//! - Mention count update (tray badge).
//! - Native notification (DM mention).
//! - Watchdog alert (GDI leak warning).
//! - Yeni mesaj / mesaj düzenleme (Faz 3.0+).
//!
//! Tüm diğer state transferi **pull-based** ([`crate::IpcCommand`]).
//!
//! Cross-references:
//! - ADR-0012 §3 — pull-based IPC pattern.
//! - [`crate::types::IpcEventError`] — push başarısızlığında typed hata.

use serde::{Deserialize, Serialize};

/// Watchdog alert türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchdogKind {
    /// GDI warning threshold (default 7000) aşıldı.
    GdiLeakWarning,
    /// GDI critical threshold (default 9000) aşıldı → soft restart tetiklendi.
    GdiLeakCritical,
    /// IPC buffer 50 MB'ı aştı.
    IpcBufferWarning,
    /// IPC buffer 100 MB'ı aştı → WebView refresh.
    IpcBufferCritical,
}

/// Backend → Frontend küçük olay.
///
/// Push sadece bu enum varyantları için. Büyük JSON payload → Faz 4 SharedBuffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
#[non_exhaustive]
pub enum IpcEvent {
    // -----------------------------------------------------------------------
    // Auth (Faz 2.0 1b)
    // -----------------------------------------------------------------------
    /// Login başarılı (token validate edildi, gateway connect tetiklendi).
    LoginSuccess {
        /// Validate edilmiş user ID (snowflake).
        user_id: u64,
    },
    /// Login hatası (token invalid, MFA gerekli, vb.).
    LoginFailure {
        /// Hata kodu (örn. "401", "mfa_required").
        code: String,
        /// Hata mesajı.
        message: String,
    },

    // -----------------------------------------------------------------------
    // Messages (Faz 3.0 — gateway `MESSAGE_CREATE` push)
    // -----------------------------------------------------------------------
    /// Yeni mesaj alındı (gateway `MESSAGE_CREATE` veya `SendMessage` ack).
    MessageCreated {
        /// Discord channel snowflake.
        channel_id: u64,
        /// Discord message snowflake.
        message_id: u64,
    },
    /// Mevcut mesaj düzenlendi (gateway `MESSAGE_UPDATE`).
    MessageEdited {
        /// Discord channel snowflake.
        channel_id: u64,
        /// Discord message snowflake.
        message_id: u64,
    },

    // -----------------------------------------------------------------------
    // Drafts (Faz 5.0 1b)
    // -----------------------------------------------------------------------
    /// Mesaj draft'ı autosave edildi (periyodik watchdog callback).
    DraftSaved {
        /// Discord channel snowflake.
        channel_id: u64,
        /// Draft karakter sayısı.
        content_len: usize,
    },

    // -----------------------------------------------------------------------
    // Mentions / counts
    // -----------------------------------------------------------------------
    /// Mention/unread sayısı değişti → tray badge güncelle.
    UnreadCountChanged {
        /// Yeni unread count (tüm guild'ler).
        count: u32,
    },

    // -----------------------------------------------------------------------
    // Theme + watchdog (Faz 1.0 iskeleti)
    // -----------------------------------------------------------------------
    /// Tema değişti (OS veya kullanıcı tarafından).
    ThemeChanged {
        /// Yeni tema (`"dark"` | `"light"`).
        theme: String,
    },
    /// Watchdog alert (GDI leak / IPC buffer / vb.).
    WatchdogAlert { kind: WatchdogKind, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serde_tag_is_kind() {
        let evt = IpcEvent::UnreadCountChanged { count: 5 };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("\"kind\":\"UnreadCountChanged\""));
        assert!(json.contains("\"count\":5"));
    }

    #[test]
    fn watchdog_kind_serde_snake_case() {
        let kind = WatchdogKind::GdiLeakCritical;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"gdi_leak_critical\"");
    }

    #[test]
    fn watchdog_alert_event_serde_round_trip() {
        let evt = IpcEvent::WatchdogAlert {
            kind: WatchdogKind::IpcBufferWarning,
            message: "buffer 55MB".into(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: IpcEvent = serde_json::from_str(&json).unwrap();
        match back {
            IpcEvent::WatchdogAlert { kind, message } => {
                assert_eq!(kind, WatchdogKind::IpcBufferWarning);
                assert_eq!(message, "buffer 55MB");
            }
            _ => panic!("variant mismatch"),
        }
    }

    #[test]
    fn new_events_round_trip() {
        let login_ok = IpcEvent::LoginSuccess { user_id: 42 };
        let json = serde_json::to_string(&login_ok).unwrap();
        let back: IpcEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, IpcEvent::LoginSuccess { user_id: 42 }));

        let created = IpcEvent::MessageCreated {
            channel_id: 7,
            message_id: 100,
        };
        let json = serde_json::to_string(&created).unwrap();
        let back: IpcEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            IpcEvent::MessageCreated {
                channel_id: 7,
                message_id: 100
            }
        ));

        let edited = IpcEvent::MessageEdited {
            channel_id: 7,
            message_id: 100,
        };
        let json = serde_json::to_string(&edited).unwrap();
        let back: IpcEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, IpcEvent::MessageEdited { .. }));

        let draft = IpcEvent::DraftSaved {
            channel_id: 1,
            content_len: 128,
        };
        let json = serde_json::to_string(&draft).unwrap();
        let back: IpcEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            IpcEvent::DraftSaved {
                content_len: 128,
                ..
            }
        ));
    }

    #[test]
    fn login_failure_carries_code_and_message() {
        let evt = IpcEvent::LoginFailure {
            code: "mfa_required".into(),
            message: "MFA TOTP gerekli".into(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: IpcEvent = serde_json::from_str(&json).unwrap();
        match back {
            IpcEvent::LoginFailure { code, message } => {
                assert_eq!(code, "mfa_required");
                assert_eq!(message, "MFA TOTP gerekli");
            }
            _ => panic!("LoginFailure round-trip failed"),
        }
    }
}
