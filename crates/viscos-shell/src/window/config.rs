//! Shell and tray configuration types.
//!
//! `ShellConfig` — window + tray konfigürasyonu.
//! `TrayMenuItem`, `TrayMenu` — in-memory tray menü tipleri.

use viscos_webview::WindowConfig;

/// Shell konfigürasyonu (Faz 1.0'da `WebViewConfig` + window boyutları).
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// WebView backend konfigürasyonu (pencere boyutu, URL).
    pub window: WindowConfig,
    /// System tray aktif mi? Default: true.
    pub tray_enabled: bool,
    /// DevTools (F12) kısayolu aktif mi? Default: debug mode'da.
    pub devtools_enabled: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            tray_enabled: true,
            devtools_enabled: cfg!(debug_assertions),
        }
    }
}

/// Tray menü öğesi.
#[derive(Debug, Clone)]
pub enum TrayMenuItem {
    /// Statik etiket (tıklanamaz).
    Label(String),
    /// Tıklanabilir menü öğesi.
    Action {
        id: String,
        label: String,
        enabled: bool,
    },
    /// Ayırıcı.
    Separator,
}

/// Tray menüsü (Faz 1.0'da in-memory, gerçek `tao::menu::Menu` değil).
#[derive(Debug, Clone, Default)]
pub struct TrayMenu {
    items: Vec<TrayMenuItem>,
}

impl TrayMenu {
    /// Yeni boş tray menü.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Menü öğesi ekle.
    pub fn push(&mut self, item: TrayMenuItem) -> &mut Self {
        self.items.push(item);
        self
    }

    /// Menü öğelerini döndür (read-only).
    #[must_use]
    pub fn items(&self) -> &[TrayMenuItem] {
        &self.items
    }
}
