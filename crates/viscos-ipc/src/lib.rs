//! `viscos-ipc` — pull-based IPC bridge between Rust backend and Discord frontend.
//!
//! Faz 1.0 kapsamı (ADR-0012 §3, `webview2-hardening.md` Bölüm 3):
//! - [`IpcCommand`] enum: JS → Rust pull-based komutlar.
//! - [`IpcEvent`] enum: Rust → JS küçük olaylar (tray badge, notification).
//! - [`IpcHandler`] trait: her command için async handler.
//! - [`DefaultIpcRouter`]: basit dispatch + `IpcCommandError::Unimplemented` stub'lar.
//! - [`IpcBuffer`] trait: büyük blob transfer için Faz 4 placeholder.
//!
//! **Pull-based default:** Rust tarafı asla büyük JSON push etmez. JS tarafı
//! `invoke("get_state")` ile ihtiyacı olan veriyi çeker.
//!
//! **Push exception:** Sadece küçük, gerçek zamanlı olaylar (mention count,
//! notification, yeni mesaj) push kalabilir. Tüm diğer state transferi pull.
//!
//! **Type system:** [`types`] modülü typed hataları ([`IpcCommandError`],
//! [`IpcEventError`]) ve Result alias'larını tutar. Library boundary typed,
//! application boundary `anyhow` (ADR-0007).
//!
//! Cross-references:
//! - [`webview2-hardening.md` §3 — Pull-Based IPC Pattern](../../.cursor/plans/webview2-hardening.md#3-pull-based-ipc-pattern-kritik)
//! - ADR-0012 §3 — `viscos.ipc` bridge protocol.
//! - [`phase-1.0-window-webview.md` §3.3](../../.cursor/plans/phase-1.0-window-webview.md#33-viscos-ipc-iskelet).
//! - ADR-0007 — error handling policy.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod buffer;
pub mod command;
pub mod event;
pub mod router;
pub mod types;

pub use buffer::IpcBuffer;
pub use command::{IpcCommand, IpcHandler};
pub use event::{IpcEvent, WatchdogKind};
pub use router::DefaultIpcRouter;
pub use types::{IpcCommandError, IpcCommandResult, IpcEventError, IpcEventResult};

/// Faz 1.0 IPC protokol versiyonu.
///
/// Breaking change (variant ekleme/çıkarma, field rename) → major bump + ADR.
pub const IPC_PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_protocol_version_is_one() {
        assert_eq!(IPC_PROTOCOL_VERSION, 1);
    }

    #[test]
    fn command_serde_round_trip_smoke() {
        // Serde tag/content yapısı doğru kuruldu mu?
        let cmd = IpcCommand::GetUnreadCount { guild_id: None };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let back: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        match back {
            IpcCommand::GetUnreadCount { guild_id } => assert!(guild_id.is_none()),
            _ => panic!("variant mismatch"),
        }
    }
}
