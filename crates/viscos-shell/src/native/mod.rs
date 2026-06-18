//! Viscos native side panel + theming + native notifications + Vencord bridge.
//!
//! Faz 5.0 — Vesktop'tan gerçek farklılaşma noktası: iced native side panel
//! ile sunucu/kanal/üye listesi. WebView2 sadece mesaj alanını render eder.
//!
//! Modules:
//! - [`theme`]: Dark / Light / Auto theming system.
//! - [`panel`]: iced side panel placeholder widget (real data binding Faz 5.x).
//! - [`notify`]: Native Windows toast notifications.
//! - [`native_bridge`]: Vencord/Equicord `ViscosNative` POC API.
//!
//! Cross-references:
//! - [`phase-5.0-native-ui.md`](../../../.cursor/plans/phase-5.0-native-ui.md)
//! - ADR-0012 §5 — `iced 0.14` native side panel kararı.
//! - [`viscos_auth_research.md` §VesktopNative API referansı](../../../.cursor/plans/viscos_auth_research.md).

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod native_bridge;
pub mod notify;
pub mod panel;
pub mod theme;

pub use native_bridge::{
    DefaultViscosNative, ViscosNative, ViscosNativeRequest, ViscosNativeResponse,
};
pub use notify::Notifier;
pub use panel::SidePanel;
pub use theme::{Theme, ThemePalette};
