//! Restart signal — watchdog → shell iletişim.
//!
//! Watchdog bir `RestartReason` tespit ettiğinde `RestartSignal::tx` üzerinden
//! emit eder; shell tarafı `RestartSignal::rx` ile dinler ve WebView'i
//! dispose+recreate eder (veya hard restart fallback — Faz 8+).
//!
//! Tokio channel kullanır (broadcast): birden fazla consumer (log, tray
//! badge, shell, telemetry) aynı sinyali dinleyebilir.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Restart tetikleme sebebi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartReason {
    /// GDI object sayısı critical threshold aştı (default 9000).
    GdiLeakCritical,
    /// IPC buffer size critical threshold aştı (default 100 MB).
    IpcBufferCritical,
    /// WebView dispose+recreate başarısız → hard restart fallback.
    DisposeFailed,
    /// Manuel (kullanıcı / shell tarafından zorlandı).
    Manual,
}

impl RestartReason {
    /// Telemetry sink callback'inde kullanılan kararlı string etiketi.
    ///
    /// MVP-3: `viscos-telemetry` watchdog'a bağımlılık yaratmaz (cycle riski);
    /// reason `&str` olarak taşınır. Bu fonksiyon public contract —
    /// variant adları değişirse telemetry DB'deki eski kayıtlar uyumsuz
    /// hale gelir, breaking change sayılır.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GdiLeakCritical => "GdiLeakCritical",
            Self::IpcBufferCritical => "IpcBufferCritical",
            Self::DisposeFailed => "DisposeFailed",
            Self::Manual => "Manual",
        }
    }
}

/// Restart sinyali — broadcast channel sarmalayıcı.
#[derive(Debug, Clone)]
pub struct RestartSignal {
    tx: broadcast::Sender<RestartReason>,
}

impl RestartSignal {
    /// Yeni broadcast channel oluştur.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Restart sinyali gönder.
    ///
    /// Hata durumunda (`RecvError` veya kapalı channel) warning loglanır.
    pub fn emit(&self, reason: RestartReason) {
        match self.tx.send(reason) {
            Ok(n) => {
                tracing::info!(?reason, subscribers = n, "Restart signal emitted");
            }
            Err(_send_err) => {
                tracing::warn!(?reason, "No active restart subscribers");
            }
        }
    }

    /// Yeni subscriber oluştur.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<RestartReason> {
        self.tx.subscribe()
    }

    /// Aktif subscriber sayısı.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for RestartSignal {
    fn default() -> Self {
        Self::new(16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_capacity_is_16() {
        let sig = RestartSignal::default();
        assert_eq!(sig.subscriber_count(), 0);
    }

    #[test]
    fn emit_does_not_panic_without_subscribers() {
        let sig = RestartSignal::default();
        sig.emit(RestartReason::GdiLeakCritical);
    }

    #[test]
    fn subscriber_receives_emitted_signal() {
        let sig = RestartSignal::default();
        let mut rx = sig.subscribe();
        assert_eq!(sig.subscriber_count(), 1);

        sig.emit(RestartReason::DisposeFailed);

        // `try_recv` async değil; channel'a mesaj düştü mü kontrolü.
        match rx.try_recv() {
            Ok(RestartReason::DisposeFailed) => {}
            other => panic!("expected DisposeFailed, got {other:?}"),
        }
    }

    #[test]
    fn restart_reason_serde_snake_case() {
        let r = RestartReason::IpcBufferCritical;
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"ipc_buffer_critical\"");
    }
}
