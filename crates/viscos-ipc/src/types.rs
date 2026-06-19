//! IPC type system — `IpcCommand` / `IpcEvent` re-exports + typed errors.
//!
//! **Data flow:**
//!
//! - [`IpcCommand`] — JS → Rust pull-based komutlar (frontend, kullanıcı
//!   etkileşimi, query). Her command bir async handler'a dispatch olur.
//! - [`IpcEvent`] — Rust → JS küçük push olayları (badge, notification,
//!   alert). Büyük state transferi için pull kullanılır (ADR-0012 §3).
//!
//! **Error typing (ADR-0007):** IPC katmanı library boundary'dir — typed
//! [`IpcCommandError`] ve [`IpcEventError`] döndürür. `anyhow` yalnızca
//! application boundary'de (binary main) kullanılır; kütüphane katmanında
//! `?` operatörü tüketici tarafında somut varyantı yakalar.
//!
//! **`#[non_exhaustive]`:** Yeni variant eklemek non-breaking olur (downstream
//! match'lerde `_ =>` kolu zorunlu). AI ve insan tüm varyantları tek seferde
//! eklemek zorunda değil; tüketici kodu yeni varyant geldiğinde `_` koluyla
//! hatasız derlenir.
//!
//! Cross-references:
//! - [`crate::command`] — command tanımları + handler trait.
//! - [`crate::event`] — event enum'ları + `WatchdogKind`.
//! - [`crate::router`] — dispatch implementasyonu.
//! - ADR-0012 §3 — pull-based IPC pattern.

pub use crate::command::*;
pub use crate::event::*;

// ---------------------------------------------------------------------------
// Typed errors (ADR-0007: library boundary typed, application boundary anyhow)
// ---------------------------------------------------------------------------

use thiserror::Error;
use viscos_error::ViscosError;

/// `IpcCommand` handler'larının döndüğü typed hata.
///
/// `#[non_exhaustive]` — yeni varyant eklemek non-breaking. Tüketici exhaustive
/// match yapamaz; `_ =>` kolu bulundurmalı.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IpcCommandError {
    /// Router'a tanıtılmamış command varyantı (default `StubHandler` akışı).
    #[error("unknown IPC command: {0}")]
    UnknownCommand(String),

    /// Payload parse hatası (serde JSON decode).
    #[error("invalid IPC command payload: {0}")]
    BadPayload(#[from] serde_json::Error),

    /// `viscos` alt sistemlerinden biri hata yaydı.
    #[error("internal viscos error: {0}")]
    Internal(#[from] ViscosError),

    /// Bilinçli olarak implemente edilmemiş command varyantı (Faz X.Y stub).
    #[error("not yet implemented: {0}")]
    Unimplemented(&'static str),
}

/// `IpcEvent` yayılımı sırasında oluşan typed hata.
///
/// Channel kapalıysa (consumer drop) veya serialize başarısız olursa bu hata
/// yayılır. Üretim kodunda bu hata genelde warning log'a düşer; event'in
/// kaybolması kritik değildir (state zaten cache'e yazılmış).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IpcEventError {
    /// Event serialize hatası (serde).
    #[error("event serialize error: {0}")]
    Serialize(serde_json::Error),

    /// Receiver tarafı kanalı kapattı (`UnboundedSender::send` hatası).
    #[error("IPC event channel closed (receiver dropped)")]
    ChannelClosed,

    /// Alt sistem hatası.
    #[error("internal viscos error: {0}")]
    Internal(#[from] ViscosError),
}

// ---------------------------------------------------------------------------
// Convenience type aliases
// ---------------------------------------------------------------------------

/// `IpcCommand` handler'larının döndüğü Result tipi.
pub type IpcCommandResult<T> = Result<T, IpcCommandError>;

/// `IpcEvent` yayılımı Result tipi.
pub type IpcEventResult<T> = Result<T, IpcEventError>;

// ---------------------------------------------------------------------------
// From impl'leri
// ---------------------------------------------------------------------------

impl From<tokio::sync::mpsc::error::SendError<IpcEvent>> for IpcEventError {
    fn from(_: tokio::sync::mpsc::error::SendError<IpcEvent>) -> Self {
        Self::ChannelClosed
    }
}

impl From<tokio::sync::mpsc::error::TrySendError<IpcEvent>> for IpcEventError {
    fn from(err: tokio::sync::mpsc::error::TrySendError<IpcEvent>) -> Self {
        match err {
            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                // Kanal dolu → backpressure. ChannelClosed ile aynı kategori:
                // üretim kodu bu hatayı warning olarak loglar.
                Self::ChannelClosed
            }
            tokio::sync::mpsc::error::TrySendError::Closed(_) => Self::ChannelClosed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_command_error_contains_variant_name() {
        let err = IpcCommandError::UnknownCommand("FakeCommand".to_string());
        let s = err.to_string();
        assert!(s.contains("FakeCommand"));
        assert!(s.contains("unknown IPC command"));
    }

    #[test]
    fn bad_payload_error_from_serde() {
        // `serde_json::Error` üretmenin en kolay yolu invalid JSON parse.
        let bad = serde_json::from_str::<serde_json::Value>("{not valid}");
        let serde_err = bad.expect_err("invalid JSON");
        let ipc_err: IpcCommandError = serde_err.into();
        assert!(matches!(ipc_err, IpcCommandError::BadPayload(_)));
    }

    #[test]
    fn internal_error_wraps_viscos_error() {
        let viscos_err = ViscosError::Unimplemented("phase-2.0 unread count");
        let ipc_err: IpcCommandError = viscos_err.into();
        assert!(matches!(ipc_err, IpcCommandError::Internal(_)));
    }

    #[test]
    fn channel_closed_detected_from_send_error() {
        // Tüketici drop edilmiş bir kanal üzerinden send etmek için unbounded
        // kanalı kurup tüketiciyi düşürürüz.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<IpcEvent>();
        drop(rx);
        let send_result = tx.send(IpcEvent::ThemeChanged {
            theme: "dark".into(),
        });
        let send_err = send_result.expect_err("send must fail when rx dropped");
        let ipc_err: IpcEventError = send_err.into();
        assert!(matches!(ipc_err, IpcEventError::ChannelClosed));
    }

    #[test]
    fn unimplemented_command_error_carries_phase_label() {
        let err = IpcCommandError::Unimplemented("phase-2.0 unread count");
        assert_eq!(
            err.to_string(),
            "not yet implemented: phase-2.0 unread count"
        );
    }

    #[test]
    fn type_aliases_resolve() {
        // Compile-time doğrulama: alias'lar doğru hedefe çözülüyor.
        let _ok: IpcCommandResult<()> = Ok(());
        let _ok: IpcEventResult<()> = Ok(());
    }

    #[test]
    fn non_exhaustive_allows_wildcard() {
        // `#[non_exhaustive]` sayesinde tüketici exhaustive match yapamaz;
        // burada sadece varyant oluşturma testi.
        let _ = IpcCommandError::UnknownCommand("X".into());
        let _ = IpcEventError::ChannelClosed;
    }
}
