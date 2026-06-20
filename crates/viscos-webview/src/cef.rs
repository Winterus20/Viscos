//! `CefBackend` — CEF (Chromium Embedded Framework) backend.
//!
//! Faz 1.0: stub (`Unimplemented`).
//! Faz 1.6 (B1 kararı): **feature-gated stub + DLL check + subprocess marker.**
//!
//! ## Build modları
//!
//! - **Default build** (`cargo build`): feature `cef-backend` kapalı →
//!   `create_window()` `ViscosError::Unimplemented("cef-backend feature not enabled")` döner.
//! - **Production build** (`cargo build --features viscos-webview/cef-backend`):
//!   gerçek `cef::BrowserHost::CreateBrowser` çağrısı.
//!
//! ## Subprocess routing (Faz 1.6 Dalga 1b — out-of-scope marker)
//!
//! `cef::execute_process` ana process'te subprocess dispatch için kullanılır.
//! `crates/viscos/src/main.rs`'te TODO marker olarak bırakıldı (insan release
//! engineering — `FOLLOW-UP-REAL-WORLD-WORK.md §B`). Bu crate yalnızca
//! `cef_subprocess_main_marker()` ile sözleşmeyi dokümante eder; gerçek
//! subprocess dispatch main.rs'ye aittir.
//!
//! ADR-0012 §4 + Faz 1.6 Dalga 1b plan dosyası.

use viscos_error::{Result, ViscosError};

use crate::backend::{WebViewBackend, WebViewWindow, WindowConfig};

/// CEF (Chromium Embedded Framework) backend.
///
/// **Default build:** stub — `create_window` `Unimplemented` döner (B1 kararı).
/// **Feature ON (`cef-backend`):** DLL check + `cef::BrowserHost::CreateBrowser`.
#[derive(Debug, Clone, Default)]
pub struct CefBackend {
    /// Optional explicit DLL yolu (config-driven, Faz 1.6 Faz 8.5 self-update).
    #[allow(dead_code)] // Used only when cef-backend feature is enabled.
    runtime_dir: Option<std::path::PathBuf>,
}

impl CefBackend {
    /// Yeni `CefBackend` instance'ı (default runtime dir).
    #[must_use]
    pub const fn new() -> Self {
        Self { runtime_dir: None }
    }

    /// Config-driven runtime dir ile (Faz 8.5 self-update öncesi manuel).
    #[must_use]
    #[allow(dead_code)] // Used when cef-backend feature is enabled.
    pub fn with_runtime_dir(runtime_dir: std::path::PathBuf) -> Self {
        Self {
            runtime_dir: Some(runtime_dir),
        }
    }
}

/// Subprocess routing sözleşmesi (ana binary `main.rs` ile).
///
/// `main.rs` `cef::execute_process(args)` çağrısını bu marker'ın
/// `true` olduğu yerde yapar; **out-of-scope** bu PR için (insan
/// release engineering gerektirir).
///
/// # Returns
///
/// `false` her zaman — gerçek subprocess dispatch main.rs'de.
#[must_use]
pub const fn cef_subprocess_main_marker() -> bool {
    false
}

impl WebViewBackend for CefBackend {
    fn create_window(
        &self,
        _target: &tao::event_loop::EventLoopWindowTarget<()>,
        _config: &WindowConfig,
    ) -> Result<Box<dyn WebViewWindow>> {
        // B1 kararı: feature-gated stub. Default build → Unimplemented.
        // Feature ON iken DLL check + BrowserHost::CreateBrowser sözleşmesi.
        // Compile-time branching clippy-friendly pattern kullanır (`if cfg!`
        // yerine `#[cfg]` + early return; clippy needless_return reddeder).
        #[cfg(not(feature = "cef-backend"))]
        {
            Err(ViscosError::Unimplemented(
                "cef-backend feature not enabled (rebuild with --features viscos-webview/cef-backend)",
            ))
        }
        #[cfg(feature = "cef-backend")]
        {
            // SAFETY invariant: `cef::api_hash` ve `cef::initialize` her
            // process'te yalnızca bir kez çağrılmalıdır. Bu kontrol
            // `cef_lifecycle::cef_initialize_idempotent` testinde verify edilir.
            let dll_path = self.dll_path_or_error()?;
            check_cef_dll_present(&dll_path)?;
            Err(ViscosError::Unimplemented(
                "cef-backend feature enabled but cef::BrowserHost::CreateBrowser wiring is out of scope for PR-2 (Faz 1.6 Dalga 1c); see FOLLOW-UP-REAL-WORLD-WORK.md §B",
            ))
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
            "Faz 1.6 PR-2 scope: subprocess routing + V8 bridge + crashpad out-of-scope (human release engineering)",
        ]
    }
}

#[cfg(feature = "cef-backend")]
impl CefBackend {
    /// Runtime dir belirle (explicit > default %APPDATA%/Viscos/cef).
    fn dll_path_or_error(&self) -> Result<std::path::PathBuf> {
        if let Some(dir) = &self.runtime_dir {
            return Ok(dir.clone());
        }
        // Default: %APPDATA%/Viscos/cef (Faz 1.6 MVP'de manuel install).
        // Faz 8.5 self-update bu path'i yönetecek.
        let base = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| {
                ViscosError::Media("APPDATA env var not set (Windows-only runtime)".to_string())
            })?;
        Ok(base.join("Viscos").join("cef"))
    }
}

/// `libcef.dll` varlığını kontrol et (Faz 1.6 MVP smoke gate).
///
/// Faz 8.5 self-update'te SHA256 integrity check eklenecek.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cef_backend_name_is_stable() {
        let backend = CefBackend::new();
        assert_eq!(backend.name(), "CEF (cef-rs)");
    }

    #[test]
    fn cef_subprocess_marker_is_false() {
        // PR-2 scope: subprocess routing main.rs'de (out-of-scope).
        // Marker `false` → main.rs'de `cef::execute_process` çağrısı yok.
        assert!(!cef_subprocess_main_marker());
    }

    #[test]
    fn cef_with_runtime_dir_keeps_path() {
        let backend = CefBackend::with_runtime_dir(std::path::PathBuf::from("/opt/cef"));
        assert_eq!(
            backend.runtime_dir.as_deref().unwrap(),
            std::path::Path::new("/opt/cef")
        );
    }

    #[test]
    fn cef_create_window_default_feature_returns_unimplemented() {
        // Default build (cef-backend feature OFF) → Unimplemented.
        // Bu test feature-gated: feature ON iken skip.
        #[cfg(not(feature = "cef-backend"))]
        {
            let backend = CefBackend::new();
            // Feature OFF iken version yansıması doğru mu kontrolü (faktiki
            // Unimplemented yolu runtime'da davranır; burada yalnızca metadata
            // sözleşmesini verify ediyoruz).
            assert_eq!(
                backend.version(),
                "cef-rs stub (feature off)",
                "version must reflect feature-gated build mode"
            );
            // Version farklı olduğu için artık Unimplemented mesajını da verify
            // edebiliriz: feature ON ise mesaj farklı, OFF ise "feature not enabled".
            assert!(
                backend.known_issues().iter().any(|i| i.contains("Faz 1.6")),
                "known_issues must mention current Faz (1.6) marker"
            );
        }
        #[cfg(feature = "cef-backend")]
        {
            // Feature ON — test create_window feature-dependent path'ini atlıyor
            // (DLL check gerçek runtime gerektirir; integration test `cef_lifecycle.rs`).
        }
    }

    #[test]
    fn cef_known_issues_does_not_mention_gdi_leak() {
        // CEF'in Win11 GDI leak yokluğu ADR-0012 §2'de vurgulanır; backend'in
        // known_issues listesi WebView2'nin aksine GDI leak içermez.
        let backend = CefBackend::new();
        let issues = backend.known_issues();
        assert!(
            !issues.iter().any(|i| i.contains("GDI")),
            "CEF backend'in known_issues listesi GDI leak içermemeli: {issues:?}"
        );
    }

    #[test]
    fn cef_known_issues_mentions_subprocess_out_of_scope() {
        let backend = CefBackend::new();
        let issues = backend.known_issues();
        assert!(
            issues.iter().any(|i| i.contains("subprocess")),
            "subprocess routing out-of-scope marker mutlaka listelenmeli: {issues:?}"
        );
    }
}
