//! Cross-module integration tests — `viscos-shell::native::theme` palette
//! determinism + parse round-trip.

use viscos_shell::native::theme::{Theme, ThemePalette};

#[test]
fn dark_palette_has_discord_blurple_accent() {
    let p = ThemePalette::dark();
    assert_eq!(p.accent, "#5865F2");
}

#[test]
fn light_palette_shares_blurple_accent() {
    let p = ThemePalette::light();
    assert_eq!(p.accent, "#5865F2");
}

#[test]
fn all_palette_colors_are_seven_char_hex() {
    for palette in [ThemePalette::dark(), ThemePalette::light()] {
        for color in [
            &palette.background,
            &palette.sidebar,
            &palette.mention_badge,
            &palette.text_primary,
            &palette.text_muted,
            &palette.accent,
            &palette.hover,
        ] {
            assert!(color.starts_with('#'), "{color}");
            assert_eq!(color.len(), 7, "{color}");
        }
    }
}

#[test]
fn theme_palette_call_returns_correct_variant() {
    assert_eq!(Theme::Dark.palette(), ThemePalette::dark());
    assert_eq!(Theme::Light.palette(), ThemePalette::light());
    // Auto: stub → dark
    assert_eq!(Theme::Auto.palette(), ThemePalette::dark());
}

#[test]
fn theme_parse_accepts_lowercase_and_aliases() {
    assert_eq!(Theme::parse("dark"), Some(Theme::Dark));
    assert_eq!(Theme::parse("DARK"), Some(Theme::Dark));
    assert_eq!(Theme::parse("light"), Some(Theme::Light));
    assert_eq!(Theme::parse("auto"), Some(Theme::Auto));
    assert_eq!(Theme::parse("system"), Some(Theme::Auto));
}
