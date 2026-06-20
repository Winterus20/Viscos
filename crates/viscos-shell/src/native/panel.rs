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
use std::collections::HashMap;

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

/// Native panel state — MVP-3 scaffold (Faz 5.0 sonunda gerçek data bağlanır).
///
/// `SidePanelState` izole state'i (seçili guild, seçili kanal, unread sayaçları)
/// `SidePanel`'den ayırır. Bu sayede render katmanı (iced `Element` üreten
/// `SidePanelRenderer`) ile state mutasyonu (event handler'ları) bağımsız
/// test edilebilir.
///
/// Tüm mutasyonlar `&mut self` üzerinden yapılır — `Send + Sync` değildir
/// çünkü iced `Element` üretimi main thread'de çalışır. İhtiyaç hâlinde
/// `Arc<parking_lot::RwLock<SidePanelState>>` ile sarılabilir (Faz 5.x).
#[derive(Debug, Clone, Default)]
pub struct SidePanelState {
    /// Aktif seçili guild ID (`None` = DM listesi gösterilir).
    selected_guild_id: Option<u64>,
    /// Aktif seçili kanal ID (`None` = hiçbir kanal seçili değil).
    selected_channel_id: Option<u64>,
    /// Kanal başına unread mention sayısı (`HashMap<channel_id, count>`).
    unread_counts: HashMap<u64, u32>,
}

impl SidePanelState {
    /// Yeni boş state oluştur.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Aktif guild seç.
    ///
    /// Aynı guild tekrar seçilirse idempotent — channel selection korunur.
    pub fn select_guild(&mut self, id: u64) {
        if self.selected_guild_id == Some(id) {
            return;
        }
        self.selected_guild_id = Some(id);
        // Guild değişince kanal seçimini temizlemek yerine koruyoruz; Faz 5.x'te
        // guild değişimi event'i ile ayrı bir reset kuralı gelecek.
    }

    /// Aktif kanal seç.
    ///
    /// Aynı kanal tekrar seçilirse unread sayacı sıfırlanır (mention tıklandı).
    pub fn select_channel(&mut self, id: u64) {
        if self.selected_channel_id == Some(id) {
            return;
        }
        self.selected_channel_id = Some(id);
        // Kanal açıldığında unread mention'ları temizle.
        self.unread_counts.insert(id, 0);
    }

    /// Bir kanalın unread mention sayısını ayarla (Gateway `MESSAGE_CREATE`
    /// mention event'inden sonra çağrılacak).
    pub fn set_unread(&mut self, channel_id: u64, count: u32) {
        if count == 0 {
            self.unread_counts.remove(&channel_id);
        } else {
            self.unread_counts.insert(channel_id, count);
        }
    }

    /// Increment unread sayacı (zero-saturation).
    pub fn increment_unread(&mut self, channel_id: u64) {
        let entry = self.unread_counts.entry(channel_id).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    /// Seçili guild ID.
    #[must_use]
    pub const fn selected_guild(&self) -> Option<u64> {
        self.selected_guild_id
    }

    /// Seçili kanal ID.
    #[must_use]
    pub const fn selected_channel(&self) -> Option<u64> {
        self.selected_channel_id
    }

    /// Belirli bir kanalın unread mention sayısı (yoksa 0).
    #[must_use]
    pub fn unread(&self, channel_id: u64) -> u32 {
        self.unread_counts.get(&channel_id).copied().unwrap_or(0)
    }

    /// Toplam unread mention sayısı (tüm kanallar).
    #[must_use]
    pub fn total_unread(&self) -> u32 {
        self.unread_counts.values().copied().sum()
    }

    /// Snapshot of unread map (test + IPC push için).
    #[must_use]
    pub fn unread_snapshot(&self) -> Vec<(u64, u32)> {
        let mut entries: Vec<(u64, u32)> =
            self.unread_counts.iter().map(|(k, v)| (*k, *v)).collect();
        entries.sort_unstable_by_key(|(k, _)| *k);
        entries
    }
}

/// Side panel render trait (MVP-3 stub — Faz 5.0 sonunda gerçek `iced` bind).
///
/// MVP-3'te gerçek `iced::Element` üretmiyoruz; renderer'lar `PanelLayout`
/// üretir. Faz 5.0'da bu trait'in dönüş tipi `iced::Element<SidePanelMessage>`
/// olacak ve `view()` iced `Widget` ağacı kuracak.
///
/// Renderer'ları trait object olarak tutmak MVP-3'te `Send + Sync` zorunluluğu
/// olmadığı için trait bound gevşek tutuldu; Faz 5.0'da `where Self: Send + Sync`
/// eklenecek.
pub trait SidePanelRenderer {
    /// State snapshot'ından bir `PanelLayout` üret.
    ///
    /// # Examples
    ///
    /// ```
    /// use viscos_shell::native::panel::{SidePanelRenderer, SidePanelState};
    ///
    /// struct Counting;
    /// impl SidePanelRenderer for Counting {
    ///     fn view(&self, _state: &SidePanelState) -> viscos_shell::native::panel::PanelLayout {
    ///         viscos_shell::native::panel::PanelLayout {
    ///             width: 240,
    ///             height: 600,
    ///             guild_count: 1,
    ///             channel_count: 5,
    ///             member_count: 12,
    ///         }
    ///     }
    /// }
    ///
    /// let r = Counting;
    /// let layout = r.view(&SidePanelState::new());
    /// assert_eq!(layout.width, 240);
    /// assert!(layout.is_populated());
    /// ```
    fn view(&self, state: &SidePanelState) -> PanelLayout;
}

/// Default MVP-3 renderer — boş layout döndürür (data binding henüz yok).
///
/// Faz 5.0'da bu renderer yerini iced `Widget` ağacı kuran gerçek bir
/// renderer'a bırakacak. Şimdilik `SidePanel::view()` ile aynı sayıları
/// üretir.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultSidePanelRenderer;

impl SidePanelRenderer for DefaultSidePanelRenderer {
    fn view(&self, _state: &SidePanelState) -> PanelLayout {
        PanelLayout {
            width: 72,
            height: 600,
            guild_count: 0,
            channel_count: 0,
            member_count: 0,
        }
    }
}

/// iced `Element` üretiminde kullanılacak message enum'u (MVP-3 placeholder).
///
/// `SidePanel`'in iced ile konuşması için bir message tipine ihtiyaç var;
/// MVP-3'te bu tip renderer tarafından üretilecek event'leri temsil eder.
/// Faz 5.0'da `iced::widget::button::on_press` callback'leri bu enum'u
/// kullanacak (`SelectGuild(id)`, `SelectChannel(id)`, vb.).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SidePanelMessage {
    /// Bir guild seçildi.
    SelectGuild(u64),
    /// Bir kanal seçildi.
    SelectChannel(u64),
    /// Mention badge'ına tıklandı.
    OpenUnread(u64),
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

    #[test]
    fn side_panel_state_starts_empty() {
        let state = SidePanelState::new();
        assert_eq!(state.selected_guild(), None);
        assert_eq!(state.selected_channel(), None);
        assert_eq!(state.total_unread(), 0);
    }

    #[test]
    fn side_panel_state_select_guild_is_idempotent() {
        let mut state = SidePanelState::new();
        state.select_guild(42);
        state.select_guild(42);
        state.select_guild(42);
        assert_eq!(state.selected_guild(), Some(42));
    }

    #[test]
    fn side_panel_state_channel_select_clears_unread() {
        let mut state = SidePanelState::new();
        state.select_guild(1);
        state.set_unread(99, 7);
        assert_eq!(state.unread(99), 7);

        // Opening the channel should clear its unread count.
        state.select_channel(99);
        assert_eq!(state.unread(99), 0);
        assert_eq!(state.selected_channel(), Some(99));
    }

    #[test]
    fn side_panel_state_set_unread_zero_removes_entry() {
        let mut state = SidePanelState::new();
        state.set_unread(7, 3);
        state.set_unread(7, 0);
        assert_eq!(state.unread(7), 0);
        assert_eq!(state.unread_snapshot().len(), 0);
    }

    #[test]
    fn side_panel_state_increment_unread_saturates() {
        let mut state = SidePanelState::new();
        state.set_unread(5, u32::MAX);
        state.increment_unread(5);
        assert_eq!(state.unread(5), u32::MAX);
    }

    #[test]
    fn side_panel_state_total_unread_sums_all_channels() {
        let mut state = SidePanelState::new();
        state.set_unread(1, 3);
        state.set_unread(2, 5);
        state.set_unread(3, 1);
        assert_eq!(state.total_unread(), 9);
    }

    #[test]
    fn default_renderer_returns_empty_layout() {
        let r = DefaultSidePanelRenderer;
        let layout = r.view(&SidePanelState::new());
        assert!(!layout.is_populated());
    }

    #[test]
    fn custom_renderer_observes_state() {
        struct ChannelCounter;
        impl SidePanelRenderer for ChannelCounter {
            fn view(&self, state: &SidePanelState) -> PanelLayout {
                PanelLayout {
                    width: 240,
                    height: 600,
                    guild_count: state.selected_guild().map(|_| 1).unwrap_or(0),
                    channel_count: state.selected_channel().map(|_| 1).unwrap_or(0),
                    member_count: 0,
                }
            }
        }

        let r = ChannelCounter;
        let mut state = SidePanelState::new();
        assert_eq!(r.view(&state).guild_count, 0);

        state.select_guild(1);
        state.select_channel(7);
        let layout = r.view(&state);
        assert_eq!(layout.guild_count, 1);
        assert_eq!(layout.channel_count, 1);
        assert!(layout.is_populated());
    }
}
