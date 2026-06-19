//! `viscos-shell` — native shell katmanı (Faz 1.0 stub + Faz 5.0/6.0 eklentileri).
//!
//! Faz 1.0 kapsamı:
//! - `tao` event loop kurulumu için `Shell` struct + `ShellConfig` (pencere + tray).
//! - `iced 0.14` spike: production kanıtı az olduğu için `SPIKE_NOTE` constant.
//! - Frame timing ölçümü (`FrameTimer`) — native frame drop <%1 hedefi.
//! - Tray icon stub (`tray` feature `tao 0.35` ile).
//! - Resize davranışı: laggy mi değil mi saptama için `resize_observer` helper.
//!
//! Faz 5.0 (Faz 5 native UI):
//! - `native::theme` — Dark / Light / Auto theming + Discord-tarzı palette.
//! - `native::panel` — iced side panel placeholder.
//! - `native::notify` — Windows toast notifications (`notify-rust`).
//! - `native::native_bridge` — Vencord/Equicord `ViscosNative` POC API.
//!
//! Faz 6.0 (Entegrasyon):
//! - `integration::hotkeys` — Global + window hotkeys (`global-hotkey` + `muda`).
//! - `integration::drag_drop` — Drag & drop dosya paylaşımı.
//! - `integration::deep_link` — `viscos://` URL parser + Windows registry.
//! - `integration::autostart` — Windows auto-start (`auto-launch`).
//! - `integration::single_instance` — Single-instance lock.
//!
//! Cross-references:
//! - [`phase-1.0-window-webview.md` §3.1](../../.cursor/plans/phase-1.0-window-webview.md#31-viscos-shell)
//! - [`phase-5.0-native-ui.md`](../../.cursor/plans/phase-5.0-native-ui.md)
//! - [`phase-6.0-hotkeys.md`](../../.cursor/plans/phase-6.0-hotkeys.md)
//! - ADR-0012 §5 — `iced 0.14` spike.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod frame_timer;
pub mod integration;
pub mod native;
pub mod window;

pub use frame_timer::{FrameStats, FrameTimer};
pub use window::{
    ResizeObserver, Shell, ShellBuilder, ShellConfig, TrayMenu, TrayMenuItem, TrayState,
};

/// Faz 1.0 — `iced 0.14` spike durumu.
///
/// `iced 0.14` son deneysel sürüm (1.0 freeze öncesi). Production kanıtı
/// Halloy/Sniffnet/Neothesia production ama **Discord client + WebView overlay
/// senaryosu yok**. Faz 1.0 ilk haftasında 1 haftalık spike yapılacak.
///
/// **Spike başarısız olursa:** `iced 0.14` → `0.13` downgrade veya `egui`
/// değerlendirmesi (immediate-mode, native shell için farklı trade-off).
///
/// Faz 1.0 stub'ında iced dependency eklenmedi; spike başarılı olursa Faz 1.5'te
/// eklenir (`iced = { version = "0.14", features = ["wgpu", "tokio"] }`).
pub const SPIKE_NOTE: &str = "Faz 1.0 — iced 0.14 spike sonucu: beklemede (ADR-0012 §5)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spike_note_mentions_phase_1_0() {
        assert!(SPIKE_NOTE.contains("1.0"));
        assert!(SPIKE_NOTE.contains("ADR-0012"));
    }
}
