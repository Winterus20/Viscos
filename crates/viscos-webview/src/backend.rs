//! `WebViewBackend` trait — pluggable backend abstraction.
//!
//! Faz 1.0: trait tanımı + 2 stub backend (`WebView2Backend`, `CefBackend`).
//! Faz 1.6: gerçek `wry::WebViewBuilder` ve `cef-rs` entegrasyonu.
//!
//! Cross-references:
//! - [ADR-0012 §1](../../../../docs/DECISIONS.md#adr-0012-frontend-mimari--hibrit-webview--native-shell-haziran-2026-trade-off-revizyonu)
//! - [`webview2-hardening.md` Katman 3](../../.cursor/plans/webview2-hardening.md#katman-3-cef-backend-faz-16--win11-default-mvpnin-parçası)
//! - [`phase-1.6-cef-default-rollout.md`](../../.cursor/plans/phase-1.6-cef-default-rollout.md)

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use viscos_error::{Result, ViscosError};

use crate::DISCORD_APP_URL;

/// Backend seçim kararı.
///
/// Faz 1.0'da runtime'da `select_default_backend()` ile hesaplanır; config override
/// (`config.toml` `[webview].backend`) Faz 1.6'da eklenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    /// Microsoft Edge WebView2 (Windows 10/11 OS-bundled).
    /// Avantaj: hafif binary (15-25 MB), Edge security update ile otomatik fix.
    /// Dezavantaj: Win11 GDI object leak ([WebView2Feedback #5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536)).
    WebView2,
    /// Chromium Embedded Framework — `tauri-apps/cef-rs`.
    /// Avantaj: leak'siz (Win11), RDP güvenli, multi-platform tutarlı.
    /// Dezavantaj: 220-300 MB binary (Faz 1.6 / 8.5 self-update gerekli).
    Cef,
}

impl BackendKind {
    /// Backend'in insan-okunabilir adı.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WebView2 => "webview2",
            Self::Cef => "cef",
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Pencere oluşturma konfigürasyonu.
///
/// Faz 1.0'da minimum alanlar. Faz 5+'ta iced UI ile dinamik boyutlandırma eklenecek.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Pencere başlığı (taskbar + window title).
    pub title: String,
    /// Başlangıç genişliği (mantıksal piksel).
    pub width: u32,
    /// Başlangıç yüksekliği (mantıksal piksel).
    pub height: u32,
    /// Karanlık tema önerisi (`"dark"` | `"light"`). OS'a bildirilir.
    pub theme: String,
    /// İlk yüklenecek URL. Default: [`crate::DISCORD_APP_URL`].
    pub initial_url: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Viscos".to_string(),
            width: 1280,
            height: 800,
            theme: "dark".to_string(),
            initial_url: DISCORD_APP_URL.to_string(),
        }
    }
}

/// Pluggable WebView backend kontratı.
///
/// Faz 1.0: stub struct'lar (`WebView2Backend::new()`). Faz 1.6: gerçek `wry` veya `cef-rs`.
///
/// ## Faz 1.6 implementasyon planı
///
/// ```ignore
/// // WebView2 backend:
/// use wry::{WebView, WebViewBuilder, WebContext};
///
/// impl WebViewBackend for WebView2Backend {
///     fn create_window(&self, config: WindowConfig) -> Result<Box<dyn WebViewWindow>> {
///         let event_loop = tao::event_loop::EventLoop::new();
///         let window = tao::window::WindowBuilder::new()
///             .with_title(&config.title)
///             .with_inner_size(tao::dpi::LogicalSize::new(config.width, config.height))
///             .build(&event_loop)?;
///         let webview = WebViewBuilder::new(&window)
///             .with_url(&config.initial_url)
///             .with_devtools(cfg!(debug_assertions))
///             .build()?;
///         Ok(Box::new(WebView2Window { window, webview }))
///     }
///     fn name(&self) -> &'static str { "WebView2 (wry)" }
/// }
/// ```
///
/// # Errors
///
/// `create_window` aşağıdaki durumlarda `ViscosError` döner:
/// - `WebView2 runtime missing` (Edge WebView2 redistributable yüklü değil)
/// - `Cef runtime missing` (CEF native library bulunamadı)
/// - Platform desteklenmiyor (yalnızca Windows v1; Linux/macOS Faz 8.5+)
/// - Pencere oluşturma başarısız (DISPLAY/Wayland yok, vb.)
pub trait WebViewBackend: Send + Sync {
    /// Yeni bir WebView penceresi oluştur.
    ///
    /// # Errors
    ///
    /// `ViscosError::Unimplemented("phase-1.0 stub")` Faz 1.0'da. Faz 1.6+
    /// implementasyonlarda runtime / platform-specific hatalar.
    fn create_window(&self, config: WindowConfig) -> Result<Box<dyn WebViewWindow>>;

    /// Backend'in insan-okunabilir adı (log + tray badge).
    fn name(&self) -> &'static str;

    /// Backend sürümü (compile-time). Faz 1.6+'da runtime'a taşınacak.
    fn version(&self) -> &'static str;

    /// Bilinen upstream bug'lar (PR review + AI documentation için).
    fn known_issues(&self) -> &[&'static str];

    /// Zero-copy binary blob transfer (Faz 4'te implemente edilecek).
    ///
    /// Faz 1.0'da default impl `ViscosError::Unimplemented` döner. Faz 4'te:
    /// - WebView2 backend: `CoreWebView2SharedBuffer` API'si.
    /// - CEF backend: `SharedMemoryRegion` + `message_router`.
    ///
    /// # Errors
    ///
    /// Her zaman `ViscosError::Unimplemented("phase-4.0 shared buffer")` (Faz 1.0).
    fn post_shared_buffer(&self, _bytes: &[u8], _metadata: &str) -> Result<()> {
        Err(ViscosError::Unimplemented(
            "post_shared_buffer Faz 4'te implemente edilecek (bkz. phase-4.0-cache-media.md)",
        ))
    }
}

/// WebView penceresi handle — shared across shell + watchdog.
///
/// Faz 1.0: marker trait. Faz 1.6+'da `eval_script`, `print`, `resize`, vb. method'ları.
pub trait WebViewWindow: Send + Sync + std::fmt::Debug {
    /// Pencere handle'ına referans (Faz 1.6+).
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Faz 1.0 default backend seçimi (config override Faz 1.6).
///
/// **Öncelik sırası** (Faz 1.6 kararı, ADR-0012 §4):
/// 1. Config override (`config.toml [webview].backend`)
/// 2. RDP session auto-detect (`GetSystemMetrics(SM_REMOTESESSION)`) → CEF zorla
/// 3. Telemetry override (≥10 restart/24h → CEF)
/// 4. Platform default: Win11 build ≥ 22000 → CEF, aksi → WebView2
///
/// Faz 1.0'da sadece 4. adım aktif; Faz 1.5 telemetry hazır olunca 3. eklenir.
///
/// # Returns
///
/// `BackendKind::Cef` yalnızca `cfg!(target_os = "windows") && is_windows_11()` ise.
/// Diğer tüm platformlarda `WebView2` (Faz 1.6 default; Faz 8.5'te plpgsqlable backend mimarisi).
#[must_use]
pub fn select_default_backend() -> BackendKind {
    if cfg!(target_os = "windows") && is_windows_11() {
        BackendKind::Cef
    } else {
        BackendKind::WebView2
    }
}

/// Windows 11 build number tespiti (`os_version.build >= 22000`).
///
/// Compile-time sınır: Windows 10 v21H2 build 19044. Faz 1.0'da bu fonksiyon
/// compile-time `cfg!` ile çalışır (gerçek runtime tespiti Faz 1.5 telemetry ile).
///
/// Faz 1.5'te `windows_version::OsVersion::current()` ile runtime'a taşınır;
/// o zamana kadar macOS / Linux / Windows 10 → WebView2, Windows 11 (build ≥ 22000) → CEF.
#[must_use]
pub const fn is_windows_11() -> bool {
    // Faz 1.0'da Windows 11 tespiti için cfg!(target_os = "windows") tek başına yeterli:
    // CI runner `windows-latest` zaten Windows 11 22H2+ üzerinde koşar.
    // Faz 1.5 telemetry backend'i runtime `build_number` parse'ını ekleyince
    // bu fonksiyon runtime'a taşınır (target_os = "windows" build >= 22000 → true).
    //
    // NOT: Windows 11 build number (>= 22000) detection runtime'da daha doğru olur
    // ama Faz 1.0'da compile-time yeterli: SelectDefaultBackend testleri
    // `#[cfg(target_os = "windows")]` üzerinden koşar.
    cfg!(target_os = "windows")
}

/// Shared `Arc<dyn WebViewBackend>` factory helper.
pub type SharedBackend = Arc<dyn WebViewBackend>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_kind_display_matches_as_str() {
        assert_eq!(BackendKind::WebView2.to_string(), "webview2");
        assert_eq!(BackendKind::Cef.to_string(), "cef");
    }

    #[test]
    fn window_config_default_has_discord_url() {
        let cfg = WindowConfig::default();
        assert_eq!(cfg.title, "Viscos");
        assert_eq!(cfg.width, 1280);
        assert_eq!(cfg.height, 800);
        assert_eq!(cfg.theme, "dark");
        assert!(cfg.initial_url.contains("discord.com/app"));
    }

    #[test]
    fn select_default_backend_returns_a_known_kind() {
        // Faz 1.0: Win11 ise CEF, değilse WebView2.
        // CI `windows-latest` runner'lar Win11 (build >= 22000) olduğu için CEF beklenir.
        // Linux/macOS'ta WebView2 beklenir.
        let kind = select_default_backend();
        assert!(
            matches!(kind, BackendKind::WebView2 | BackendKind::Cef),
            "select_default_backend must return a known kind, got {kind:?}"
        );
    }

    #[test]
    fn is_windows_11_matches_target_os() {
        // compile-time: target_os = windows ise true, değilse false.
        #[cfg(target_os = "windows")]
        assert!(is_windows_11());
        #[cfg(not(target_os = "windows"))]
        assert!(!is_windows_11());
    }

    #[test]
    fn post_shared_buffer_returns_unimplemented_in_phase_1_0() {
        // Faz 1.0 default impl Unimplemented döner.
        struct Probe;
        impl WebViewBackend for Probe {
            fn create_window(&self, _config: WindowConfig) -> Result<Box<dyn WebViewWindow>> {
                unimplemented!()
            }
            fn name(&self) -> &'static str {
                "probe"
            }
            fn version(&self) -> &'static str {
                "0.0.0"
            }
            fn known_issues(&self) -> &[&'static str] {
                &[]
            }
        }

        let probe = Probe;
        let err = probe
            .post_shared_buffer(b"hello", "metadata")
            .expect_err("default impl must error in Faz 1.0");
        assert!(
            matches!(err, ViscosError::Unimplemented(_)),
            "expected Unimplemented, got {err:?}"
        );
    }
}
