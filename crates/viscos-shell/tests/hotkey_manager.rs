//! Cross-module integration tests — `viscos-shell::integration::hotkeys`
//! binding + manager state.

use viscos_shell::integration::hotkeys::{
    DEFAULT_BINDINGS, HotkeyAction, HotkeyBinding, HotkeyManager,
};

#[test]
fn default_bindings_contain_five_actions() {
    assert_eq!(DEFAULT_BINDINGS.len(), 5);
    let actions: Vec<HotkeyAction> = DEFAULT_BINDINGS.iter().map(|(_, a)| *a).collect();
    assert!(actions.contains(&HotkeyAction::ToggleMute));
    assert!(actions.contains(&HotkeyAction::ToggleDeafen));
    assert!(actions.contains(&HotkeyAction::QuickSwitcher));
    assert!(actions.contains(&HotkeyAction::OpenSettings));
    assert!(actions.contains(&HotkeyAction::ToggleDevtools));
}

#[test]
fn binding_serializes_to_json() {
    let b = HotkeyBinding::new("Ctrl+Shift+M", HotkeyAction::ToggleMute);
    let json = serde_json::to_string(&b).expect("serialize");
    let back: HotkeyBinding = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, b);
}

#[test]
fn manager_register_and_unregister() {
    let mut mgr = HotkeyManager::new().expect("manager");
    // Override default binding.
    mgr.register(HotkeyBinding::new("F1", HotkeyAction::ToggleMute))
        .expect("register");
    assert_eq!(mgr.combo_for(HotkeyAction::ToggleMute), Some("F1"));

    // Unregister removes the binding.
    mgr.unregister(HotkeyAction::ToggleMute)
        .expect("unregister");
    assert!(mgr.combo_for(HotkeyAction::ToggleMute).is_none());
}

#[test]
fn manager_empty_combo_rejected() {
    let mut mgr = HotkeyManager::new().expect("manager");
    let result = mgr.register(HotkeyBinding::new("", HotkeyAction::ToggleMute));
    assert!(result.is_err(), "empty combo must error");
}

#[test]
fn manager_bindings_list_contains_all_registered() {
    let mut mgr = HotkeyManager::new().expect("manager");
    mgr.register(HotkeyBinding::new("F2", HotkeyAction::ToggleDevtools))
        .expect("register");
    let bindings = mgr.bindings();
    assert!(
        bindings
            .iter()
            .any(|b| b.action == HotkeyAction::ToggleDevtools && b.combo == "F2")
    );
}
