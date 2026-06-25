//! Phase boundary table — durable record of every stub in `viscos-shell`.
//!
//! Doc-only module — ships no production code. Mirrors the structure of
//! `viscos-webview::stubs`: one entry per stub, what it currently does,
//! and the phase the real implementation lives in. Update on every
//! transition from stub to real implementation.
//!
//! Sources cross-referenced:
//! - [`docs/VISCOS-CODEBASE-STATUS-REPORT.md`](../../../docs/VISCOS-CODEBASE-STATUS-REPORT.md) §3 + §6
//! - [`.cursor/plans/phase-5.0-native-ui.md`](../../../.cursor/plans/phase-5.0-native-ui.md)
//! - [`.cursor/plans/phase-6.0-hotkeys.md`](../../../.cursor/plans/phase-6.0-hotkeys.md)
//! - [`.cursor/plans/phase-8.0-distribution.md`](../../../.cursor/plans/phase-8.0-distribution.md)
//!
//! ## Table
//!
//! | Item | File:Line | Current behavior | Target phase |
//! |---|---|---|---|
//! | `TrayState::new` (non-Windows) | `window/tray.rs:100` | Returns `Err(ViscosError::Unimplemented("tray-icon Windows-only MVP-3"))`. Linux/macOS tray backends are deliberately not wired in v1. | v2.0 (post-V1 MVP); see status report §3 + §5 |
//! | `TrayState::set_badge` (non-Windows) | `window/tray.rs:142` | Returns `Err(ViscosError::Unimplemented("tray-icon Windows-only MVP-3"))`; cached `badge_text` is not mutated. | v2.0 — same milestone as `TrayState::new` |
//! | `AudioController::toggle_mute` (non-Windows) | `integration/audio.rs:124` | Returns `Err(ViscosError::Unimplemented("WASAPI Windows-only"))` without touching the cached `mic_muted` state. | v2.0 — see status report §3 (`audio.rs:124,160`) + §6 |
//! | `AudioController::toggle_deafen` (non-Windows) | `integration/audio.rs:160` | Same `Unimplemented("WASAPI Windows-only")`; no cached-state mutation. | v2.0 — same milestone as `toggle_mute` |
//! | `HotkeyManager::events` | `integration/hotkeys/manager.rs:104-106` | Returns `HotkeyEventStream::stub()` — inert handle whose `is_stub()` returns `true`. No real OS event delivery. | Faz 6.0 (`phase-6.0-hotkeys.md` §2) |
//! | `deep_link::register_protocol` | `integration/deep_link.rs:117-122` | Returns `Ok(())` after a `tracing::warn!`. No `HKCU\Software\Classes\viscos` registry write happens. | Faz 8.0 (`phase-8.0-distribution.md`) — MSI installer owns the write |
//! | `drag_drop::handle_drop` | `integration/drag_drop.rs:24-42` | Validates `path.is_file()`, logs `tracing::info!`, returns `Ok(())`. **No** `MediaUploader::upload` is invoked. | Faz 5.x (`phase-5.0-native-ui.md` — drag-drop) |
//! | `AutoLaunch::enable` | `integration/autostart.rs:36-58` | Logs `tracing::info!` + returns `Ok(())`. **No** `HKCU\…\Run` registry write happens. | Faz 6.0 (`phase-6.0-hotkeys.md` §5) |
//! | `AutoLaunch::disable` | `integration/autostart.rs:46-49` | Same `Ok(())` no-op + log; no registry delete. | Faz 6.0 — same milestone as `enable` |
//! | `AutoLaunch::is_enabled` | `integration/autostart.rs:55-58` | Hard-coded `Ok(false)` after `tracing::debug!` log; does not query the registry. | Faz 6.0 — same milestone |
//! | `SingleInstance::acquire` | `integration/single_instance.rs:51-71` | `parking_lot::Mutex::try_lock` on a process-local static — **in-process only**; a second `viscos.exe` launched by the user will *not* see the lock. | Faz 6.0 (`phase-6.0-hotkeys.md` §6) — `single-instance 0.3` named mutex / Unix socket |
//! | `SingleInstance::on_secondary_launch` | `integration/single_instance.rs:65-71` | Accepts the closure and silently drops it. No named pipe / Unix socket set up. | Faz 6.0 — same milestone as `acquire` |
//! | `WebView2Refresher::refresh_if_needed` | `integration/webview_refresh.rs:67-75` | Returns `Ok(false)` unconditionally after `tracing::debug!`. The WebView2 is never closed / recreated, so the GDI leak continues to accumulate in long-lived sessions. | Faz 8.x (`phase-8.0-distribution.md` §2.1) |
//! | `SidePanel::view` | `native/panel.rs:56-64` | Returns a hand-built `PanelLayout { width: 72, height: 600, guild_count: 0, channel_count: 0, member_count: 0 }`. `is_populated()` is `false`. | Faz 5.x (`phase-5.0-native-ui.md` §5) — depends on `iced 0.14` spike resolving |
//! | `Notifier::notify` | `native/notify.rs:54-66` | Emits `tracing::info!(app, title, body, "native notification (Faz 5.0 stub …)")` + returns `Ok(())`. The OS is never asked to display a toast. | Faz 1.5 (tray integration) — `phase-1.5-telemetry-and-restart-optimization.md` |
//! | `disk_info` (free_bytes/total_bytes) | `native/native_bridge.rs:182-191` | Returns `Ok((0, 0))` after computing `current_dir` (swallowing failures). The metadata is queried but its bytes fields are never read. | Faz 6.0 (`phase-6.0-hotkeys.md` §7 — Vencord/Equicord full integration) — `sysinfo` or `GetDiskFreeSpaceExW` |
//!
//! ## Cross-crate stubs
//!
//! The following cross-cutting concerns live in `viscos-webview` and are
//! documented in that crate's own phase boundary table; listed here for
//! discoverability:
//! - `WebView2Window::close` (`webview2.rs:231-238`) — log-only stub.
//! - `CefBackend::create_window` (`cef.rs`) — feature-gated `Unimplemented`.
//! - `WebViewBackend::post_shared_buffer` default impl — `Unimplemented`.
//!
//! See `viscos-webview/src/stubs.rs` for details.
