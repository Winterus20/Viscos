//! Viscos theming system — Dark / Light / Auto.
//!
//! Faz 5.0'da iced side panel + WebView2 native notification accent rengi
//! palette'i tüketir. Vencord plugin'leri de bu palette'i `viscos.theme` API'si
//! üzerinden okuyabilir (Faz 6.x).
//!
//! Cross-references:
//! - [`phase-5.0-native-ui.md` §4 Theme System](../../../.cursor/plans/phase-5.0-native-ui.md).

/// Kullanıcı-facing tema seçimi.
///
/// `Auto` OS tema tercihini (`Windows: Settings → Personalization → Colors`)
/// izler; manuel override kullanıcı tarafından zorlanırsa `Dark` / `Light`
/// döner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Theme {
    /// Koyu arka plan, açık metin (#18181D / #D8D8D8).
    #[default]
    Dark,
    /// Açık arka plan, koyu metin (#FFFFFF / #101018).
    Light,
    /// OS tema tercihini takip et (Faz 6.x'te gerçek OS detection).
    Auto,
}

impl Theme {
    /// Tema için [`ThemePalette`] döndür.
    ///
    /// # Examples
    ///
    /// ```
    /// use viscos_shell::native::theme::{Theme, ThemePalette};
    ///
    /// let dark = Theme::Dark.palette();
    /// assert!(dark.background.starts_with('#'));
    /// ```
    #[must_use]
    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Dark => ThemePalette::dark(),
            Self::Light => ThemePalette::light(),
            // Auto: Faz 6.x'te OS detection eklenecek. Şimdilik dark fallback.
            Self::Auto => ThemePalette::dark(),
        }
    }

    /// Tema adını döndür (`"dark"` | `"light"` | `"auto"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Auto => "auto",
        }
    }

    /// String'ten parse et. Tanınmazsa `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "auto" | "system" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Renk paleti — Discord ile hizalı, native shell + WebView CSS injection
/// için hex formatında.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePalette {
    /// Arka plan (canvas / window background).
    pub background: String,
    /// Yan panel arka planı (guild + channel + member list).
    pub sidebar: String,
    /// Mention badge / hata rengi.
    pub mention_badge: String,
    /// Birincil metin.
    pub text_primary: String,
    /// İkincil / muted metin.
    pub text_muted: String,
    /// Aksan rengi (Discord blurple).
    pub accent: String,
    /// Hover state arka planı.
    pub hover: String,
}

impl ThemePalette {
    /// Discord-tarzı karanlık palet.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            background: "#18181D".into(),
            sidebar: "#141418".into(),
            mention_badge: "#D84C4C".into(),
            text_primary: "#D8D8D8".into(),
            text_muted: "#777880".into(),
            accent: "#5865F2".into(),
            hover: "#28282F".into(),
        }
    }

    /// Discord-tarzı açık palet.
    #[must_use]
    pub fn light() -> Self {
        Self {
            background: "#FFFFFF".into(),
            sidebar: "#F5F5F8".into(),
            mention_badge: "#D84C4C".into(),
            text_primary: "#101018".into(),
            text_muted: "#585866".into(),
            accent: "#5865F2".into(),
            hover: "#EAEAEF".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_dark() {
        assert_eq!(Theme::default(), Theme::Dark);
    }

    #[test]
    fn theme_as_str_round_trip() {
        for t in [Theme::Dark, Theme::Light, Theme::Auto] {
            let s = t.as_str();
            assert_eq!(Theme::parse(s), Some(t));
        }
    }

    #[test]
    fn theme_parse_unknown_returns_none() {
        assert!(Theme::parse("foo").is_none());
        assert!(Theme::parse("").is_none());
    }

    #[test]
    fn theme_parse_system_alias() {
        // Vesktop/Discord "system" → Auto
        assert_eq!(Theme::parse("system"), Some(Theme::Auto));
    }

    #[test]
    fn dark_and_light_palettes_differ_on_background() {
        let dark = ThemePalette::dark();
        let light = ThemePalette::light();
        assert_ne!(dark.background, light.background);
        assert_ne!(dark.text_primary, light.text_primary);
        // Accent aynı kalır (Discord blurple)
        assert_eq!(dark.accent, light.accent);
    }

    #[test]
    fn palette_palette_call_matches_const() {
        assert_eq!(Theme::Dark.palette(), ThemePalette::dark());
        assert_eq!(Theme::Light.palette(), ThemePalette::light());
        assert_eq!(Theme::Auto.palette(), ThemePalette::dark());
    }

    #[test]
    fn palette_hex_format() {
        let p = ThemePalette::dark();
        for color in [
            &p.background,
            &p.sidebar,
            &p.mention_badge,
            &p.text_primary,
            &p.text_muted,
            &p.accent,
            &p.hover,
        ] {
            assert!(color.starts_with('#'), "{color} should start with #");
            assert_eq!(color.len(), 7, "{color} should be #RRGGBB");
        }
    }
}
