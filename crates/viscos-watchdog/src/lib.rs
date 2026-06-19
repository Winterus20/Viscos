//! `viscos-watchdog` — GDI object leak watchdog + draft autosave hook (Faz 1.0).
//!
//! Microsoft WebView2 GDI leak [`WebView2Feedback #5536`](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536)
//! upstream'de fix'siz (STATE: OPEN, Haziran 2026). Viscos'un Faz 1+ stratejisi:
//!
//! 1. **Katman 1 — Bu crate (Faz 1.0):** Sürekli `GetGuiResources` izleme,
//!    threshold aşımında soft restart (WebView dispose + recreate).
//! 2. Katman 2 — Telemetry (Faz 1.5): GDI time-series, restart optimizasyonu.
//! 3. Katman 3 — CEF default (Faz 1.6): Win11 leak'siz backend'e geçiş.
//!
//! ## Threshold'lar (Haziran 2026 güncellemesi)
//!
//! - **7000 GDI** → warning (önceki plan 5000).
//! - **9000 GDI** → critical, soft restart tetikle.
//! - Restart öncesi `DraftAutosave::snapshot_open_composers()` çağrılır →
//!   mesaj taslakları SQLite'a yazılır (Faz 2'de tam entegrasyon; Faz 1'de
//!   in-memory stub).
//!
//! Cross-references:
//! - [`webview2-hardening.md` Katman 1](../../.cursor/plans/webview2-hardening.md#katman-1-watchdog-faz-1--viscos-watchdog-crate)
//! - [`phase-1.0-window-webview.md` §3.4](../../.cursor/plans/phase-1.0-window-webview.md#34-viscos-watchdog-kritik)

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod autosave;
pub mod gdi;
pub mod restart;
pub mod watchdog;

pub use autosave::{DraftAutosave, StubAutosave};
pub use gdi::{GdiCounter, GdiSample};
pub use restart::{RestartReason, RestartSignal};
pub use watchdog::{TelemetrySink, Watchdog, WatchdogConfig};

/// Default sample interval (30s).
pub const DEFAULT_SAMPLE_INTERVAL_SECS: u64 = 30;

/// Default warning threshold (GDI obje sayısı).
pub const DEFAULT_GDI_WARNING: u32 = 7000;

/// Default critical threshold — soft restart tetikleyici.
pub const DEFAULT_GDI_CRITICAL: u32 = 9000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_phase_1_0_plan() {
        assert_eq!(DEFAULT_GDI_WARNING, 7000);
        assert_eq!(DEFAULT_GDI_CRITICAL, 9000);
        assert_eq!(DEFAULT_SAMPLE_INTERVAL_SECS, 30);
    }
}
