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

mod controller;
mod manager;
mod types;

pub use controller::HotkeyController;
pub use manager::HotkeyManager;
pub use types::{DEFAULT_BINDINGS, HotkeyAction, HotkeyBinding, HotkeyEventStream, parse_combo};

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
