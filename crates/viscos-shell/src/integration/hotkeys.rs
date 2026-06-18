//! Global + window hotkeys (Faz 6.0).
//!
//! Default binding'ler (config'ten override edilebilir):
//! - `Ctrl+Shift+M` → ToggleMute
//! - `Ctrl+Shift+D` → ToggleDeafen
//! - `Ctrl+K` → QuickSwitcher
//! - `Ctrl+,` → OpenSettings
//!
//! Faz 6.0'da `global-hotkey 0.6` (OS çapında) + `muda 0.15` (window-spesific).
//! Faz 5.0'da sadece tip + binding registry; gerçek register Faz 1.6
//! event loop entegrasyonu ile mümkün (CI'da tray gerekli).
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md` §2 Global Hotkeys](../../../.cursor/plans/phase-6.0-hotkeys.md)
//! - `config/default.toml` `[hotkeys]` bölümü.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use viscos_error::ViscosError;

/// Hotkey tetiklendiğinde uygulamanın handle edeceği action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum HotkeyAction {
    /// Mikrofon mute toggle.
    ToggleMute,
    /// Ses (deafen) toggle.
    ToggleDeafen,
    /// Quick switcher aç (Ctrl+K).
    QuickSwitcher,
    /// Settings penceresi aç.
    OpenSettings,
    /// DevTools aç/kapa (F12 / Ctrl+Shift+I).
    ToggleDevtools,
}

impl HotkeyAction {
    /// Action adı (`"toggleMute"` | ...).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ToggleMute => "toggleMute",
            Self::ToggleDeafen => "toggleDeafen",
            Self::QuickSwitcher => "quickSwitcher",
            Self::OpenSettings => "openSettings",
            Self::ToggleDevtools => "toggleDevtools",
        }
    }
}

/// Tek bir hotkey binding'i (combo string + action).
///
/// `combo` sözdizimi: `"Ctrl+Shift+M"`, `"Ctrl+K"`, `"Ctrl+Comma"`.
/// `+` ile ayrılmış modifier veya key. Faz 6.0'da `global-hotkey 0.6`'nın
/// `Code` + `Modifiers` API'sine map edilir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyBinding {
    /// Modifier + key kombinasyonu (örn. `"Ctrl+Shift+M"`).
    pub combo: String,
    /// Tetiklenecek action.
    pub action: HotkeyAction,
}

impl HotkeyBinding {
    /// Yeni binding oluştur.
    #[must_use]
    pub fn new(combo: impl Into<String>, action: HotkeyAction) -> Self {
        Self {
            combo: combo.into(),
            action,
        }
    }
}

/// Default binding listesi — `config/default.toml [hotkeys]` ile senkron.
///
/// Kullanıcı config'ten override ederse `HotkeyManager::register` ile
/// eski binding değiştirilir.
pub const DEFAULT_BINDINGS: &[(&str, HotkeyAction)] = &[
    ("Ctrl+Shift+M", HotkeyAction::ToggleMute),
    ("Ctrl+Shift+D", HotkeyAction::ToggleDeafen),
    ("Ctrl+K", HotkeyAction::QuickSwitcher),
    ("Ctrl+Comma", HotkeyAction::OpenSettings),
];

/// Hotkey yöneticisi — kayıt + event broadcast.
///
/// Faz 6.0 stub: `global-hotkey 0.6` ve `muda 0.15` crate'leri `Cargo.toml`'a
/// dependency olarak eklendi, ancak `register` method'u CI'da gerçek OS
/// hotkey kaydı yapmaz (tray gerekli). State management (kayıtlı binding'ler)
/// test edilebilir; gerçek tuş yakalama Faz 1.6 event loop'unda.
pub struct HotkeyManager {
    /// Kayıtlı binding'ler: action → combo.
    bindings: HashMap<HotkeyAction, String>,
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
        Ok(Self { bindings })
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
            return Err(ViscosError::Media("empty hotkey combo".into()));
        }
        self.bindings.insert(binding.action, binding.combo);
        Ok(())
    }

    /// Action'ın binding'ini kaldır.
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

    /// Event stream (broadcast) — Faz 6.0 stub.
    ///
    /// Gerçek implementasyon: `tokio::sync::broadcast::Sender<HotkeyAction>`
    /// ve `global-hotkey` event loop'undan `Receiver`. Faz 1.0'da stub.
    #[must_use]
    pub fn events(&self) -> HotkeyEventStream {
        HotkeyEventStream::stub()
    }
}

/// Hotkey event stream (broadcast receiver) — Faz 6.0 stub.
///
/// `global-hotkey 0.6` + `muda 0.15` Faz 1.6'da entegre edilecek.
/// Faz 5.0'da sadece tip iskeleti.
pub struct HotkeyEventStream {
    is_stub: bool,
}

impl HotkeyEventStream {
    /// Stub stream oluştur.
    #[must_use]
    pub fn stub() -> Self {
        Self { is_stub: true }
    }

    /// Stub mı?
    #[must_use]
    pub const fn is_stub(&self) -> bool {
        self.is_stub
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_include_mute_and_deafen() {
        let mgr = HotkeyManager::new().unwrap();
        assert_eq!(
            mgr.combo_for(HotkeyAction::ToggleMute),
            Some("Ctrl+Shift+M")
        );
        assert_eq!(
            mgr.combo_for(HotkeyAction::ToggleDeafen),
            Some("Ctrl+Shift+D")
        );
        assert_eq!(mgr.combo_for(HotkeyAction::QuickSwitcher), Some("Ctrl+K"));
        assert_eq!(
            mgr.combo_for(HotkeyAction::OpenSettings),
            Some("Ctrl+Comma")
        );
    }

    #[test]
    fn register_overwrites_existing() {
        let mut mgr = HotkeyManager::new().unwrap();
        mgr.register(HotkeyBinding::new("F1", HotkeyAction::ToggleMute))
            .unwrap();
        assert_eq!(mgr.combo_for(HotkeyAction::ToggleMute), Some("F1"));
    }

    #[test]
    fn register_empty_combo_errors() {
        let mut mgr = HotkeyManager::new().unwrap();
        let result = mgr.register(HotkeyBinding::new("", HotkeyAction::ToggleMute));
        assert!(result.is_err());
    }

    #[test]
    fn unregister_removes_binding() {
        let mut mgr = HotkeyManager::new().unwrap();
        mgr.unregister(HotkeyAction::ToggleMute).unwrap();
        assert!(mgr.combo_for(HotkeyAction::ToggleMute).is_none());
    }

    #[test]
    fn binding_serde_round_trip() {
        let b = HotkeyBinding::new("Ctrl+Shift+M", HotkeyAction::ToggleMute);
        let json = serde_json::to_string(&b).unwrap();
        let back: HotkeyBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn action_serde_camel_case() {
        let json = serde_json::to_string(&HotkeyAction::ToggleDevtools).unwrap();
        assert_eq!(json, "\"toggleDevtools\"");
    }

    #[test]
    fn bindings_vec_contains_all_defaults() {
        let mgr = HotkeyManager::new().unwrap();
        let bindings = mgr.bindings();
        assert_eq!(bindings.len(), DEFAULT_BINDINGS.len());
    }

    #[test]
    fn event_stream_is_stub_in_phase_5() {
        let mgr = HotkeyManager::new().unwrap();
        let stream = mgr.events();
        assert!(stream.is_stub());
    }
}
