//! Backend trait placeholder — Faz 1+'ta WebViewBackend, AuthBackend, GatewayBackend
//! gibi somut trait'ler burada tanımlanacak.

use viscos_error::Result;

/// Genel backend kontratı: başlat, durdur, sağlık kontrolü.
///
/// Faz 1+'ta doldurulacak:
/// - `WebViewBackend` (wry/CEF)
/// - `GatewayBackend` (twilight-gateway)
/// - `AuthBackend` (keyring-core)
pub trait Backend: Send + Sync {
    /// Backend'i başlat (async).
    fn start(&self) -> Result<()>;

    /// Backend'i düzenli olarak durdur (graceful shutdown).
    fn stop(&self) -> Result<()>;

    /// Sağlık kontrolü — watchdog Faz 1'de bunu periyodik çağırır.
    fn health(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test backend — trait sözleşmesinin somut implementasyonu.
    struct DummyBackend;

    impl Backend for DummyBackend {
        fn start(&self) -> Result<()> {
            Ok(())
        }

        fn stop(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn dummy_backend_satisfies_trait() {
        let b = DummyBackend;
        assert!(b.start().is_ok());
        assert!(b.stop().is_ok());
        assert!(b.health().is_ok());
    }
}
