//! `IpcBuffer` trait — büyük binary blob transfer (Faz 4'te implemente edilecek).
//!
//! Faz 1.0'da stub. Faz 4'te:
//! - WebView2 backend: `CoreWebView2SharedBuffer` API'si.
//! - CEF backend: `SharedMemoryRegion` + `message_router`.
//!
//! # Neden Faz 1'de değil?
//!
//! [`webview2-hardening.md` §3.1](../../.cursor/plans/webview2-hardening.md#31-faz-4-backlog-büyük-bloblar-için-webview2-sharedbuffer)
//! — Faz 1'de pull-based JSON IPC yeterli; avatar/sticker gibi büyük blob'lar
//! Faz 4'te SharedBuffer ile zero-copy transfer edilecek.
//!
//! # Upstream kısıt
//!
//! [`WebView2Feedback #3360`](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3360)
//! — 32.000 × 1MB SharedBuffer sonrası crash, Edge 114+ fix'li.

use viscos_error::{Result, ViscosError};

/// Büyük binary blob transfer trait'i (zero-copy).
///
/// Faz 1.0'da default implementation `ViscosError::Unimplemented` döner.
/// Faz 4'te `viscos-webview::WebViewBackend::post_shared_buffer` ile implemente.
pub trait IpcBuffer: Send + Sync {
    /// Binary blob'u frontend'e gönder.
    ///
    /// # Errors
    ///
    /// Her zaman `ViscosError::Unimplemented("phase-4.0 shared buffer")`.
    fn post(&self, _bytes: &[u8], _metadata: &str) -> Result<()> {
        Err(ViscosError::Unimplemented(
            "post_shared_buffer Faz 4'te implemente edilecek",
        ))
    }
}

/// Default `IpcBuffer` implementasyonu (Faz 1.0 stub).
#[derive(Debug, Default, Clone, Copy)]
pub struct StubBuffer;

impl IpcBuffer for StubBuffer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_buffer_post_returns_unimplemented() {
        let buf = StubBuffer;
        let err = buf.post(b"hello", "metadata").expect_err("must error");
        assert!(matches!(err, ViscosError::Unimplemented(_)));
    }
}
