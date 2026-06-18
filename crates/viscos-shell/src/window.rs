//! `Shell` — tao event loop + tray icon + resize observer (Faz 1.0 stub).
//!
//! Faz 1.0'da `tao::event_loop::EventLoop::new()` çağrısı yapılmaz (CI'da
//! GUI loop gerekmez; sadece tip tanımları + tray menu builder expose edilir).
//! Faz 1.6'da `Shell::run()` gerçek event loop'u başlatacak.
//!
//! Cross-references:
//! - [`phase-1.0-window-webview.md` §3.1](../../.cursor/plans/phase-1.0-window-webview.md#31-viscos-shell)
//! - [`phase-1.5-telemetry-and-restart-optimization.md`] (tray badge)

use viscos_core::VISCOS_VERSION;
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

/// Default tray menü — status + quit.
///
/// Faz 1.0 stub: `tao::tray::TrayIconBuilder` ile gerçek inşa Faz 1.6'da.
#[must_use]
pub fn default_tray_menu() -> TrayMenu {
    let mut menu = TrayMenu::new();
    menu.push(TrayMenuItem::Label(format!("Viscos v{VISCOS_VERSION}")));
    menu.push(TrayMenuItem::Separator);
    menu.push(TrayMenuItem::Action {
        id: "status".to_string(),
        label: "Online".to_string(),
        enabled: false,
    });
    menu.push(TrayMenuItem::Separator);
    menu.push(TrayMenuItem::Action {
        id: "quit".to_string(),
        label: "Quit".to_string(),
        enabled: true,
    });
    menu
}

/// Resize davranışı gözlemcisi (placeholder).
///
/// Faz 1.0'da sadece struct + method imzaları. Faz 1.5 telemetry'sinde
/// actual frame time / lag metric'leri toplanacak.
#[derive(Debug, Clone, Default)]
pub struct ResizeObserver;

impl ResizeObserver {
    /// Yeni resize observer oluştur.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Frame time ölçümü (microseconds). Stub: sabit 16_667µs (60 FPS).
    #[must_use]
    pub const fn frame_time_us(&self) -> u64 {
        16_667
    }

    /// Resize laggy mi? Stub: false (gerçek ölçüm Faz 1.5'te).
    #[must_use]
    pub const fn is_laggy(&self) -> bool {
        false
    }
}

/// `Shell` handle (Faz 1.0 stub).
///
/// Faz 1.6'da `tao::event_loop::EventLoop::new()` + `WindowBuilder` +
/// `TrayIconBuilder` ile gerçek implementasyon.
#[derive(Debug)]
pub struct Shell {
    config: ShellConfig,
    tray_menu: TrayMenu,
    resize_observer: ResizeObserver,
}

impl Shell {
    /// Yeni `Shell` instance'ı.
    #[must_use]
    pub fn new(config: ShellConfig) -> Self {
        Self {
            config,
            tray_menu: default_tray_menu(),
            resize_observer: ResizeObserver::new(),
        }
    }

    /// Shell konfigürasyonu (read-only).
    #[must_use]
    pub const fn config(&self) -> &ShellConfig {
        &self.config
    }

    /// Tray menü (read-only).
    #[must_use]
    pub const fn tray_menu(&self) -> &TrayMenu {
        &self.tray_menu
    }

    /// Resize observer (read-only).
    #[must_use]
    pub const fn resize_observer(&self) -> &ResizeObserver {
        &self.resize_observer
    }

    /// Event loop'u başlat (Faz 1.0 stub).
    ///
    /// Faz 1.0'da bu method `EventLoop::run()` çağırmaz; sadece konfigürasyon
    /// doğrulaması yapar ve "Shell ready" loglanır.
    ///
    /// Faz 1.6'da `tao::event_loop::EventLoop::new()` + run + window attach.
    ///
    /// # Errors
    ///
    /// Faz 1.0'da her zaman OK (sadece konfigürasyon sanity check).
    pub fn run(&self) -> anyhow::Result<()> {
        tracing::info!(
            title = %self.config.window.title,
            width = self.config.window.width,
            height = self.config.window.height,
            tray_enabled = self.config.tray_enabled,
            devtools_enabled = self.config.devtools_enabled,
            "Shell ready (Faz 1.0 stub — event loop will start in Faz 1.6)"
        );
        Ok(())
    }
}

/// `Shell` builder (fluent API).
#[derive(Debug, Default)]
pub struct ShellBuilder {
    config: ShellConfig,
}

impl ShellBuilder {
    /// Yeni builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Window config set.
    #[must_use]
    pub fn window(mut self, window: WindowConfig) -> Self {
        self.config.window = window;
        self
    }

    /// Tray toggle.
    #[must_use]
    pub const fn tray_enabled(mut self, enabled: bool) -> Self {
        self.config.tray_enabled = enabled;
        self
    }

    /// DevTools toggle.
    #[must_use]
    pub const fn devtools_enabled(mut self, enabled: bool) -> Self {
        self.config.devtools_enabled = enabled;
        self
    }

    /// Shell oluştur.
    #[must_use]
    pub fn build(self) -> Shell {
        Shell::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tray_menu_has_status_and_quit() {
        let menu = default_tray_menu();
        let items = menu.items();
        assert!(
            items
                .iter()
                .any(|i| matches!(i, TrayMenuItem::Action { id, .. } if id == "status"))
        );
        assert!(
            items
                .iter()
                .any(|i| matches!(i, TrayMenuItem::Action { id, .. } if id == "quit"))
        );
        assert!(items.iter().any(|i| matches!(i, TrayMenuItem::Separator)));
    }

    #[test]
    fn default_tray_menu_first_item_is_version_label() {
        let menu = default_tray_menu();
        let first = menu.items().first().expect("at least one item");
        match first {
            TrayMenuItem::Label(s) => assert!(s.starts_with("Viscos v")),
            _ => panic!("expected Label, got {first:?}"),
        }
    }

    #[test]
    fn shell_config_default_has_dark_theme() {
        let cfg = ShellConfig::default();
        assert_eq!(cfg.window.theme, "dark");
        assert_eq!(cfg.window.title, "Viscos");
        assert!(cfg.tray_enabled);
    }

    #[test]
    fn shell_builder_fluent_api() {
        let shell = ShellBuilder::new()
            .tray_enabled(false)
            .devtools_enabled(true)
            .build();
        assert!(!shell.config().tray_enabled);
        assert!(shell.config().devtools_enabled);
    }

    #[test]
    fn shell_run_succeeds_in_phase_1_0() {
        let shell = ShellBuilder::new().build();
        assert!(shell.run().is_ok());
    }

    #[test]
    fn resize_observer_stub_returns_constants() {
        let obs = ResizeObserver::new();
        assert_eq!(obs.frame_time_us(), 16_667);
        assert!(!obs.is_laggy());
    }

    #[test]
    fn tray_menu_push_returns_mut_ref() {
        let mut menu = TrayMenu::new();
        menu.push(TrayMenuItem::Separator)
            .push(TrayMenuItem::Label("x".into()));
        assert_eq!(menu.items().len(), 2);
    }
}
