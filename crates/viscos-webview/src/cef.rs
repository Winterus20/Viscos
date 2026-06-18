//! `CefBackend` stub — Faz 1.6'da `cef-rs` ile gerçek implementasyon.
//!
//! Faz 1.0 davranışı: `create_window` `ViscosError::Unimplemented` döner.
//! Faz 1.6'da:
//! - `cef-rs` ile Chromium embedded binary load
//! - Win11 default'a geçiş (ADR-0012 §4)
//! - `select_default_backend()` Win11 → `BackendKind::Cef`
//!
//! Not: CEF binary büyüklüğü ~220-300 MB; Faz 8.5'te self-update gerekir.

use viscos_error::{Result, ViscosError};

use crate::backend::{WebViewBackend, WebViewWindow, WindowConfig};

/// CEF (Chromium Embedded Framework) backend.
///
/// Win11 default (Faz 1.6), Win10 opt-in.
#[derive(Debug, Clone, Default)]
pub struct CefBackend;

impl CefBackend {
    /// Yeni `CefBackend` instance'ı.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl WebViewBackend for CefBackend {
    fn create_window(&self, _config: WindowConfig) -> Result<Box<dyn WebViewWindow>> {
        // Faz 1.0: stub. Faz 1.6'da `cef-rs` initialization + window attach.
        Err(ViscosError::Unimplemented("phase-1.0 cef stub"))
    }

    fn name(&self) -> &'static str {
        "CEF (cef-rs)"
    }

    fn version(&self) -> &'static str {
        "cef-rs stub"
    }

    fn known_issues(&self) -> &[&'static str] {
        &[
            "cef-rs startup time 1.5-2.5s (Chromium initialization)",
            "Binary size 220-300 MB (Faz 8.5 self-update required)",
            "Idle RAM +50-100 MB vs WebView2",
            "Disk cache +150 MB (%APPDATA%/Viscos/cef-cache)",
        ]
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
    fn cef_create_window_returns_unimplemented_in_phase_1_0() {
        let backend = CefBackend::new();
        let result = backend.create_window(WindowConfig::default());
        match result {
            Err(ViscosError::Unimplemented(_)) => {}
            Err(other_err) => panic!("expected Unimplemented error, got {other_err:?}"),
            Ok(_) => panic!("expected error in Faz 1.0 stub, got Ok value"),
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
}
