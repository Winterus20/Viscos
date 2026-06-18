//! Placeholder event tipleri — Faz 1+'ta Discord Gateway event'lerini typed olarak sarmalayacak.

use serde::{Deserialize, Serialize};

/// Application-level event (UI → core → backend yönünde).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AppEvent {
    /// Kullanıcı login talep etti.
    LoginRequested,
    /// Sunucu listesi yenileme talebi.
    GuildsRefreshRequested,
}

/// Core-level event (backend → core → UI yönünde).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CoreEvent {
    /// Config yüklendi.
    ConfigLoaded { version: String },
    /// Backend hazır.
    BackendReady,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_event_variants_compile() {
        let _ = AppEvent::LoginRequested;
        let _ = AppEvent::GuildsRefreshRequested;
    }

    #[test]
    fn core_event_variants_compile() {
        let _ = CoreEvent::ConfigLoaded {
            version: "0.1.0".to_string(),
        };
        let _ = CoreEvent::BackendReady;
    }
}
