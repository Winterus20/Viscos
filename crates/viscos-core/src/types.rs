//! Domain types — I/O-free (std + serde).

use serde::{Deserialize, Serialize};

/// Placeholder for Viscos'un global runtime context'i. Faz 1+'ta doldurulacak:
/// - WebView backend handle
/// - Tray + hotkey kayıtları
/// - IPC kanal referansları
/// - Config snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContext {
    pub name: String,
    pub version: String,
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            name: "Viscos".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
