//! `viscos-webview` — pluggable WebView backend abstraction.
//!
//! Faz 1.0 kapsamı (proceed-as-proposed, ADR-0012):
//! - `WebViewBackend` trait tanımı: backend-agnostic WebView açma API'si.
//! - `BackendKind` enum + `select_default_backend()`: Win11 → CEF, Win10 → WebView2 (Faz 1.6 default).
//! - `WindowConfig` + `WebViewWindow` trait: pencere + WebView handle abstraction.
//! - `WebView2Backend::new()` ve `CefBackend::new()` stub struct'lar (gerçek entegrasyon Faz 1.6).
//! - `BRIDGE-RESILIENCE.md` deliverable dokümanı (crate root'unda).
//!
//! Faz 1.6'da gerçek `wry::WebViewBuilder` entegrasyonu + CEF `cef-rs` backend'i eklenecek.
//! Faz 4'te `WebViewBackend::post_shared_buffer` implemente edilecek (WebView2 SharedBuffer / CEF SharedMemoryRegion).
//!
//! Cross-cutting referans: [`webview2-hardening.md`](../../.cursor/plans/webview2-hardening.md),
//! [`packet-0012-frontend-hybrid.md`](../../.cursor/packets/packet-0012-frontend-hybrid.md).

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod backend;
pub mod cef;
pub mod webview2;

pub use backend::{
    BackendKind, WebViewBackend, WebViewWindow, WindowConfig, select_default_backend,
};

/// Faz 1.0'da Discord web client'ın yükleneceği URL.
///
/// ADR-0012 §1: Hibrit mimari — native shell + Discord web app (CEF/WebView2 içinde).
pub const DISCORD_APP_URL: &str = "https://discord.com/app";

/// Faz 1.0'da henüz gerçek WebView oluşturulmadığı için loglanan placeholder mesaj.
pub const STUB_PHASE_NOTE: &str =
    "Faz 1.0 stub: gerçek WebView oluşturma Faz 1.6'da (wry + cef-rs entegrasyonu)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discord_app_url_is_set() {
        assert!(DISCORD_APP_URL.starts_with("https://"));
        assert!(DISCORD_APP_URL.contains("discord.com"));
    }

    #[test]
    fn stub_phase_note_mentions_phase_1_6() {
        assert!(STUB_PHASE_NOTE.contains("1.6"));
    }
}
