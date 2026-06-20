//! Integration tests for `resolve_backend` priority chain (Faz 1.6 Dalga 1c).
//!
//! Extracted from `src/backend.rs`'s inline `mod tests` to keep the
//! production file under the 400-line refactor threshold (.cursorrules Bölüm 2).
//!
//! ## Coverage
//!
//! - CLI override wins (webview2 / cef / auto).
//! - Config fallback (override + auto fallthrough).
//! - Case-insensitive parsing.
//! - Unknown backend → Media error.
//! - RDP / Win11 detection smoke tests.
//! - `post_shared_buffer` Faz 1.0 default impl returns `Unimplemented`.

use viscos_error::ViscosError;
use viscos_webview::{
    BackendKind, WindowConfig, is_rdp_session, resolve_backend, select_default_backend,
};

// ─── resolve_backend priority chain ─────────────────────────────────────

#[test]
fn resolve_backend_cli_wins_for_webview2() {
    let kind = resolve_backend(Some("webview2"), Some("cef")).expect("valid CLI");
    assert_eq!(kind, BackendKind::WebView2);
}

#[test]
fn resolve_backend_cli_wins_for_cef() {
    let kind = resolve_backend(Some("cef"), None).expect("valid CLI");
    assert_eq!(kind, BackendKind::Cef);
}

#[test]
fn resolve_backend_cli_case_insensitive() {
    assert_eq!(
        resolve_backend(Some("WEBVIEW2"), None).unwrap(),
        BackendKind::WebView2
    );
    assert_eq!(
        resolve_backend(Some("Cef"), None).unwrap(),
        BackendKind::Cef
    );
    let auto_lower = resolve_backend(Some("aUtO"), None).unwrap();
    let default = select_default_backend();
    assert_eq!(auto_lower, default);
}

#[test]
fn resolve_backend_config_used_when_cli_absent() {
    let kind = resolve_backend(None, Some("cef")).expect("valid config");
    assert_eq!(kind, BackendKind::Cef);
}

#[test]
fn resolve_backend_config_auto_falls_through_to_detection() {
    let kind = resolve_backend(None, Some("auto")).expect("valid auto");
    let platform_default = if cfg!(target_os = "windows") && is_rdp_session() {
        BackendKind::Cef
    } else {
        select_default_backend()
    };
    assert_eq!(kind, platform_default);
}

#[test]
fn resolve_backend_unknown_cli_returns_error() {
    let err = resolve_backend(Some("tauri"), None).expect_err("invalid backend");
    assert!(
        matches!(err, viscos_error::ViscosError::Media(_)),
        "expected Media error variant, got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("tauri"),
        "error must echo offending value: {msg}"
    );
}

#[test]
fn resolve_backend_empty_string_config_falls_through() {
    let kind = resolve_backend(None, Some("")).expect("empty config falls through");
    let platform_default = if cfg!(target_os = "windows") && is_rdp_session() {
        BackendKind::Cef
    } else {
        select_default_backend()
    };
    assert_eq!(kind, platform_default);
}

// ─── WebViewBackend trait surface ───────────────────────────────────────

/// `post_shared_buffer` Faz 1.0 default impl returns `ViscosError::Unimplemented`.
#[test]
fn post_shared_buffer_returns_unimplemented_in_phase_1_0() {
    use viscos_webview::{WebViewBackend, WebViewWindow};

    struct Probe;
    impl WebViewBackend for Probe {
        fn create_window(
            &self,
            _target: &tao::event_loop::EventLoopWindowTarget<()>,
            _config: &WindowConfig,
        ) -> viscos_error::Result<Box<dyn WebViewWindow>> {
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
