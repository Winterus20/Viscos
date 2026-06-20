//! Integration test — `CefManager` backend selection.

use viscos_config::Config;
use viscos_distribution::{CefBackendChoice, CefManager};
use viscos_webview::{BackendKind, select_default_backend};

#[test]
fn current_backend_matches_select_default_when_auto() {
    let cfg = Config::default();
    let backend = CefManager::current_backend(&cfg);
    let expected: CefBackendChoice = select_default_backend(None).into();
    assert_eq!(backend, expected);
}

#[test]
fn current_backend_respects_explicit_cef_override() {
    let mut cfg = Config::default();
    cfg.webview.backend = "cef".to_string();
    assert_eq!(CefManager::current_backend(&cfg), CefBackendChoice::Cef);
}

#[test]
fn current_backend_respects_explicit_webview2_override() {
    let mut cfg = Config::default();
    cfg.webview.backend = "webview2".to_string();
    assert_eq!(
        CefManager::current_backend(&cfg),
        CefBackendChoice::WebView2
    );
}

#[test]
fn current_backend_falls_back_to_platform_default_for_unknown_value() {
    let mut cfg = Config::default();
    cfg.webview.backend = "unknown-backend".to_string();
    let backend = CefManager::current_backend(&cfg);
    let expected: CefBackendChoice = select_default_backend(None).into();
    assert_eq!(backend, expected);
}

#[test]
fn detect_installed_returns_none_in_phase_8_5_stub() {
    let cfg = Config::default();
    let result = CefManager::detect_installed(&cfg).expect("stub");
    assert!(
        result.is_none(),
        "Faz 8.5 stub must report CEF not detected"
    );
}

#[test]
fn set_default_backend_stub_succeeds() {
    let cfg = Config::default();
    assert!(CefManager::set_default_backend(&cfg, CefBackendChoice::Cef).is_ok());
    assert!(CefManager::set_default_backend(&cfg, CefBackendChoice::WebView2).is_ok());
}

#[test]
fn choice_into_backend_kind_round_trip() {
    let webview2 = CefBackendChoice::WebView2;
    let cef = CefBackendChoice::Cef;
    assert_eq!(BackendKind::from(webview2), BackendKind::WebView2);
    assert_eq!(BackendKind::from(cef), BackendKind::Cef);

    assert_eq!(
        CefBackendChoice::from(BackendKind::WebView2),
        CefBackendChoice::WebView2
    );
    assert_eq!(
        CefBackendChoice::from(BackendKind::Cef),
        CefBackendChoice::Cef
    );
}

#[test]
fn cef_manager_installed_state_reflects_fields() {
    let empty = CefManager::new("", std::path::PathBuf::from("/"));
    assert!(!empty.is_installed(), "empty version must be not installed");

    let installed = CefManager::new("148.3.0", std::path::PathBuf::from("/tmp/cef"));
    assert!(installed.is_installed());
}
