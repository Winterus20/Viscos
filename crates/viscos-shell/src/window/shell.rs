//! `Shell` struct, `ShellBuilder`, and `ResizeObserver`.
//!
//! `Shell` is the Faz 1.0 stub of the tao event loop handle.
//! `ShellBuilder` provides a fluent API for constructing a `Shell`.
//! `ResizeObserver` is a placeholder frame-timing probe (Faz 1.5 will add real metrics).

use viscos_webview::WindowConfig;

use super::config::{ShellConfig, TrayMenu};
use super::tray::default_tray_menu;

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
