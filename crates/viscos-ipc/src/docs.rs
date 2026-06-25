//! Statik dokümantasyon tabloları + test fixture helper'ları.
//!
//! Bu modül çalışma zamanı kod içermez; yalnızca `router` modülündeki 17
//! komut varyantının Faz → handler eşleme tablosunu ve test'lerde kullanılan
//! fixture helper'ları taşır. Amaç: `router.rs` dosyasını 400 satır soft
//! limit'inin altında tutmak (`.cursorrules` §2).
//!
//! **Kaynak-kod eşleme:** Tablodaki her satır, `router::StubHandler` içindeki
//! `match` koluyla bire bir eşleşir. Yeni variant eklendiğinde her iki yer
//! güncellenmelidir.

/// Faz 1.0 → Faz 5.0 stub command eşleme tablosu.
///
/// | # | Command variant        | Faz      | Rationale                                    | Forward-reference                          |
/// |---|------------------------|----------|----------------------------------------------|--------------------------------------------|
/// | 1 | `AuthPasteToken`       | Faz 2.0  | C3 token-paste fallback (ADR-0011)           | `viscos-auth::AuthHandler`                 |
/// | 2 | `AuthValidateMfa`      | Faz 2.0  | TOTP code üretimi (ADR-0011)                 | `viscos-auth::MfaHandler`                  |
/// | 3 | `AuthLogout`           | Faz 2.0  | Keyring temizliği (ADR-0011)                 | `viscos-auth::AuthHandler::logout`         |
/// | 4 | `AuthGetStatus`        | Faz 2.0  | Keyring + cache snapshot (ADR-0011)          | `viscos-auth::AuthHandler::status`         |
/// | 5 | `LoginRequest`         | Faz 2.0  | Geriye uyumluluk (`AuthPasteToken` tercih)   | `viscos-auth` (redirect)                   |
/// | 6 | `Logout`               | Faz 2.0  | Geriye uyumluluk (`AuthLogout` tercih)       | `viscos-auth` (redirect)                   |
/// | 7 | `GetGuildList`         | Faz 3.0  | REST + cache merge (ADR-0010)                | `viscos-gateway::GuildHandler`             |
/// | 8 | `GetChannelList`       | Faz 3.0  | REST + cache merge (ADR-0010)                | `viscos-gateway::ChannelHandler`           |
/// | 9 | `GetMessages`          | Faz 3.0  | Cache öncelikli read (ADR-0010)              | `viscos-gateway::MessageHandler::list`     |
/// |10 | `SendMessage`          | Faz 3.0  | `POST /channels/{id}/messages` (ADR-0008)    | `viscos-gateway::MessageHandler::send`     |
/// |11 | `TriggerTyping`        | Faz 3.0  | `POST /channels/{id}/typing` (ADR-0008)      | `viscos-gateway::MessageHandler::typing`   |
/// |12 | `SaveMessageDraft`     | Faz 5.0  | Periyodik autosave (Faz 5.0 1b)              | `viscos-cache::DraftStore`                 |
/// |13 | `CancelMessageDraft`   | Faz 5.0  | Draft temizleme (Faz 5.0 1b)                 | `viscos-cache::DraftStore`                 |
/// |14 | `MarkChannelRead`      | Faz 3.0  | Mention badge sıfırlama (ADR-0008)          | `viscos-gateway::AckHandler`               |
/// |15 | `GetUnreadCount`       | Faz 2.0  | Mention badge (Faz 2.0 1b)                   | `viscos-core::MentionStore`                |
/// |16 | `Navigate`             | Faz 1.6  | WebView2 deep-link + CEF fallback (ADR-0012) | `viscos-webview::NavHandler`               |
/// |17 | `SetTheme`             | Faz 5.0  | Theme sync (Faz 5.0 1b)                      | `viscos-shell::ThemeHandler`               |
pub const STUB_COMMAND_TABLE_DOC: &str = "see module docs";

/// 17 stub command varyantını test için inşa eder.
///
/// `pub` API yüzeyi genişletmemek için `#[cfg(test)]` altında `pub(crate)` —
/// `router::tests` modülü tarafından tüketilir. Production binary'ye
/// sızmaz (`dead_code` uyarısı test build'inde göz ardı edilir).
#[cfg(test)]
pub(crate) fn all_stub_commands() -> Vec<(&'static str, crate::command::IpcCommand)> {
    use crate::command::IpcCommand;
    vec![
        (
            "GetUnreadCount",
            IpcCommand::GetUnreadCount { guild_id: None },
        ),
        (
            "Navigate",
            IpcCommand::Navigate {
                url: "https://discord.com".into(),
            },
        ),
        (
            "SetTheme",
            IpcCommand::SetTheme {
                theme: "light".into(),
            },
        ),
        (
            "AuthPasteToken",
            IpcCommand::AuthPasteToken { token: "x".into() },
        ),
        (
            "AuthValidateMfa",
            IpcCommand::AuthValidateMfa {
                totp_secret: "x".into(),
            },
        ),
        ("AuthLogout", IpcCommand::AuthLogout),
        ("AuthGetStatus", IpcCommand::AuthGetStatus),
        ("LoginRequest", IpcCommand::LoginRequest { token: None }),
        ("Logout", IpcCommand::Logout {}),
        ("GetGuildList", IpcCommand::GetGuildList {}),
        ("GetChannelList", IpcCommand::GetChannelList { guild_id: 1 }),
        (
            "GetMessages",
            IpcCommand::GetMessages {
                channel_id: 7,
                limit: 50,
            },
        ),
        (
            "SendMessage",
            IpcCommand::SendMessage {
                channel_id: 7,
                content: "hi".into(),
            },
        ),
        ("TriggerTyping", IpcCommand::TriggerTyping { channel_id: 7 }),
        (
            "SaveMessageDraft",
            IpcCommand::SaveMessageDraft {
                channel_id: 7,
                content: "x".into(),
            },
        ),
        (
            "CancelMessageDraft",
            IpcCommand::CancelMessageDraft { channel_id: 7 },
        ),
        (
            "MarkChannelRead",
            IpcCommand::MarkChannelRead { channel_id: 7 },
        ),
    ]
}
