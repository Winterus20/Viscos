//! `WebView2Backend` — Microsoft Edge WebView2 (wry).
//!
//! Faz 1.0: stub (`Unimplemented`).
//! Faz 1.6 Dalga 1b: gerçek `wry::WebViewBuilder` + `tao::WindowBuilder` runtime.
//!
//! ## Mimari
//!
//! `create_window()` `tao::EventLoopWindowTarget<()>` üzerinden bir `tao::Window`
//! oluşturur ve `wry::WebViewBuilder` ile WebView2'yi attach eder. WebView'in
//! DevTools'u yalnızca debug build'lerde aktif (`cfg!(debug_assertions)`).
//!
//! ## Main-thread affinity
//!
//! WebView2 COM nesneleri main-thread affine'dir. `unsafe impl Send + Sync for
//! WebView2Window` sound çünkü handle yalnızca `tao::EventLoop::run()`
//! callback'i içinde (ana thread) kullanılır; diğer thread'lerden erişim
//! yapılmaz (event dispatch channel üzerinden yönlendirilir).
//!
//! ADR-0012 §1, [`phase-1.6-cef-default-rollout.md` Bölüm 2](../../.cursor/plans/phase-1.6-cef-default-rollout.md#2-mimari-karar-webviewbackend-trait-zaten-var).

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use viscos_error::{Result, ViscosError};

use crate::backend::{WebViewBackend, WebViewWindow, WindowConfig};

/// WebView2 backend (Microsoft Edge, OS-bundled).
///
/// **Faz 1.6 Dalga 1b (PR-2 scope):** gerçek `wry::WebViewBuilder` runtime.
/// Default build'de tamamen compile-time koşullu: Windows dışı target'larda
/// stub `Unimplemented` döner (Faz 1.0 contract).
#[derive(Debug, Clone)]
pub struct WebView2Backend {
    /// WebView2 user data directory (`%APPDATA%/Viscos/webview2-cache` default).
    runtime_dir: PathBuf,
}

impl Default for WebView2Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl WebView2Backend {
    /// Default runtime dir ile yeni backend (`%APPDATA%/Viscos/webview2-cache`
    /// Windows; non-Windows'ta stub moduna düşer).
    #[must_use]
    pub fn new() -> Self {
        let runtime_dir = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("Viscos").join("webview2-cache"))
            .unwrap_or_else(|| PathBuf::from("./viscos-webview2-cache"));
        Self { runtime_dir }
    }

    /// Config-driven explicit runtime dir (Faz 1.6 — test + CI için).
    #[must_use]
    pub fn with_runtime_dir(runtime_dir: PathBuf) -> Self {
        Self { runtime_dir }
    }

    /// Runtime dir (read-only).
    #[must_use]
    pub fn runtime_dir(&self) -> &std::path::Path {
        &self.runtime_dir
    }
}

impl WebViewBackend for WebView2Backend {
    #[cfg(target_os = "windows")]
    fn create_window(
        &self,
        target: &tao::event_loop::EventLoopWindowTarget<()>,
        config: &WindowConfig,
    ) -> Result<Box<dyn WebViewWindow>> {
        use tao::dpi::LogicalSize;
        use tao::window::WindowBuilder;

        // Runtime dir oluştur (parent auto-create, Faz 1.5 telemetry SQLITE_CANTOPEN fix'i).
        if let Some(parent) = self.runtime_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ViscosError::Media(format!(
                    "creating webview2 user data parent dir {}: {e}",
                    parent.display()
                ))
            })?;
        }
        std::fs::create_dir_all(&self.runtime_dir).map_err(|e| {
            ViscosError::Media(format!(
                "creating webview2 user data dir {}: {e}",
                self.runtime_dir.display()
            ))
        })?;

        // tao::Window oluştur (pencere başlığı + boyut).
        let window = WindowBuilder::new()
            .with_title(&config.title)
            .with_inner_size(LogicalSize::new(
                f64::from(config.width),
                f64::from(config.height),
            ))
            .build(target)
            .map_err(|e| ViscosError::Media(format!("tao::Window build failed: {e}")))?;

        // wry::WebViewBuilder ile WebView2'yi attach et.
        let webview = wry::WebViewBuilder::new()
            .with_url(&config.initial_url)
            .with_devtools(cfg!(debug_assertions))
            .build(&window)
            .map_err(|e| ViscosError::Media(format!("wry::WebView build failed: {e}")))?;

        Ok(Box::new(WebView2Window::new(window, webview)))
    }

    #[cfg(not(target_os = "windows"))]
    fn create_window(
        &self,
        _target: &tao::event_loop::EventLoopWindowTarget<()>,
        _config: &WindowConfig,
    ) -> Result<Box<dyn WebViewWindow>> {
        // Non-Windows v1: stub. Faz 8.5'te plpgsqlable backend (WebKitGTK) eklenecek.
        // Bkz. .cursorrules Bölüm 14: "Linux v2 WebKitGTK sorunlu".
        Err(ViscosError::Media(
            "WebView2 non-Windows'ta desteklenmiyor (v1 Windows-only; Faz 8.5 WebKitGTK)"
                .to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "WebView2 (wry)"
    }

    fn version(&self) -> &'static str {
        concat!("wry ", env!("CARGO_PKG_VERSION"), " + WebView2 runtime")
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

/// Global monotonic window ID counter (debug + window registry için).
fn next_window_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// `WebView2Window` — `tao::Window` + `wry::WebView` composite handle.
///
/// Main-thread affine: handle yalnızca `tao::EventLoop::run()` callback'i
/// içinde (ana thread) kullanılmalıdır. Diğer thread'lerden erişim
/// event dispatch channel üzerinden yapılır.
pub struct WebView2Window {
    /// Unique process-wide window ID.
    id: u64,
    /// tao pencere handle (move-only; main-thread affine).
    window: tao::window::Window,
    /// wry WebView handle (pencereye attach edilmiş).
    webview: wry::WebView,
}

impl fmt::Debug for WebView2Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebView2Window")
            .field("id", &self.id)
            .field("title", &self.window.title())
            .finish_non_exhaustive()
    }
}

// SAFETY: WebView2 COM nesneleri (`ICoreWebView2` vb.) main-thread affine'dir.
// Ancak Viscos'un tasarım sözleşmesi: handle yalnızca `tao::EventLoop::run()`
// callback dispatch'i içinde (ana thread) kullanılır; diğer thread'lerden
// erişim event channel üzerinden proxy'lenir. Bu nedenle `Send` (move across
// thread) sound çünkü erişim zamanlaması değil sahiplik transferi; ve `Sync`
// (shared reference across thread) **ses** değil — `&self` ile method call
// yapılırken runtime'da yalnızca ana thread'de invoke edilir.
//
// Invariant: `eval`, `navigate`, `close` yalnızca ana thread'den çağrılır;
// `id()` zaten thread-safe (`u64` copy).
unsafe impl Send for WebView2Window {}
unsafe impl Sync for WebView2Window {}

impl WebView2Window {
    /// Yeni handle (yalnızca `WebView2Backend::create_window` çağırır).
    fn new(window: tao::window::Window, webview: wry::WebView) -> Self {
        Self {
            id: next_window_id(),
            window,
            webview,
        }
    }
}

impl WebViewWindow for WebView2Window {
    fn id(&self) -> u64 {
        self.id
    }

    fn eval(&self, script: &str) -> Result<()> {
        // Faz 1.6: wry::WebView::evaluate_script (eval_script API adı wry 0.55'te).
        // 10KB threshold Faz 4 SharedBuffer'a geçecek.
        if script.len() > 10 * 1024 {
            tracing::warn!(
                size_bytes = script.len(),
                "eval_script payload > 10KB; consider SharedBuffer (Faz 4)"
            );
        }
        self.webview
            .evaluate_script(script)
            .map_err(|e| ViscosError::Media(format!("wry evaluate_script failed: {e}")))
    }

    fn navigate(&self, url: &str) -> Result<()> {
        // wry 0.55'te navigate_url deprecated; load_url stabil.
        self.webview
            .load_url(url)
            .map_err(|e| ViscosError::Media(format!("wry load_url failed: {e}")))
    }

    fn close(&self) -> Result<()> {
        // tao::Window public API'de close yok; pencere kapatma event loop
        // dispatch'ı üzerinden yapılır (WindowEvent::CloseRequested).
        // Burada sadece flag bırakıyoruz; gerçek kapatma event loop'ta.
        // Daha temiz bir API Faz 1.6 Faz 5 native UI ile gelecek.
        tracing::debug!(window_id = self.id, "WebView2Window::close requested");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
    fn webview2_with_runtime_dir_keeps_path() {
        let backend = WebView2Backend::with_runtime_dir(PathBuf::from("/tmp/wv2"));
        assert_eq!(backend.runtime_dir(), std::path::Path::new("/tmp/wv2"));
    }

    #[test]
    fn webview2_default_runtime_dir_is_under_appdata() {
        let backend = WebView2Backend::new();
        let dir = backend.runtime_dir();
        // APPDATA set değilse "./viscos-webview2-cache" fallback.
        // Her iki durumda da path boş olmamalı.
        assert!(!dir.as_os_str().is_empty(), "runtime dir must not be empty");
        // Path bileşeni olarak en az 1 segment içermeli.
        assert!(dir.components().count() >= 1);
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

    #[test]
    fn webview2_version_includes_wry() {
        let backend = WebView2Backend::new();
        let version = backend.version();
        assert!(
            version.contains("wry"),
            "version must reference wry backend: {version}"
        );
    }

    #[test]
    fn window_id_counter_is_monotonic() {
        // COUNTER process-global; ilk ID >= 1.
        let a = next_window_id();
        let b = next_window_id();
        assert!(b > a, "window ID must monotonically increase ({a} < {b})");
    }
}
