//! `viscos-distribution` — distribution, auto-update, crash reporting,
//! profiling, code signing, and CEF backend management.
//!
//! Faz 8.0 kapsamı:
//! - `updater` — GitHub Releases üzerinden auto-update (Faz 8.0 stub).
//! - `crash`   — opt-in crash reporting (`minidumper` entegrasyonu stub).
//! - `profile` — heap profiling (`dhat`, feature-gated).
//! - `signing` — Authenticode code signing (release engineering stub).
//!
//! Faz 8.5 kapsamı:
//! - `cef_manager`     — CEF runtime detect / current / set_default.
//! - `chromium_flags`  — Chromium flag config loader + deny-list.
//! - `cef_update`      — CEF runtime self-update (Chromium advisory feed stub).
//!
//! Cross-references:
//! - [`phase-8.0-distribution.md`](../../.cursor/plans/phase-8.0-distribution.md)
//! - [`phase-8.5-cef-backend.md`](../../.cursor/plans/phase-8.5-cef-backend.md)
//! - [`webview2-hardening.md`](../../.cursor/plans/webview2-hardening.md)
//! - ADR-0012 §CefUpdate.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod cef_manager;
pub mod cef_update;
pub mod chromium_flags;
pub mod crash;
pub mod profile;
pub mod signing;
pub mod updater;

pub use cef_manager::{CefBackendChoice, CefManager, CefManagerError};
pub use cef_update::{CefRelease, CefUpdateError, CefUpdateTrigger, CefUpdater};
pub use chromium_flags::{ChromiumFlags, ChromiumFlagsError, DEFAULT_DENY_FLAGS};
pub use crash::{CrashConfig, CrashError, CrashOptInStatus, CrashReporter};
pub use profile::{HeapProfilerGuard, init_heap_profiling};
pub use signing::{CodeSigner, SignerConfig, SignerError};
pub use updater::{ReleaseInfo, Updater, UpdaterError};

/// Default GitHub repository (`owner/name`). Centralized to keep config + module consistent.
pub const DEFAULT_REPO: &str = "Winterus20/Viscos";

/// Default binary name (exe stem).
pub const DEFAULT_BINARY_NAME: &str = "viscos";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_non_empty() {
        assert!(!DEFAULT_REPO.is_empty());
        assert!(!DEFAULT_BINARY_NAME.is_empty());
        assert_eq!(DEFAULT_BINARY_NAME, "viscos");
    }
}
