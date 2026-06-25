//! `CefBackend` — CEF (Chromium Embedded Framework) backend.
//!
//! Faz 1.6 Dalga 1c (PR-6): **real `cef::BrowserHost::CreateBrowser` wiring** +
//! `wrap_app!` / `wrap_browser_process_handler!` / `wrap_client!` /
//! `wrap_life_span_handler!` macros.
//!
//! - **Default build:** `cef-backend` feature kapalı → `Media("cef backend not enabled")`.
//! - **Production build** (`--features viscos-webview/cef-backend`): DLL check
//!   + `cef::BrowserHost::CreateBrowser` + `CefWindow` handle.
//!
//! ADR-0012 §4 + Faz 1.6 Dalga 1b/c plan dosyaları.

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
use std::sync::{Arc, Mutex, OnceLock};

use viscos_error::{Result, ViscosError};

use crate::backend::{WebViewBackend, WebViewWindow, WindowConfig};

/// CEF (Chromium Embedded Framework) backend.
#[derive(Debug, Clone, Default)]
pub struct CefBackend {
    /// Optional explicit DLL yolu (config-driven, Faz 8.5 self-update).
    #[allow(dead_code)]
    runtime_dir: Option<std::path::PathBuf>,
}

impl CefBackend {
    #[must_use]
    pub const fn new() -> Self {
        Self { runtime_dir: None }
    }
    #[must_use]
    #[allow(dead_code)]
    pub fn with_runtime_dir(runtime_dir: std::path::PathBuf) -> Self {
        Self {
            runtime_dir: Some(runtime_dir),
        }
    }
}

/// CEF subprocess dispatch entry point — `main.rs`'ten çağrılır.
///
/// CEF multi-process mimarisinde ana binary subprocess dispatch için
/// `cef::execute_process` çağrısı yapar. Çağrı ana thread'in entry point'inde
/// `cef::initialize`'dan **önce** yapılmalıdır (CEF protokolü).
///
/// - Ana process: `cef::execute_process` `-1` döner → `None`.
/// - Subprocess (renderer/gpu/network): non-negative exit code → `Some(code)`.
/// - Feature kapalı: `None` (CEF runtime link edilmedi).
#[must_use]
pub fn execute_process_if_subprocess() -> Option<i32> {
    #[cfg(feature = "cef-backend")]
    {
        let main_args = cef_rs::MainArgs::default();
        let exit_code = cef_rs::execute_process(Some(&main_args), None, std::ptr::null_mut());
        if exit_code >= 0 {
            tracing::info!(
                exit_code,
                "CEF subprocess dispatch — main process terminating"
            );
            Some(exit_code)
        } else {
            None
        }
    }
    #[cfg(not(feature = "cef-backend"))]
    {
        None
    }
}

/// Subprocess routing sözleşme marker'ı (geriye uyumluluk).
#[must_use]
pub const fn cef_subprocess_main_marker() -> bool {
    false
}

impl WebViewBackend for CefBackend {
    fn create_window(
        &self,
        target: &tao::event_loop::EventLoopWindowTarget<()>,
        config: &WindowConfig,
    ) -> Result<Box<dyn WebViewWindow>> {
        #[cfg(all(feature = "cef-backend", target_os = "windows"))]
        {
            let dll_path = self.dll_path_or_error()?;
            check_cef_dll_present(&dll_path)?;
            create_cef_window(target, config)
        }
        #[cfg(not(all(feature = "cef-backend", target_os = "windows")))]
        {
            let _ = (target, config);
            Err(ViscosError::Media("cef backend not enabled (rebuild with --features viscos-webview/cef-backend; Windows-only runtime)".to_string()))
        }
    }

    fn name(&self) -> &'static str {
        "CEF (cef-rs)"
    }
    fn version(&self) -> &'static str {
        #[cfg(feature = "cef-backend")]
        {
            "cef-rs cef-v148.3.0+148.0.9"
        }
        #[cfg(not(feature = "cef-backend"))]
        {
            "cef-rs stub (feature off)"
        }
    }
    fn known_issues(&self) -> &[&'static str] {
        &[
            "cef-rs startup time 1.5-2.5s (Chromium initialization)",
            "Binary size 220-300 MB (Faz 8.5 self-update required)",
            "Idle RAM +50-100 MB vs WebView2",
            "Disk cache +150 MB (%APPDATA%/Viscos/cef-cache)",
            "Faz 1.6 PR-6: V8 bridge + crashpad out-of-scope (human release engineering); subprocess routing + BrowserHost::CreateBrowser wired",
        ]
    }
}

#[cfg(feature = "cef-backend")]
impl CefBackend {
    fn dll_path_or_error(&self) -> Result<std::path::PathBuf> {
        if let Some(dir) = &self.runtime_dir {
            return Ok(dir.clone());
        }
        let base = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| {
                ViscosError::Media("APPDATA env var not set (Windows-only runtime)".to_string())
            })?;
        Ok(base.join("Viscos").join("cef"))
    }
}

#[cfg(feature = "cef-backend")]
fn check_cef_dll_present(runtime_dir: &std::path::Path) -> Result<()> {
    let dll = runtime_dir.join("libcef.dll");
    if dll.is_file() {
        tracing::info!(path = %dll.display(), "CEF runtime DLL found");
        Ok(())
    } else {
        Err(ViscosError::Media(format!(
            "libcef.dll not found at {} (Faz 8.5 self-update not yet implemented)",
            dll.display()
        )))
    }
}

// =============================================================================
// Real CEF wiring — feature `cef-backend` + Windows-only
// =============================================================================

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
struct CefWindowState {
    hwnd: isize,
    width: i32,
    height: i32,
    initial_url: String,
    browser: Mutex<Option<cef_rs::Browser>>,
}

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
static CEF_INIT: OnceLock<Result<(), String>> = OnceLock::new();

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
fn create_cef_window(
    target: &tao::event_loop::EventLoopWindowTarget<()>,
    config: &WindowConfig,
) -> Result<Box<dyn WebViewWindow>> {
    use tao::dpi::LogicalSize;
    use tao::window::WindowBuilder;
    let tao_window = WindowBuilder::new()
        .with_title(&config.title)
        .with_inner_size(LogicalSize::new(
            f64::from(config.width),
            f64::from(config.height),
        ))
        .build(target)
        .map_err(|e| ViscosError::Media(format!("tao::Window build failed: {e}")))?;
    let hwnd = tao_window.hwnd().0 as isize;
    let state = Arc::new(CefWindowState {
        hwnd,
        width: i32::try_from(config.width).unwrap_or(i32::MAX),
        height: i32::try_from(config.height).unwrap_or(i32::MAX),
        initial_url: config.initial_url.clone(),
        browser: Mutex::new(None),
    });
    let cached = CEF_INIT.get_or_init(|| {
        let mut app = CefAppBuilder::build(state.clone());
        let args = cef_rs::args::Args::new();
        let settings = cef_rs::Settings {
            no_sandbox: 1,
            ..Default::default()
        };
        let ret = cef_rs::initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if ret == 1 {
            Ok(())
        } else {
            Err(format!(
                "cef::initialize returned {ret} (runtime missing or ABI mismatch)"
            ))
        }
    });
    cached
        .as_ref()
        .map(|_| tracing::info!("CEF initialized (idempotent gate)"))
        .map_err(|msg| ViscosError::Media(msg.clone()))?;
    tracing::info!(hwnd, url = %config.initial_url, "CEF window created");
    Ok(Box::new(CefWindow::new(state)))
}

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
mod cef_impl {
    use super::*;
    use cef_rs::{
        Browser, BrowserProcessHandler, BrowserSettings, CefString, Client, LifeSpanHandler, Rect,
        WindowInfo,
    };

    wrap_app! {
        pub struct CefAppBuilder { state: Arc<CefWindowState> }
        impl App {
            fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
                Some(CefBrowserProcessHandlerBuilder::build(self.state.clone()))
            }
        }
    }
    impl CefAppBuilder {
        pub fn build(state: Arc<CefWindowState>) -> cef_rs::App {
            Self::new(state)
        }
    }

    wrap_browser_process_handler! {
        pub struct CefBrowserProcessHandlerBuilder {
            state: Arc<CefWindowState>,
            client: std::cell::RefCell<Option<cef_rs::Client>>,
        }
        impl BrowserProcessHandler {
            fn on_context_initialized(&self) {
                // SAFETY: cef_dll_sys::HWND is a transparent wrapper around the
                // raw HWND pointer; tao::Window::hwnd().0 is `*mut c_void`.
                let parent_hwnd = cef_rs::sys::HWND(self.state.hwnd as *mut _);
                let bounds = Rect { x: 0, y: 0, width: self.state.width, height: self.state.height };
                let window_info = WindowInfo { ..Default::default() }.set_as_child(parent_hwnd, &bounds);
                let mut client_slot = self.client.borrow_mut();
                *client_slot = Some(CefClientBuilder::build(self.state.clone()));
                let url = CefString::from(self.state.initial_url.as_str());
                cef_rs::browser_host_create_browser(Some(&window_info), client_slot.as_mut(), Some(&url), Some(&BrowserSettings::default()), None, None);
            }
        }
    }
    impl CefBrowserProcessHandlerBuilder {
        pub fn build(state: Arc<CefWindowState>) -> cef_rs::BrowserProcessHandler {
            Self::new(state, std::cell::RefCell::new(None))
        }
    }

    wrap_client! {
        pub struct CefClientBuilder { state: Arc<CefWindowState> }
        impl Client {
            fn life_span_handler(&self) -> Option<LifeSpanHandler> {
                Some(CefLifeSpanHandlerBuilder::build(self.state.clone()))
            }
        }
    }
    impl CefClientBuilder {
        pub fn build(state: Arc<CefWindowState>) -> cef_rs::Client {
            Self::new(state)
        }
    }

    wrap_life_span_handler! {
        pub struct CefLifeSpanHandlerBuilder { state: Arc<CefWindowState> }
        impl LifeSpanHandler {
            fn on_after_created(&self, browser: Option<&mut Browser>) {
                if let Some(browser) = browser {
                    if let Ok(mut guard) = self.state.browser.lock() {
                        *guard = Some(browser.clone());
                        tracing::info!("CEF browser instance created (on_after_created)");
                    }
                }
            }
        }
    }
    impl CefLifeSpanHandlerBuilder {
        pub fn build(state: Arc<CefWindowState>) -> cef_rs::LifeSpanHandler {
            Self::new(state)
        }
    }
}

/// CEF-backed WebView window handle.
#[cfg(all(feature = "cef-backend", target_os = "windows"))]
pub struct CefWindow {
    id: u64,
    state: Arc<CefWindowState>,
}

// SAFETY: CEF `Browser` is reference-counted via `cef_rs::rc::RefGuard` (Send + Sync).
// Shared state is `Arc<CefWindowState>`. `eval` / `navigate` / `close` yalnızca
// tao event loop ana thread'inde dispatch edilir (WebViewWindow sözleşmesi).
#[cfg(all(feature = "cef-backend", target_os = "windows"))]
unsafe impl Send for CefWindow {}
#[cfg(all(feature = "cef-backend", target_os = "windows"))]
unsafe impl Sync for CefWindow {}

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
impl std::fmt::Debug for CefWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CefWindow")
            .field("id", &self.id)
            .field("hwnd", &self.state.hwnd)
            .finish_non_exhaustive()
    }
}

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
impl CefWindow {
    fn new(state: Arc<CefWindowState>) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            state,
        }
    }
    fn lock_browser(&self) -> Result<std::sync::MutexGuard<'_, Option<cef_rs::Browser>>> {
        self.state
            .browser
            .lock()
            .map_err(|err| ViscosError::Media(format!("CEF state mutex poisoned: {err}")))
    }
    fn not_ready_error() -> ViscosError {
        ViscosError::Media("CEF browser not yet created (await on_after_created)".to_string())
    }
}

#[cfg(all(feature = "cef-backend", target_os = "windows"))]
impl WebViewWindow for CefWindow {
    fn id(&self) -> u64 {
        self.id
    }
    fn eval(&self, script: &str) -> Result<()> {
        if script.len() > 10 * 1024 {
            tracing::warn!(
                size_bytes = script.len(),
                "eval_script payload > 10KB; consider SharedBuffer (Faz 4)"
            );
        }
        let guard = self.lock_browser()?;
        let browser = guard.as_ref().ok_or_else(Self::not_ready_error)?;
        let frame = browser
            .main_frame()
            .ok_or_else(|| ViscosError::Media("CEF main frame unavailable".to_string()))?;
        let code = cef_rs::CefString::from(script);
        frame.execute_java_script(Some(&code), None, 0);
        Ok(())
    }
    fn navigate(&self, url: &str) -> Result<()> {
        let guard = self.lock_browser()?;
        let browser = guard.as_ref().ok_or_else(Self::not_ready_error)?;
        let frame = browser
            .main_frame()
            .ok_or_else(|| ViscosError::Media("CEF main frame unavailable".to_string()))?;
        let target = cef_rs::CefString::from(url);
        frame.load_url(Some(&target));
        Ok(())
    }
    fn close(&self) -> Result<()> {
        let guard = self.lock_browser()?;
        if let Some(browser) = guard.as_ref() {
            if let Some(host) = browser.host() {
                host.close_browser(1);
            }
        }
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Pump CEF message loop work — call from your main event loop (~16ms for 60fps).
/// No-op when `cef-backend` feature is off.
pub fn pump_cef_message_loop() -> Result<()> {
    #[cfg(all(feature = "cef-backend", target_os = "windows"))]
    {
        cef_rs::do_message_loop_work();
    }
    Ok(())
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;

    #[test]
    fn cef_backend_name_is_stable() { assert_eq!(CefBackend::new().name(), "CEF (cef-rs)"); }

    #[test]
    fn cef_version_reflects_feature_gate() {
        #[cfg(feature = "cef-backend")]
        assert!(CefBackend::new().version().contains("cef-v148"));
        #[cfg(not(feature = "cef-backend"))]
        assert_eq!(CefBackend::new().version(), "cef-rs stub (feature off)");
    }

    #[test]
    fn cef_known_issues_marks_wiring_done() {
        let backend = CefBackend::new();
        let issues = backend.known_issues();
        assert!(issues.iter().any(|i| i.contains("BrowserHost::CreateBrowser")), "{issues:?}");
    }

    #[test]
    fn subprocess_marker_is_false() { assert!(!cef_subprocess_main_marker()); }

    #[test]
    fn pump_cef_message_loop_is_safe_no_op_when_feature_off() { assert!(pump_cef_message_loop().is_ok()); }
}
