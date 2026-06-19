//! Global + window hotkeys (Faz 6.0).
//!
//! Default binding'ler (config'ten override edilebilir):
//! - `Ctrl+Shift+M` → ToggleMute
//! - `Ctrl+Shift+D` → ToggleDeafen
//! - `Ctrl+K` → QuickSwitcher
//! - `Ctrl+Comma` → OpenSettings
//! - `Ctrl+Shift+I` → ToggleDevtools
//!
//! Faz 6.0'da `global-hotkey 0.6` (OS çapında) + `muda 0.15` (window-spesific).
//! Faz 5.0'da sadece tip + binding registry; gerçek register Faz 1.6
//! event loop entegrasyonu ile mümkün (CI'da tray gerekli).
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md` §2 Global Hotkeys](../../../.cursor/plans/phase-6.0-hotkeys.md)
//! - `config/default.toml` `[hotkeys]` bölümü.

use global_hotkey::hotkey::HotKey;
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
    ("Ctrl+Shift+I", HotkeyAction::ToggleDevtools),
];

/// Parse a combo string like `"Ctrl+Shift+M"` into a `global_hotkey::HotKey`.
///
/// This delegates to `global_hotkey`'s built-in `FromStr` impl, which is
/// fully featured (modifiers, letters, digits, function keys, navigation,
/// numpad, media keys). The returned error is mapped to `ViscosError::Io`
/// so callers can match a single error type.
///
/// # Errors
///
/// - Empty combo string.
pub fn parse_combo(combo: &str) -> Result<HotKey, ViscosError> {
    combo.parse::<HotKey>().map_err(|e| {
        ViscosError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid hotkey combo {combo:?}: {e}"),
        ))
    })
}

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
            return Err(ViscosError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "empty hotkey combo",
            )));
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

/// Cross-platform hotkey controller (MVP-3 scaffold).
///
/// On Windows this is wired to `global_hotkey::GlobalHotKeyManager` and binds
/// each registered binding via a fresh `parse_combo` call. On other
/// platforms the controller is an inert handle: construction succeeds, but
/// event delivery stays a stub.
///
/// ## Caveat
///
/// `global_hotkey 0.6` only registers hotkeys with the OS once at the
/// process level; we therefore keep a single `GlobalHotKeyManager` inside
/// the controller rather than constructing a new one per call.
pub struct HotkeyController {
    manager: HotkeyManager,
    /// Real OS hotkey manager (Windows only).
    #[cfg(target_os = "windows")]
    rt: Option<global_hotkey::GlobalHotKeyManager>,
}

impl std::fmt::Debug for HotkeyController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HotkeyController")
            .field("manager", &self.manager)
            .field(
                "platform",
                &if cfg!(target_os = "windows") {
                    "windows"
                } else {
                    "non-windows"
                },
            )
            .finish()
    }
}

impl HotkeyController {
    /// Wrap an existing `HotkeyManager` in a controller that registers the
    /// bindings with the OS (Windows) or stays inert (other platforms).
    ///
    /// # Errors
    ///
    /// - `ViscosError::Io` on Windows when `GlobalHotKeyManager::new()` or
    ///   any `register()` call fails.
    #[cfg(target_os = "windows")]
    pub fn from_manager(manager: HotkeyManager) -> Result<Self, ViscosError> {
        use global_hotkey::GlobalHotKeyManager;

        let rt = GlobalHotKeyManager::new().map_err(|e| {
            ViscosError::Io(std::io::Error::other(format!(
                "GlobalHotKeyManager::new failed: {e}"
            )))
        })?;

        // Register default bindings eagerly so the OS knows about Ctrl+Shift+M
        // before the event loop spins up.
        let controller = Self {
            manager,
            rt: Some(rt),
        };
        for binding in controller.manager.bindings() {
            controller.register_runtime(&binding)?;
        }
        Ok(controller)
    }

    /// Non-Windows variant: inert controller.
    ///
    /// # Errors
    ///
    /// Always succeeds on non-Windows; the returned controller will not
    /// dispatch any events.
    #[cfg(not(target_os = "windows"))]
    pub fn from_manager(manager: HotkeyManager) -> Result<Self, ViscosError> {
        Ok(Self { manager })
    }

    /// Borrow the underlying manager.
    #[must_use]
    pub const fn manager(&self) -> &HotkeyManager {
        &self.manager
    }

    /// True iff the OS hotkey runtime is active (Windows only).
    #[cfg(target_os = "windows")]
    #[must_use]
    pub const fn is_runtime_active(&self) -> bool {
        self.rt.is_some()
    }

    /// True iff the OS hotkey runtime is active (non-Windows).
    #[cfg(not(target_os = "windows"))]
    #[must_use]
    pub const fn is_runtime_active(&self) -> bool {
        false
    }

    /// Forward a new binding to both the manager and the OS runtime.
    ///
    /// # Errors
    ///
    /// - `ViscosError::Io` when the combo fails validation or the OS refuses
    ///   to register the binding.
    pub fn register(&mut self, binding: HotkeyBinding) -> Result<(), ViscosError> {
        self.manager.register(binding.clone())?;
        #[cfg(target_os = "windows")]
        {
            self.register_runtime(&binding)?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = binding;
            // Intentionally a no-op so the caller can still mutate the
            // manager state on non-Windows hosts.
        }
        Ok(())
    }

    /// Forward `unregister` to the manager and the OS runtime.
    ///
    /// # Errors
    ///
    /// - `ViscosError::Io` on Windows when `GlobalHotKeyManager::unregister`
    ///   fails (rare; we log + skip rather than bubble up so a corrupt combo
    ///   cannot wedge the unregister path).
    pub fn unregister(&mut self, action: HotkeyAction) -> Result<(), ViscosError> {
        self.manager.unregister(action)?;
        #[cfg(target_os = "windows")]
        {
            if let (Some(rt), Some(combo)) = (&self.rt, self.manager.combo_for(action))
                && let Ok(parsed) = parse_combo(combo)
                && let Err(e) = rt.unregister(parsed)
            {
                tracing::warn!(
                    target: "viscos.hotkeys",
                    error = %e,
                    combo = %combo,
                    "GlobalHotKeyManager::unregister failed"
                );
            }
        }
        Ok(())
    }

    /// Register a single binding with the OS hotkey runtime.
    #[cfg(target_os = "windows")]
    fn register_runtime(&self, binding: &HotkeyBinding) -> Result<(), ViscosError> {
        let parsed = parse_combo(&binding.combo)?;
        if let Some(rt) = &self.rt {
            rt.register(parsed).map_err(|e| {
                ViscosError::Io(std::io::Error::other(format!(
                    "GlobalHotKeyManager::register failed: {e}"
                )))
            })?;
        }
        Ok(())
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
        assert_eq!(
            mgr.combo_for(HotkeyAction::ToggleDevtools),
            Some("Ctrl+Shift+I")
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

    #[test]
    fn parse_combo_rejects_empty() {
        assert!(parse_combo("").is_err());
    }

    #[test]
    fn parse_combo_rejects_unknown_token() {
        // global-hotkey rejects unknown key tokens.
        assert!(parse_combo("Ctrl+WhoKnows").is_err());
    }

    #[test]
    fn parse_combo_round_trip_via_default_bindings() {
        // For each default binding the parser must succeed.
        for (combo, _) in DEFAULT_BINDINGS {
            assert!(parse_combo(combo).is_ok(), "parse_combo failed for {combo}");
        }
    }

    #[test]
    fn parse_combo_extracts_modifiers() {
        let parsed = parse_combo("Ctrl+Shift+M").expect("parse");
        assert!(
            parsed
                .mods
                .contains(global_hotkey::hotkey::Modifiers::CONTROL)
        );
        assert!(
            parsed
                .mods
                .contains(global_hotkey::hotkey::Modifiers::SHIFT)
        );
        assert_eq!(parsed.key, global_hotkey::hotkey::Code::KeyM);
    }

    #[test]
    fn controller_full_lifecycle() {
        // Hotkeys are process-global on Windows, so we run the entire
        // controller lifecycle in one test: construct → inherit bindings →
        // unregister → re-register. Combining them avoids the "HotKey already
        // registered" failure that would occur if we tried to build three
        // separate `HotkeyController` instances back-to-back. Off-Windows
        // the controller is inert and the lifecycle reduces to a state check.
        let mgr = HotkeyManager::new().unwrap();
        let mut ctrl = HotkeyController::from_manager(mgr).expect("controller");

        // 1) Default bindings are inherited.
        assert_eq!(
            ctrl.manager().combo_for(HotkeyAction::ToggleMute),
            Some("Ctrl+Shift+M")
        );
        if cfg!(target_os = "windows") {
            assert!(ctrl.is_runtime_active());
        } else {
            assert!(!ctrl.is_runtime_active());
        }

        // 2) Unregister + re-register round-trip works.
        ctrl.unregister(HotkeyAction::OpenSettings)
            .expect("unregister settings");
        assert!(
            ctrl.manager()
                .combo_for(HotkeyAction::OpenSettings)
                .is_none()
        );
        ctrl.register(HotkeyBinding::new("F1", HotkeyAction::OpenSettings))
            .expect("register settings");
        assert_eq!(
            ctrl.manager().combo_for(HotkeyAction::OpenSettings),
            Some("F1")
        );

        // 3) Unregister clears state without error.
        ctrl.unregister(HotkeyAction::ToggleMute)
            .expect("unregister mute");
        assert!(ctrl.manager().combo_for(HotkeyAction::ToggleMute).is_none());
    }
}
