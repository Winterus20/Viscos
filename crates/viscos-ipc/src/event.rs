//! Rust → JS küçük olay enum'ları (push exception).
//!
//! Push-based event'ler sadece **küçük ve gerçek zamanlı** olaylar için:
//! - Mention count update (tray badge).
//! - Native notification (DM mention).
//! - Watchdog alert (GDI leak warning).
//!
//! Tüm diğer state transferi **pull-based** (`IpcCommand`).

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
    /// Mention/unread sayısı değişti → tray badge güncelle.
    UnreadCountChanged {
        /// Yeni unread count (tüm guild'ler).
        count: u32,
    },
    /// Tema değişti (OS veya kullanıcı tarafından).
    ThemeChanged {
        /// Yeni tema (`"dark"` | `"light"`).
        theme: String,
    },
    /// Watchdog alert (GDI leak / IPC buffer / vb.).
    WatchdogAlert { kind: WatchdogKind, message: String },
    // Faz 2+'da genişletilecek:
    // - `MessageReceived { channel_id, message_id }`
    // - `GuildJoined { guild_id }`
    // - `AuthStateChanged { state: "logged_in" | "logged_out" }`
    //
    // Not: Bu doc comment listesi Faz 2+'da eklenecek variant'ları gösterir;
    // enum #[non_exhaustive] olduğu için burada tanımlanmaları şart değil.
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
}
