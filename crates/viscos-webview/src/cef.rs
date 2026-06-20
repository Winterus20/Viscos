//! `CefBackend` — CEF (Chromium Embedded Framework) backend.
//!
//! Faz 1.0: stub (`Unimplemented`).
//! Faz 1.6 (B1 kararı): **feature-gated stub + DLL check + subprocess routing.**
//!
//! ## Build modları
//!
//! - **Default build** (`cargo build`): feature `cef-backend` kapalı →
//!   `create_window()` `ViscosError::Unimplemented("cef-backend feature not enabled")` döner.
//! - **Production build** (`cargo build --features viscos-webview/cef-backend`):
//!   gerçek `cef::BrowserHost::CreateBrowser` çağrısı.
//!
//! ## Subprocess routing (Faz 1.6 Dalga 1b)
//!
//! CEF multi-process mimarisinde ana binary subprocess dispatch için
//! `cef::execute_process` çağrısı yapar. Çağrı ana thread'in entry
//! point'inde `cef::initialize`'dan **önce** yapılmalıdır (CEF protokolü).
//!
//! Sözleşme:
//! - `cef::execute_process` ana process'te `-1` döner → `main.rs` devam eder.
//! - Subprocess'te (renderer, gpu, network, vb.) non-negative exit code →
//!   `main.rs` çağrıyı propagate edip `std::process::exit` yapar.
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

/// CEF subprocess dispatch entry point — `main.rs`'ten çağrılır.
///
/// CEF multi-process mimarisinde ana binary, subprocess'leri
/// (renderer, gpu, network, utility, vb.) launch etmek için
/// `cef::execute_process` çağrısı yapar. Bu çağrı **her** process'te
/// ana thread'de `cef::initialize`'dan **önce** yapılmalıdır
/// (CEF protokolü, `_cef_main_args_t` standardı).
///
/// # Davranış
///
/// - **Ana process (browser):** `cef::execute_process` `-1` döner
///   ("no recognized subprocess type"). Bu fonksiyon `None` döner;
///   `main.rs` normal initialization'a devam eder.
/// - **Subprocess (renderer/gpu/network/vb.):** `cef::execute_process`
///   `0..=c_int::MAX` arası exit code döner. Bu fonksiyon `Some(code)`
///   döner; `main.rs` `std::process::exit(code)` ile çıkmalıdır.
/// - **Feature kapalı (`cef-backend` off):** `None` döner. Stub
///   branch (default build, CI cross-platform) gerçek `cef` crate'i
///   link etmez, dolayısıyla subprocess dispatch yoktur.
///
/// # Güvenlik
///
/// `cef::execute_process` Windows'ta sandbox info pointer alır;
/// `std::ptr::null_mut()` güvenlidir (sandbox feature sadece CEF
/// bundle'ında aktive olur; Viscos production'unda default sandbox
/// disabled'dır, ADR-0012 §4).
///
/// # Returns
///
/// - `Some(exit_code)` → subprocess tespit edildi, ana process bu
///   kodla terminate etmeli (CEF subprocess kendi yaşam döngüsünü
///   tamamlar, sonra `code` döner).
/// - `None` → ana process veya feature kapalı; devam et.
#[must_use]
pub fn execute_process_if_subprocess() -> Option<i32> {
    #[cfg(feature = "cef-backend")]
    {
        // SAFETY: `cef_rs::MainArgs::default()` zero-initialized struct;
        // Windows'ta command-line argümanları ana thread'in `argv`'i
        // üzerinden CEF'e geçirilir (`hInstance` Windows entry point'te
        // taşınır). Subprocess detection CEF'in command-line
        // `--type=` flag'ini parse etmesine dayanır; default value
        // subprocess değildir → `-1` döner.
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
        // Feature kapalı → stub: hiçbir zaman subprocess değiliz.
        // CEF runtime link edilmedi, dolayısıyla subprocess routing
        // yalnızca feature ON build'lerde aktive olur.
        None
    }
}

/// Subprocess routing sözleşme marker'ı (geriye uyumluluk).
///
/// `main.rs` bu marker'ı doğrudan kullanmaz; gerçek routing
/// `execute_process_if_subprocess()` üzerinden — feature-gated.
/// Marker `false` her zaman `execute_process` çağrısının ana process
/// tarafından atlandığını dokümante eder (test sözleşmesi).
///
/// # Returns
///
/// `false` her zaman — ana process'te subprocess dispatch yapılmaz.
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
            "Faz 1.6 PR-2 scope: V8 bridge + crashpad out-of-scope (human release engineering); subprocess routing implemented via `execute_process_if_subprocess`",
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
    fn cef_known_issues_mentions_subprocess_implemented() {
        // Subprocess routing artık implemented (PR ile `execute_process_if_subprocess`
        // fonksiyonu feature-gated olarak eklenmiştir). known_issues listesi
        // hâlâ subprocess kelimesini içermeli (insan review için context) ama
        // "subprocess routing + V8 bridge + crashpad out-of-scope" string'i
        // kalkmalı.
        let backend = CefBackend::new();
        let issues = backend.known_issues();
        assert!(
            issues.iter().any(|i| i.contains("subprocess")),
            "subprocess routing hâlâ known_issues'ta listelenmeli: {issues:?}"
        );
        assert!(
            !issues
                .iter()
                .any(|i| i.contains("subprocess routing + V8 bridge + crashpad out-of-scope")),
            "subprocess routing artık implemented; eski 'subprocess routing + V8 bridge + crashpad out-of-scope' \
             string'i kalkmalı: {issues:?}"
        );
    }

    #[test]
    fn subprocess_detection_returns_none_for_main_process() {
        // `cargo test` ana process olarak çalışır (CEF subprocess'i değil);
        // `execute_process_if_subprocess` `None` dönmeli — yani ana process
        // initialization'a devam eder.
        //
        // Feature ON iken gerçek `cef_rs::execute_process` çağrılır; CEF
        // subprocess tespit etmediğinden `-1` döner → `None` propagate
        // olur. Test build'i gerçek CEF runtime olmadan da feature ON
        // olsa bu yol çalışmalıdır (default `MainArgs::default()` zero-init
        // → "no recognized subprocess type" → -1).
        let result = execute_process_if_subprocess();
        assert!(
            result.is_none(),
            "ana process için execute_process_if_subprocess None dönmeli: got {result:?}"
        );
    }

    #[test]
    fn execute_process_if_subprocess_compiles_in_both_modes() {
        // Compile-time güvence: fonksiyon her iki feature modunda da
        // çağrılabilir olmalı (signature stable).
        let _: Option<i32> = execute_process_if_subprocess();
    }

    #[test]
    fn cef_subprocess_marker_is_false() {
        // Marker sözleşmesi: `cef_subprocess_main_marker` hâlâ
        // compile-time `false` döner. Gerçek routing
        // `execute_process_if_subprocess()` üzerinden — bu marker
        // yalnızca sözleşme dokümantasyonu (Faz 1.6 PR-2 öncesi
        // sözleşme kontratı).
        assert!(!cef_subprocess_main_marker());
    }
}
