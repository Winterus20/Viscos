//! Cross-module integration tests — `viscos-shell::integration::deep_link`
//! URL parser.

use viscos_shell::integration::deep_link::{DeepLinkAction, parse_viscos_url};

#[test]
fn parses_channel_with_guild() {
    let action = parse_viscos_url("viscos://channel/123/456").expect("valid URL");
    assert_eq!(
        action,
        DeepLinkAction::OpenChannel {
            guild_id: Some(123),
            channel_id: 456
        }
    );
}

#[test]
fn parses_channel_dm() {
    let action = parse_viscos_url("viscos://channel/789").expect("valid URL");
    assert_eq!(
        action,
        DeepLinkAction::OpenChannel {
            guild_id: None,
            channel_id: 789
        }
    );
}

#[test]
fn parses_invite() {
    let action = parse_viscos_url("viscos://invite/xyz123").expect("valid URL");
    assert_eq!(
        action,
        DeepLinkAction::OpenInvite {
            code: "xyz123".to_string()
        }
    );
}

#[test]
fn parses_plugin() {
    let action = parse_viscos_url("viscos://plugin/vencord-themes").expect("valid URL");
    assert_eq!(
        action,
        DeepLinkAction::OpenPlugin {
            id: "vencord-themes".to_string()
        }
    );
}

#[test]
fn returns_none_for_non_viscos_scheme() {
    assert!(parse_viscos_url("https://example.com").is_none());
    assert!(parse_viscos_url("discord://channel/1").is_none());
}

#[test]
fn returns_none_for_empty_url() {
    assert!(parse_viscos_url("").is_none());
}

#[test]
fn unknown_route_yields_unknown_variant() {
    let action = parse_viscos_url("viscos://other/foo").expect("non-empty");
    assert!(matches!(action, DeepLinkAction::Unknown(_)));
}

#[test]
fn too_many_path_segments_yields_unknown() {
    let action = parse_viscos_url("viscos://channel/1/2/3/4").expect("non-empty");
    assert!(matches!(action, DeepLinkAction::Unknown(_)));
}
