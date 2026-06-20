//! Viscos shell entegrasyon katmanı (Faz 6.0 + Faz 8.0 WebView refresh).
//!
//! Faz 6.0 kapsamı: hotkeys, drag & drop, deep linking, auto-start,
//! single-instance. Vencord/Equicord plugin tam entegrasyonu Faz 6.x'te.
//!
//! Faz 8.0 kapsamı:
//! - [`webview_refresh`]: WebView2 periyodik recreate (GDI leak mitigation).
//!
//! Modules:
//! - [`audio`]: Windows WASAPI mute/deafen scaffold (MVP-3, non-Windows stub).
//! - [`hotkeys`]: Global + window hotkeys (Ctrl+Shift+M, Ctrl+K, ...).
//! - [`drag_drop`]: Drag & drop dosya paylaşımı (stub).
//! - [`deep_link`]: `viscos://` URL parser + Windows registry registration (stub).
//! - [`autostart`]: Windows auto-start (`auto-launch`).
//! - [`single_instance`]: Single-instance lock + secondary launch forward.
//! - [`webview_refresh`]: WebView2 periyodik recreate (Faz 8.0 stub).
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md`](../../../.cursor/plans/phase-6.0-hotkeys.md)
//! - [`phase-8.0-distribution.md` §2.1](../../../.cursor/plans/phase-8.0-distribution.md#21-periyodik-refresh)

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod audio;
pub mod autostart;
pub mod deep_link;
pub mod drag_drop;
pub mod hotkeys;
pub mod single_instance;
pub mod webview_refresh;

pub use audio::AudioController;
pub use autostart::AutoLaunch;
pub use deep_link::{DeepLinkAction, parse_viscos_url, register_protocol};
pub use drag_drop::handle_drop;
pub use hotkeys::{
    DEFAULT_BINDINGS, HotkeyAction, HotkeyBinding, HotkeyController, HotkeyEventStream,
    HotkeyManager, parse_combo,
};
pub use single_instance::SingleInstance;
pub use webview_refresh::{WebView2Refresher, WebViewRefreshConfig};
