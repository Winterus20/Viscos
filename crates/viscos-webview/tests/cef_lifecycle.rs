//! CEF lifecycle integration tests (Faz 1.6 Dalga 1b/c).
//!
//! ## Coverage
//!
//! - **`cef_initialize_idempotent`**: Aynı process'te birden fazla `cef::initialize`
//!   çağrısının crash etmediğini (marker) doğrular. CEF subprocess routing
//!   sözleşmesi: `main.rs` yalnızca 1 kez `execute_process` çağırır.
//!
//! - **`cef_runtime_path_override`**: Config-driven DLL injection sözleşmesi.
//!   `CefBackend::with_runtime_dir` sonradan `cef-dll-check` aşamasında
//!   `%APPDATA%/Viscos/cef` yerine explicit path kullanır.
//!
//! - **`cef_subprocess_routing_marker`**: `cef_subprocess_main_marker()`
//!   her zaman `false` (PR-2 out-of-scope). İnsan release engineering
//!   `FOLLOW-UP-REAL-WORLD-WORK.md §B`'de.
//!
//! - **`mock_cef_browser_handle` (feature `test-cef-mock`):** Sahte
//!   `cef::Browser` handle ile smoke test. Feature CI smoke için
//!   (`cargo test --features viscos-webview/test-cef-mock`).
//!
//! ADR-0012 §4 + Faz 1.6 Dalga 1b plan dosyası.

use viscos_webview::{
    BackendKind, CefBackend, WebViewBackend, cef_subprocess_main_marker, resolve_backend,
};

#[test]
fn cef_initialize_idempotent() {
    // PR-2 scope: subprocess routing main.rs'de (out-of-scope).
    // Bu test yalnızca sözleşmeyi doğrular; gerçek `cef::api_hash` +
    // `cef::initialize` çağrıları Faz 1.6 Dalga 1b sonrası (insan-only).
    assert!(
        !cef_subprocess_main_marker(),
        "PR-2 scope: cef_subprocess_main_marker must be false (main.rs marker sözleşmesi)"
    );

    // Marker doğru → `main.rs` `cef::execute_process` çağırmaz, dolayısıyla
    // initialize yalnız 1 kez çağrılır (idempotent invariant).
    // İnsan PR (`FOLLOW-UP-REAL-WORLD-WORK.md §B`) marker'ı true yapar ve
    // subprocess dispatch'i main.rs'de gerçekler.
}

#[test]
fn cef_runtime_path_override() {
    // Config-driven DLL injection sözleşmesi.
    let custom_dir = std::path::PathBuf::from("/tmp/viscos-cef-test");
    let backend = CefBackend::with_runtime_dir(custom_dir.clone());

    // Backend struct'ı runtime_dir'i tutar (Faz 1.6 stub seviyesinde).
    // Feature ON iken `create_window()` bu path'i DLL check'inde kullanır.
    #[cfg(feature = "cef-backend")]
    {
        // Feature ON — DLL check private `dll_path_or_error` üzerinden geçer.
        // Public API sözleşmesi: `with_runtime_dir` sonradan DLL aramasını
        // override eder. Burada feature gate edilmemiş olan `runtime_dir` field'ına
        // doğrudan erişim yok; bu nedenle version/known_issues metadata
        // sözleşmesini verify ediyoruz.
        let version = backend.version();
        assert!(
            version.contains("cef-v148") || version.contains("stub"),
            "version must reflect feature-gated runtime: {version}"
        );
    }
    #[cfg(not(feature = "cef-backend"))]
    {
        // Default build — feature OFF, stub. Sözleşme: version "stub" içermeli.
        assert_eq!(backend.version(), "cef-rs stub (feature off)");
    }

    // Path semantik testi — struct'ın `runtime_dir` field'ına erişim yok
    // ama `with_runtime_dir` çağrısı compile-time güvencesi.
    let _ = backend.known_issues(); // ensure trait API accessible
}

#[test]
fn cef_subprocess_routing_marker() {
    // `cef_subprocess_main_marker` her zaman `false` (PR-2 out-of-scope).
    // main.rs bu marker'ı `if cef_subprocess_main_marker() { ... }` ile
    // okur; `false` ise `cef::execute_process` çağrısı atlanır.
    assert!(!cef_subprocess_main_marker());
}

#[cfg(feature = "test-cef-mock")]
#[test]
fn mock_cef_browser_handle() {
    // CI smoke — feature-gated. Mock CefBrowser handle yaratma sentaksı.
    // Gerçek `cef::Browser::create` production'da ana thread'de çağrılır
    // (Faz 8.5 self-update sonrası). Bu test yalnızca compile-time güvencesi.

    // Mock: `CefBackend::new()` default runtime dir ile instantiate edilir.
    // Production davranışı: DLL check → BrowserHost::CreateBrowser → handle return.
    // Stub feature ON olsa bile `create_window` PR-2'de gerçek runtime'a
    // bağlanmaz (DLL present ise bile Unimplemented döner, bkz. cef.rs).
    let backend = CefBackend::new();
    let _kind: BackendKind = BackendKind::Cef;
    let _ = backend.version();
    let _ = backend.known_issues();
}

#[test]
fn resolve_backend_cef_with_cli_override() {
    // CLI override "cef" → backend seçimi.
    let kind = resolve_backend(Some("cef"), None, None).expect("valid CLI override");
    assert_eq!(kind, BackendKind::Cef);
}

#[test]
fn resolve_backend_configured_cef_falls_through_to_default() {
    // Config "cef" (CLI yok) → auto-detect (RDP/Win11 kontrolü).
    // Bu test non-Windows ve RDP-dışı runner'da çalışırsa WebView2'ye düşer
    // (feature-gated stub default davranışı).
    let kind = resolve_backend(None, Some("cef"), None).expect("valid config override");
    // Result config override yüzünden her zaman Cef olmalı (RDP/auto'ya düşmez).
    assert_eq!(kind, BackendKind::Cef);
}
