//! JS → Rust pull-based command enum.
//!
//! Her yeni command variant **breaking change** sayılmaz (`#[non_exhaustive]` +
//! Rust tarafı default'a düşer). Ancak Rust tarafı yeni variant handle etmek
//! zorundadır — `DefaultIpcRouter` default olarak `Unimplemented` döner.

use serde::{Deserialize, Serialize};

/// Frontend → Backend pull-based komut.
///
/// Faz 1.0'da sadece iskelet. Faz 2+'da `AuthLogin`, `SendMessage`, `LoadHistory`
/// gibi Discord API'ye bağlı komutlar eklenecek.
///
/// # Protocol
///
/// Serde tagged enum:
/// ```json
/// {"type": "GetUnreadCount", "data": {"guild_id": null}}
/// ```
///
/// **Tüm command'lar async** (handler `async fn`). JS tarafı her zaman
/// `await viscos.invoke(cmd)` ile çağırır.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[non_exhaustive]
pub enum IpcCommand {
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
    // Faz 2+'da genişletilecek komutlar:
    // - `SendMessage { channel_id, content }`
    // - `LoadHistory { channel_id, before, limit }`
    // - `MarkRead { channel_id, message_id }`
    // - `AuthLogin { token }` (Faz 2.0)
    // - `AuthLogout {}`
    // - `OpenSettings { section }`
    //
    // Not: Bu doc comment listesi Faz 2+'da eklenecek variant'ları gösterir;
    // enum #[non_exhaustive] olduğu için burada tanımlanmaları şart değil.
    //
    // Faz 2+'da `IpcCommand::SendMessage { ... }` gibi yeni variant'lar eklenecek.
    // Bu satırlar sadece plan referansı.
}

/// Komut için handler trait.
///
/// Async — JS tarafı `await` ile bekler. Default implementasyon
/// `ViscosError::Unimplemented` döner; bu sayede yeni command eklendiğinde
/// implementasyon gecikse bile router compile olur.
#[async_trait::async_trait]
pub trait IpcHandler: Send + Sync {
    /// Command'ı handle et.
    ///
    /// # Errors
    ///
    /// Her zaman `ViscosError::Unimplemented("phase-X.Y feature")` Faz 1.0'da.
    /// Faz 2+'da Discord API hataları, rate-limit, vb.
    async fn handle(&self, cmd: IpcCommand) -> viscos_error::Result<serde_json::Value>;
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
}
