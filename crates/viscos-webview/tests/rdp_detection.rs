//! RDP session detection integration tests (Faz 1.6 Dalga 1c — ADR-0012 §6).
//!
//! ## Neden RDP auto-detect?
//!
//! Microsoft WebView2 RDP üzerinde GDI region leak yapıyor
//! ([WebView2Feedback #5266](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5266)).
//! Bu yüzden Faz 1.6 + ADR-0012 §6 kararı: RDP session'da CEF backend zorla
//! (feature ON ise gerçek runtime, OFF ise stub `Unimplemented`).
//!
//! ## Bu test
//!
//! - **Windows runner:** `GetSystemMetrics(SM_REMOTESESSION)` çağrısı panic etmemeli
//!   ve bool return etmeli. CI runner konsol session'da olduğu için `false` beklenir.
//! - **Non-Windows runner:** compile-time `false` (fallback).

use viscos_webview::is_rdp_session;

#[test]
fn is_rdp_session_compile_time_false_on_non_windows() {
    #[cfg(not(target_os = "windows"))]
    assert!(
        !is_rdp_session(),
        "non-Windows must hardcode RDP=false (no Win32 API)"
    );
}

#[test]
#[cfg(target_os = "windows")]
fn is_rdp_session_windows_returns_bool_without_panic() {
    // `GetSystemMetrics` panic-free olmalı (kernel API).
    // CI runner konsol session'da → SM_REMOTESESSION = 0 → false.
    let result = std::panic::catch_unwind(is_rdp_session);
    assert!(
        result.is_ok(),
        "is_rdp_session must not panic on Windows (GetSystemMetrics is panic-free)"
    );
    let rdp = result.unwrap();
    // CI windows-latest runner konsol session → false. RDP session'da
    // test koşulmaz (insan manual test); burada yalnızca API sözleşmesi.
    let _: bool = rdp;
}

#[test]
fn is_rdp_session_idempotent() {
    let a = is_rdp_session();
    let b = is_rdp_session();
    assert_eq!(
        a, b,
        "is_rdp_session must be idempotent (no mutable state across calls)"
    );
}
