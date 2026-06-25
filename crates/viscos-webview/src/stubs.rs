//! Phase boundary table — durable record of every stub in `viscos-webview`.
//!
//! This module is intentionally doc-only; it ships no production code. It
//! exists so the next person reading the crate can see at a glance which
//! items are stubs, what they currently do, and which phase the real
//! implementation lands in. Update this table whenever a stub transitions
//! to a real implementation (or when scope shifts between phases).
//!
//! Sources cross-referenced:
//! - [`docs/VISCOS-CODEBASE-STATUS-REPORT.md`](../../../docs/VISCOS-CODEBASE-STATUS-REPORT.md) §3 + §6
//! - [`docs/DECISIONS.md`](../../../docs/DECISIONS.md) ADR-0012 (Hibrit WebView/CEF)
//! - [`.cursor/plans/phase-1.6-cef-default-rollout.md`](../../../.cursor/plans/phase-1.6-cef-default-rollout.md)
//! - [`.cursor/plans/phase-8.0-distribution.md`](../../../.cursor/plans/phase-8.0-distribution.md)
//! - [`.cursor/plans/webview2-hardening.md`](../../../.cursor/plans/webview2-hardening.md)
//!
//! ## Table
//!
//! | Item | File:Line | Current behavior | Target phase |
//! |---|---|---|---|
//! | `CefBackend::create_window` (default build) | `cef.rs` | Returns `ViscosError::Unimplemented("cef-backend feature not enabled")` when crate is compiled without the `cef-backend` feature flag. | Faz 1.6 release eng (PR-2) — see status report §5 blocker #1 |
//! | `CefBackend::create_window` (feature ON) | `cef.rs:160` | DLL check (`check_cef_dll_present`) passes, then returns `ViscosError::Unimplemented("…wiring is out of scope for PR-2 (Faz 1.6 Dalga 1c)")` — does **not** call `cef::BrowserHost::CreateBrowser`. | Faz 1.6 release eng (PR-2) — out of scope per status report §3 + §6; requires human release engineering |
//! | `WebView2Window::close` | `webview2.rs:231-238` | Logs `tracing::debug!(window_id, "WebView2Window::close requested")` and returns `Ok(())`. Real close path goes through `tao::EventLoop::run` `WindowEvent::CloseRequested` + `Drop`. | Faz 5.0 (native UI) — a richer close API with explicit `ICoreWebView2Controller::Close` lives there |
//! | `WebViewBackend::post_shared_buffer` (default impl) | `backend.rs:133-137` | Returns `ViscosError::Unimplemented("post_shared_buffer Faz 4'te implemente edilecek")`. WebView2 → `CoreWebView2SharedBuffer`; CEF → `SharedMemoryRegion` + `message_router`. | Faz 4.0 (`phase-4.0-cache-media.md`) |
//! | `WebViewBackend::post_shared_buffer` per-backend | `webview2.rs` / `cef.rs` | Inherits default impl; no override yet. | Faz 4.0 — backend-specific impl lands with `CoreWebView2SharedBuffer` and CEF `SharedMemoryRegion`. |
//! | `WebView2Window::eval` (>10KB payload) | `webview2.rs:213-218` | Warns via `tracing::warn!(size_bytes, "eval_script payload > 10KB; consider SharedBuffer (Faz 4)")` but still calls `wry::WebView::evaluate_script`. | Faz 4.0 — actual SharedBuffer transfer replaces the inline eval. |
//!
//! ## Out of scope (NOT in this crate, mentioned for completeness)
//!
//! These items live in `viscos-shell` and are documented in that crate's own
//! phase boundary table; they are listed here so a future reader can locate
//! the cross-cutting concerns without grepping both crates:
//! - `tray::TrayState` non-Windows stub (`Unimplemented("tray-icon Windows-only MVP-3")`)
//! - `audio::AudioController` non-Windows stub (`Unimplemented("WASAPI Windows-only")`)
//! - `integration/hotkeys/manager.rs::HotkeyManager::events` (`HotkeyEventStream::stub`)
//! - `integration/deep_link.rs::register_protocol` (no-op + warn log)
//! - `integration/drag_drop.rs::handle_drop` upload wiring
//! - `integration/autostart.rs::{enable,disable,is_enabled}` no-ops
//! - `integration/single_instance.rs` in-process `parking_lot::Mutex`
//! - `integration/webview_refresh.rs::refresh_if_needed` always `Ok(false)`
//! - `native/panel.rs::SidePanel::view` returns empty `PanelLayout`
//! - `native/notify.rs::Notifier::notify` log-only
//! - `native/native_bridge.rs::disk_info` returns `(0, 0)`
//!
//! See `viscos-shell/src/stubs.rs` for the full table.
