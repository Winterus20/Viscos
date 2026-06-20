//! `viscos-webview` — pluggable WebView backend abstraction (Faz 1.6 Dalga 1b/c).
//!
//! ## Faz 1.6 scope
//!
//! - **`WebViewBackend` trait:** `create_window(&target, &config)` — pencere
//!   + WebView'i tek atomik adımda oluşturur (`tao::Window` + `wry::WebView`).
//! - **`WebViewWindow` trait:** `id`, `eval`, `navigate`, `close`, `as_any`.
//! - **`BackendKind` enum + `resolve_backend()`:** CLI > config > RDP > Win11/CEF > WebView2.
//! - **`WebView2Backend`:** gerçek `wry` runtime (Windows-only, MVP-1B).
//! - **`CefBackend`:** feature-gated stub (default) + DLL check (feature ON).
//!   B1 kararı: default build CEF kullanmaz; feature açıkça enable edilmeli.
//! - **`execute_process_if_subprocess`:** CEF subprocess dispatch entry point
//!   (Faz 1.6 Dalga 1b); `main.rs` bu fonksiyonu `cef::initialize`'dan önce çağırır.
//! - **RDP detection:** `GetSystemMetrics(SM_REMOTESESSION)` (ADR-0012 §6).
//! - **Win11 detection:** `windows_version::OsVersion::current().build >= 22000`
//!   (compile-time `cfg!` yerine runtime, ADR-0012 §4).
//!
//! ## Cross-references
//!
//! - [ADR-0012 §1](../../docs/DECISIONS.md#adr-0012-frontend-mimari--hibrit-webview--native-shell-haziran-2026-trade-off-revizyonu)
//! - [`webview2-hardening.md`](../../.cursor/plans/webview2-hardening.md)
//! - [`phase-1.6-cef-default-rollout.md`](../../.cursor/plans/phase-1.6-cef-default-rollout.md)

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod backend;
pub mod cef;
pub mod webview2;

pub use crate::cef::{CefBackend, cef_subprocess_main_marker, execute_process_if_subprocess};
pub use backend::{
    BackendKind, SharedBackend, WebViewBackend, WebViewWindow, WindowConfig, is_rdp_session,
    is_windows_11, resolve_backend, select_default_backend,
};
pub use webview2::{WebView2Backend, WebView2Window};

/// Faz 1.0'da Discord web client'ın yükleneceği URL.
///
/// ADR-0012 §1: Hibrit mimari — native shell + Discord web app (CEF/WebView2 içinde).
pub const DISCORD_APP_URL: &str = "https://discord.com/app";

/// Faz 1.6 Dalga 1c — Faz marker.
///
/// Production'da log + tray badge'de gösterilir.
pub const PHASE_1_6_NOTE: &str = "Faz 1.6 Dalga 1c — Win11 CEF auto-default (telemetry-driven), WebView2 fallback, RDP detection";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discord_app_url_is_set() {
        assert!(DISCORD_APP_URL.starts_with("https://"));
        assert!(DISCORD_APP_URL.contains("discord.com"));
    }

    #[test]
    fn phase_1_6_note_mentions_features() {
        assert!(PHASE_1_6_NOTE.contains("1.6"));
        assert!(PHASE_1_6_NOTE.contains("WebView2"));
        assert!(PHASE_1_6_NOTE.contains("RDP"));
    }

    #[test]
    fn resolve_backend_is_exposed() {
        // Lib API smoke: `resolve_backend` erişilebilir olmalı.
        let _ = resolve_backend(Some("webview2"), None, None);
    }

    #[test]
    fn rdp_and_win11_detection_are_exposed() {
        let _: bool = is_rdp_session();
        let _: bool = is_windows_11();
    }

    #[test]
    fn cef_subprocess_marker_is_exposed() {
        let _: bool = cef_subprocess_main_marker();
    }

    #[test]
    fn execute_process_if_subprocess_is_exposed() {
        // Subprocess routing function exposed — signature stable
        // her iki feature modunda.
        let _: Option<i32> = execute_process_if_subprocess();
    }
}
