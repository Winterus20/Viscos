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
//! - Dalga 1c: telemetry-driven CEF default on Windows 11.

use viscos_error::ViscosError;
use viscos_webview::{
    BackendKind, WindowConfig, is_rdp_session, resolve_backend, select_default_backend,
};

// ─── resolve_backend priority chain ─────────────────────────────────────

#[test]
fn resolve_backend_cli_wins_for_webview2() {
    let kind = resolve_backend(Some("webview2"), Some("cef"), None).expect("valid CLI");
    assert_eq!(kind, BackendKind::WebView2);
}

#[test]
fn resolve_backend_cli_wins_for_cef() {
    let kind = resolve_backend(Some("cef"), None, None).expect("valid CLI");
    assert_eq!(kind, BackendKind::Cef);
}

#[test]
fn resolve_backend_cli_case_insensitive() {
    assert_eq!(
        resolve_backend(Some("WEBVIEW2"), None, None).unwrap(),
        BackendKind::WebView2
    );
    assert_eq!(
        resolve_backend(Some("Cef"), None, None).unwrap(),
        BackendKind::Cef
    );
    let auto_lower = resolve_backend(Some("aUtO"), None, None).unwrap();
    let default = select_default_backend(None);
    assert_eq!(auto_lower, default);
}

#[test]
fn resolve_backend_config_used_when_cli_absent() {
    let kind = resolve_backend(None, Some("cef"), None).expect("valid config");
    assert_eq!(kind, BackendKind::Cef);
}

#[test]
fn resolve_backend_config_auto_falls_through_to_detection() {
    let kind = resolve_backend(None, Some("auto"), None).expect("valid auto");
    let platform_default = select_default_backend(None);
    assert_eq!(kind, platform_default);
}

#[test]
fn resolve_backend_unknown_cli_returns_error() {
    let err = resolve_backend(Some("tauri"), None, None).expect_err("invalid backend");
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
    let kind = resolve_backend(None, Some(""), None).expect("empty config falls through");
    let platform_default = select_default_backend(None);
    assert_eq!(kind, platform_default);
}

// ─── Dalga 1c: CLI/config override ignores telemetry ────────────────────

#[test]
fn cli_override_wins_regardless_of_telemetry() {
    use viscos_telemetry::store::TelemetryStore;

    let store = TelemetryStore::open_in_memory().expect("open");
    // Pump high GDI to trigger Required recommendation.
    store.record_gdi_sample(9000).expect("record");

    // CLI "webview2" must win even when telemetry says CEF Required.
    let kind = resolve_backend(Some("webview2"), None, Some(&store)).unwrap();
    assert_eq!(
        kind,
        BackendKind::WebView2,
        "CLI override must beat telemetry"
    );

    // CLI "cef" must win even with no telemetry.
    let kind2 = resolve_backend(Some("cef"), None, None).unwrap();
    assert_eq!(kind2, BackendKind::Cef);
}

// ─── Dalga 1c: select_default_backend telemetry-driven (Win11 only) ─────

/// Win11 platform + no telemetry → CEF default (ADR-0012 §4 B1).
///
/// This test only asserts the Win11 branch; non-Windows CI skips the CEF check.
#[test]
fn select_default_no_telemetry_win11_returns_cef() {
    #[cfg(target_os = "windows")]
    {
        use viscos_webview::is_windows_11;
        if is_windows_11() && !is_rdp_session() {
            assert_eq!(
                select_default_backend(None),
                BackendKind::Cef,
                "Win11 + no telemetry must default to CEF"
            );
        }
    }
    // On non-Windows or Win10 just confirm we get a valid kind.
    let _ = select_default_backend(None);
}

/// Win11 + telemetry `Required` (GDI peak ≥ 8500) → CEF.
#[test]
fn select_default_telemetry_required_returns_cef() {
    use viscos_telemetry::store::TelemetryStore;

    #[cfg(target_os = "windows")]
    {
        use viscos_webview::is_windows_11;
        if is_windows_11() && !is_rdp_session() {
            let store = TelemetryStore::open_in_memory().expect("open");
            store.record_gdi_sample(9000).expect("record");
            assert_eq!(
                select_default_backend(Some(&store)),
                BackendKind::Cef,
                "Win11 + Required telemetry must select CEF"
            );
        }
    }
    // Non-Windows: just confirm no panic.
    #[cfg(not(target_os = "windows"))]
    {
        let store = TelemetryStore::open_in_memory().expect("open");
        store.record_gdi_sample(9000).expect("record");
        let kind = select_default_backend(Some(&store));
        assert_eq!(
            kind,
            BackendKind::WebView2,
            "non-Windows must always be WebView2"
        );
    }
}

/// Win11 + telemetry `Optional` (GDI stable, below threshold) → WebView2.
#[test]
fn select_default_telemetry_optional_returns_webview2() {
    use viscos_telemetry::store::TelemetryStore;

    #[cfg(target_os = "windows")]
    {
        use viscos_webview::is_windows_11;
        if is_windows_11() && !is_rdp_session() {
            let store = TelemetryStore::open_in_memory().expect("open");
            // 5000 < CEF_REQUIRED_PEAK_THRESHOLD (8500) → Optional.
            store.record_gdi_sample(5000).expect("record");
            assert_eq!(
                select_default_backend(Some(&store)),
                BackendKind::WebView2,
                "Win11 + Optional telemetry must stay on WebView2"
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let store = TelemetryStore::open_in_memory().expect("open");
        store.record_gdi_sample(5000).expect("record");
        let kind = select_default_backend(Some(&store));
        assert_eq!(kind, BackendKind::WebView2);
    }
}

/// Win11 + telemetry `Unknown` (empty store) → CEF default (fall-through).
#[test]
fn select_default_telemetry_unknown_falls_through_to_cef() {
    use viscos_telemetry::store::TelemetryStore;

    #[cfg(target_os = "windows")]
    {
        use viscos_webview::is_windows_11;
        if is_windows_11() && !is_rdp_session() {
            let store = TelemetryStore::open_in_memory().expect("open");
            // Empty store → Unknown recommendation.
            assert_eq!(
                select_default_backend(Some(&store)),
                BackendKind::Cef,
                "Win11 + Unknown telemetry must fall through to CEF default"
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let store = TelemetryStore::open_in_memory().expect("open");
        let kind = select_default_backend(Some(&store));
        assert_eq!(kind, BackendKind::WebView2);
    }
}

/// RDP session → WebView2 regardless of OS and telemetry (CEF GPU pipeline issue).
#[test]
fn rdp_session_forces_webview2() {
    // We can't fake is_rdp_session() without OS cooperation, so this smoke test
    // only verifies non-RDP path on CI (where is_rdp_session() is false).
    // A real RDP runner test is in the manual validation checklist.
    let rdp = is_rdp_session();
    if rdp {
        // Actually on RDP: verify the decision.
        assert_eq!(
            select_default_backend(None),
            BackendKind::WebView2,
            "RDP session must select WebView2"
        );
    }
    // Non-RDP path: just verify we get a valid kind.
    let _ = select_default_backend(None);
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
