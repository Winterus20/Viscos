//! `WebView2Backend` stub — Faz 1.6'da `wry::WebViewBuilder` ile gerçek implementasyon.
//!
//! Faz 1.0 davranışı: `create_window` `ViscosError::Unimplemented` döner.
//! Ana binary (`crates/viscos/src/main.rs`) bu stub'ı kullanır ve "stub modunda" loglanır.
//!
//! Faz 1.6'da:
//! - `tao::event_loop::EventLoop` oluşturma
//! - `tao::window::WindowBuilder` → 1280x800 dark pencere
//! - `wry::WebViewBuilder::new(window).with_url(DISCORD_APP_URL).build()`
//! - `with_ipc_handler` ile `viscos-ipc` bridge bağlama
//! - `with_initialization_script(preload)` ile `frontend/dist/preload.js` injection
//!
//! ADR-0012 §1 referansı.

use viscos_error::{Result, ViscosError};

use crate::backend::{WebViewBackend, WebViewWindow, WindowConfig};

/// WebView2 backend (Microsoft Edge, OS-bundled).
///
/// Default backend (Faz 1.0). Faz 1.6'da `wry::WebViewBuilder` ile implementasyon.
#[derive(Debug, Clone, Default)]
pub struct WebView2Backend;

impl WebView2Backend {
    /// Yeni `WebView2Backend` instance'ı.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl WebViewBackend for WebView2Backend {
    fn create_window(&self, _config: WindowConfig) -> Result<Box<dyn WebViewWindow>> {
        // Faz 1.0: stub. Faz 1.6'da `tao::event_loop::EventLoop::new()` +
        // `WindowBuilder` + `wry::WebViewBuilder` ile gerçek WebView.
        Err(ViscosError::Unimplemented("phase-1.0 webview2 stub"))
    }

    fn name(&self) -> &'static str {
        "WebView2 (wry)"
    }

    fn version(&self) -> &'static str {
        "wry 0.55 stub"
    }

    fn known_issues(&self) -> &[&'static str] {
        // Upstream bug referansları (Faz 1.5 + Faz 1.6'da cross-referans olarak kullanılır).
        &[
            "WebView2Feedback#5536 — GDI object leak on mouse hover (Win11, STATE: OPEN)",
            "WebView2Feedback#5266 — RDP GDI region leak (STATE: OPEN)",
            "tauri-apps/wry#1691 — wrapper layer leak tracking",
            "tauri-apps/tauri#13758 — eval_script unmanaged lifecycle (mitigated by pull-based IPC)",
            "tauri-apps/tauri#13133 — channel callback memory leak (mitigated by delete onmessage)",
            "WebView2Feedback#5601 — Mouse drag main-thread starvation (Chromium 83+ regression)",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview2_backend_name_is_stable() {
        let backend = WebView2Backend::new();
        assert_eq!(backend.name(), "WebView2 (wry)");
    }

    #[test]
    fn webview2_create_window_returns_unimplemented_in_phase_1_0() {
        let backend = WebView2Backend::new();
        let result = backend.create_window(WindowConfig::default());
        match result {
            Err(ViscosError::Unimplemented(_)) => {}
            Err(other_err) => panic!("expected Unimplemented error, got {other_err:?}"),
            Ok(_) => panic!("expected error in Faz 1.0 stub, got Ok value"),
        }
    }

    #[test]
    fn webview2_known_issues_lists_gdi_leak() {
        let backend = WebView2Backend::new();
        let issues = backend.known_issues();
        assert!(!issues.is_empty(), "must document known issues");
        assert!(
            issues.iter().any(|i| i.contains("GDI")),
            "must mention the canonical GDI leak issue"
        );
    }
}
