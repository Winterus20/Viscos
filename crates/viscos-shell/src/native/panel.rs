//! iced native side panel — placeholder UI for Faz 5.0.
//!
//! Bu modül **sadece tip + layout iskeleti** sağlar. Gerçek data binding
//! (guild/channel/member listesi) Faz 5.x'te `viscos-core::types::Guild` vb.
//! ile doldurulacak. Faz 1.0 stub kararıyla uyumlu: minimal API yüzeyi
//! kurulur, içerik sonraki worker'larda genişletilir.
//!
//! `iced 0.14` API'si Faz 1.0'da spike durumunda — Faz 5.0 sonunda gerçek
//! `Element` döndüren layout fonksiyonu eklenecek.
//!
//! Cross-references:
//! - [`phase-5.0-native-ui.md` §5 Side Panel Widget'ları](../../../.cursor/plans/phase-5.0-native-ui.md)

use crate::native::theme::Theme;

/// iced native side panel — guild + channel + member listesi.
///
/// Faz 5.0 stub'ı: gerçek data binding olmadan sadece boş `view()` döner.
/// Faz 5.x'te `viscos-core::types::Guild` ve `Channel` bağlanacak.
#[derive(Debug, Clone, Default)]
pub struct SidePanel {
    /// Aktif tema (placeholder — iced style'ı besleyecek).
    pub theme: Theme,
}

impl SidePanel {
    /// Yeni side panel oluştur.
    #[must_use]
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    /// Aktif temayı döndür.
    #[must_use]
    pub const fn theme(&self) -> Theme {
        self.theme
    }

    /// iced `Element` döndürmesi gereken `view()` — Faz 5.0 stub'ı.
    ///
    /// Faz 5.0'da bu method `iced::Element<Message>` döndürmesi gerekir.
    /// Ancak `iced 0.14` spike'ı Faz 1.0'da tamamlanmadı; gerçek render
    /// Faz 5.x'te. Burada sadece `&self` üzerinden bilgi veren bir
    /// introspection API'si sunuyoruz.
    ///
    /// # Examples
    ///
    /// ```
    /// use viscos_shell::native::{SidePanel, theme::Theme};
    ///
    /// let panel = SidePanel::new(Theme::Dark);
    /// assert_eq!(panel.theme(), Theme::Dark);
    /// ```
    #[must_use]
    pub fn view(&self) -> PanelLayout {
        PanelLayout {
            width: 72,
            height: 600,
            guild_count: 0,
            channel_count: 0,
            member_count: 0,
        }
    }
}

/// `view()` döndürdüğü layout bilgisi (Faz 5.0 stub — gerçek `iced::Element`
/// yerine layout metadata).
///
/// `iced::Element` döndüren gerçek implementasyon Faz 5.x'te eklenecek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelLayout {
    /// Toplam panel genişliği (px).
    pub width: u32,
    /// Toplam panel yüksekliği (px).
    pub height: u32,
    /// Şu anda bağlı guild sayısı (0 = data binding henüz yok).
    pub guild_count: u32,
    /// Şu anda gösterilen kanal sayısı.
    pub channel_count: u32,
    /// Şu anda gösterilen üye sayısı.
    pub member_count: u32,
}

impl PanelLayout {
    /// Side panel dolu mu? (herhangi bir data var mı?)
    #[must_use]
    pub const fn is_populated(&self) -> bool {
        self.guild_count > 0 || self.channel_count > 0 || self.member_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_panel_default_uses_dark_theme() {
        let panel = SidePanel::default();
        assert_eq!(panel.theme(), Theme::Dark);
    }

    #[test]
    fn side_panel_new_sets_theme() {
        let panel = SidePanel::new(Theme::Light);
        assert_eq!(panel.theme(), Theme::Light);
    }

    #[test]
    fn view_returns_empty_layout_in_phase_5_0() {
        let panel = SidePanel::new(Theme::Dark);
        let layout = panel.view();
        assert_eq!(layout.width, 72);
        assert!(layout.height > 0);
        assert!(!layout.is_populated());
    }

    #[test]
    fn panel_layout_is_populated() {
        let layout = PanelLayout {
            width: 72,
            height: 600,
            guild_count: 3,
            channel_count: 0,
            member_count: 0,
        };
        assert!(layout.is_populated());
    }
}
