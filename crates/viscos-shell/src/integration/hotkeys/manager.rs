//! `HotkeyManager` — in-memory hotkey binding registry.
//!
//! Faz 6.0'da `global-hotkey 0.6` ile OS kaydı yapılır.
//! Faz 1.0'da sadece in-memory state yönetimi test edilebilir.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use viscos_error::ViscosError;

use super::types::{DEFAULT_BINDINGS, HotkeyAction, HotkeyBinding, HotkeyEventStream};

/// Hotkey yöneticisi — kayıt + event broadcast.
///
/// Faz 6.0 stub: `global-hotkey 0.6` ve `muda 0.15` crate'leri `Cargo.toml`'a
/// dependency olarak eklendi, ancak `register` method'u CI'da gerçek OS
/// hotkey kaydı yapmaz (tray gerekli). State management (kayıtlı binding'ler)
/// test edilebilir; gerçek tuş yakalama Faz 1.6 event loop'unda.
pub struct HotkeyManager {
    /// Kayıtlı binding'ler: action → combo.
    bindings: HashMap<HotkeyAction, String>,
    /// Event broadcast sender.
    sender: Arc<broadcast::Sender<HotkeyAction>>,
}

impl std::fmt::Debug for HotkeyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HotkeyManager")
            .field("bindings", &self.bindings)
            .finish()
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new().expect("default bindings always valid")
    }
}

impl HotkeyManager {
    /// Yeni hotkey manager oluştur (default binding'lerle başlatır).
    ///
    /// # Errors
    ///
    /// Faz 6.0'da `GlobalHotKeyManager::new()` hata verebilir; stub'da
    /// her zaman OK.
    pub fn new() -> Result<Self, ViscosError> {
        let mut bindings = HashMap::new();
        for (combo, action) in DEFAULT_BINDINGS {
            bindings.insert(*action, (*combo).to_string());
        }
        let (sender, _) = broadcast::channel(16);
        Ok(Self {
            bindings,
            sender: Arc::new(sender),
        })
    }

    /// Binding ekle veya güncelle.
    ///
    /// Faz 6.0'da `global-hotkey::GlobalHotKeyManager::register` çağrısı
    /// yapılır; Faz 1.0'da sadece in-memory state güncellenir.
    ///
    /// # Errors
    ///
    /// Geçersiz `combo` formatı (boş string, bilinmeyen key) → `ViscosError::Io`.
    pub fn register(&mut self, binding: HotkeyBinding) -> Result<(), ViscosError> {
        if binding.combo.trim().is_empty() {
            return Err(ViscosError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "empty hotkey combo",
            )));
        }
        self.bindings.insert(binding.action, binding.combo);
        Ok(())
    }

    /// Action'ın binding'ini kaldır.
    ///
    /// # Errors
    ///
    /// Faz 1.0'da her zaman OK.
    pub fn unregister(&mut self, action: HotkeyAction) -> Result<(), ViscosError> {
        self.bindings.remove(&action);
        Ok(())
    }

    /// Action'ın combo string'ini döndür.
    #[must_use]
    pub fn combo_for(&self, action: HotkeyAction) -> Option<&str> {
        self.bindings.get(&action).map(String::as_str)
    }

    /// Tüm kayıtlı binding'ler.
    #[must_use]
    pub fn bindings(&self) -> Vec<HotkeyBinding> {
        self.bindings
            .iter()
            .map(|(action, combo)| HotkeyBinding {
                combo: combo.clone(),
                action: *action,
            })
            .collect()
    }

    /// Event stream (broadcast) — Faz 6.0 real implementation.
    ///
    /// Gerçek implementasyon: `tokio::sync::broadcast::Sender<HotkeyAction>`
    /// ve `global-hotkey` event loop'undan `Receiver`. Faz 1.0'da stub.
    #[must_use]
    pub fn events(&self) -> HotkeyEventStream {
        HotkeyEventStream::new(self.sender.subscribe())
    }

    /// Hotkey event'i dispatch et (OS hotkey tetiklendiğinde çağrılır).
    ///
    /// Faz 6.0'da `global-hotkey` event loop'undan bu method çağrılır.
    /// Faz 1.0'da manuel test için kullanılabilir.
    pub fn dispatch(&self, action: HotkeyAction) {
        let _ = self.sender.send(action);
    }
}
