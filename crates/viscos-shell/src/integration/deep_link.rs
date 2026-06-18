//! Deep linking — `viscos://` URL parser + Windows registry registration.
//!
//! Faz 6.0'da desteklenen URL formatları:
//! - `viscos://channel/{guild_id}/{channel_id}`
//! - `viscos://channel/{channel_id}` (DM)
//! - `viscos://invite/{code}`
//! - `viscos://user/{user_id}` (Faz 6.x)
//! - `viscos://plugin/{id}` (Faz 6.x — Vencord plugin yükle)
//!
//! Windows registry kaydı Faz 8.0'da MSI installer ile yapılacak; Faz 6.0
//! stub'ı sadece parser + `register_protocol()` no-op (warn log).
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md` §4 Deep Linking](../../../.cursor/plans/phase-6.0-hotkeys.md)

use viscos_error::ViscosError;

/// Parse edilmiş deep link action.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeepLinkAction {
    /// Kanal aç (DM veya guild kanalı).
    OpenChannel {
        /// Guild ID (None = DM).
        guild_id: Option<u64>,
        /// Channel ID.
        channel_id: u64,
    },
    /// Davet kodu kabul et.
    OpenInvite {
        /// Discord invite code.
        code: String,
    },
    /// Plugin yükle / etkinleştir (Vencord).
    OpenPlugin {
        /// Plugin ID.
        id: String,
    },
    /// Bilinmeyen format — orijinal URL string olarak.
    Unknown(String),
}

/// `viscos://` URL'ini parse et.
///
/// # Examples
///
/// ```
/// use viscos_shell::integration::deep_link::{parse_viscos_url, DeepLinkAction};
///
/// let action = parse_viscos_url("viscos://channel/123/456").unwrap();
/// assert!(matches!(action, DeepLinkAction::OpenChannel { guild_id: Some(123), channel_id: 456 }));
/// ```
#[must_use]
pub fn parse_viscos_url(url: &str) -> Option<DeepLinkAction> {
    let stripped = url.strip_prefix("viscos://")?;
    let parts: Vec<&str> = stripped.split('/').filter(|s| !s.is_empty()).collect();

    match parts.first().copied() {
        Some("channel") => match parts.len() {
            // viscos://channel/{channel_id} → DM
            1 => None, // empty channel id
            2 => parts[1]
                .parse::<u64>()
                .ok()
                .map(|channel_id| DeepLinkAction::OpenChannel {
                    guild_id: None,
                    channel_id,
                }),
            // viscos://channel/{guild_id}/{channel_id}
            3 => {
                let guild_id = parts[1].parse::<u64>().ok()?;
                let channel_id = parts[2].parse::<u64>().ok()?;
                Some(DeepLinkAction::OpenChannel {
                    guild_id: Some(guild_id),
                    channel_id,
                })
            }
            _ => Some(DeepLinkAction::Unknown(url.to_string())),
        },
        Some("invite") if parts.len() == 2 => Some(DeepLinkAction::OpenInvite {
            code: parts[1].to_string(),
        }),
        Some("plugin") if parts.len() == 2 => Some(DeepLinkAction::OpenPlugin {
            id: parts[1].to_string(),
        }),
        // viscos:// (boş path) veya bilinmeyen scheme
        _ => {
            if url == "viscos://" || url == "viscos:" {
                None
            } else {
                Some(DeepLinkAction::Unknown(url.to_string()))
            }
        }
    }
}

/// Windows registry'ye `viscos://` URI scheme'ini kaydet (stub).
///
/// Faz 6.0'da gerçek registry yazımı MSI installer (Faz 8.0) ile olacak.
/// Burada method sadece `Ok(())` döner + warn log.
///
/// Gerçek implementasyon (Faz 8.0):
/// ```ignore
/// use winreg::enums::*;
/// use winreg::RegKey;
/// let hkcu = RegKey::predef(HKEY_CURRENT_USER);
/// let key = hkcu.create_subkey(r"Software\Classes\viscos")?;
/// key.set_value("URL Protocol", &"")?;
/// key.set_value("", &"URL:Viscos Protocol")?;
/// let shell = key.create_subkey(r"shell\open\command")?;
/// shell.set_value("", &format!("\"{}\" \"%1\"", exe_path.display()))?;
/// ```
///
/// # Errors
///
/// Faz 6.0 stub'ı hiçbir zaman hata dönmez.
pub fn register_protocol() -> Result<(), ViscosError> {
    tracing::warn!(
        "register_protocol() is a stub in Faz 6.0 — real registry write happens in MSI installer (Faz 8.0)"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_channel_with_guild_and_channel() {
        let action = parse_viscos_url("viscos://channel/123/456").unwrap();
        assert_eq!(
            action,
            DeepLinkAction::OpenChannel {
                guild_id: Some(123),
                channel_id: 456
            }
        );
    }

    #[test]
    fn parse_channel_dm_only() {
        let action = parse_viscos_url("viscos://channel/789").unwrap();
        assert_eq!(
            action,
            DeepLinkAction::OpenChannel {
                guild_id: None,
                channel_id: 789
            }
        );
    }

    #[test]
    fn parse_channel_invalid_id_returns_none() {
        // 'abc' u64 parse edilemez → None (parse başarısız)
        let action = parse_viscos_url("viscos://channel/abc/456");
        assert!(action.is_none());
    }

    #[test]
    fn parse_invite() {
        let action = parse_viscos_url("viscos://invite/abcdef").unwrap();
        assert_eq!(
            action,
            DeepLinkAction::OpenInvite {
                code: "abcdef".to_string()
            }
        );
    }

    #[test]
    fn parse_plugin() {
        let action = parse_viscos_url("viscos://plugin/my-plugin").unwrap();
        assert_eq!(
            action,
            DeepLinkAction::OpenPlugin {
                id: "my-plugin".to_string()
            }
        );
    }

    #[test]
    fn parse_unknown_scheme() {
        assert!(parse_viscos_url("https://example.com").is_none());
    }

    #[test]
    fn parse_wrong_prefix_returns_unknown() {
        let action = parse_viscos_url("viscos://other/foo").unwrap();
        assert!(matches!(action, DeepLinkAction::Unknown(_)));
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_viscos_url("viscos://").is_none());
        assert!(parse_viscos_url("").is_none());
    }

    #[test]
    fn parse_too_many_path_segments_returns_unknown() {
        let action = parse_viscos_url("viscos://channel/1/2/3/4").unwrap();
        assert!(matches!(action, DeepLinkAction::Unknown(_)));
    }

    #[test]
    fn register_protocol_stub_succeeds() {
        assert!(register_protocol().is_ok());
    }
}
