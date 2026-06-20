//! `HotkeyController` — OS hotkey runtime wrapper.
//!
//! On Windows this wraps `global_hotkey::GlobalHotKeyManager` and registers
//! each binding with the OS. On non-Windows the controller is inert.
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md`](../../../../.cursor/plans/phase-6.0-hotkeys.md)

use viscos_error::ViscosError;

use super::manager::HotkeyManager;
use super::types::{HotkeyAction, HotkeyBinding, parse_combo};

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

    /// True iff the OS hotkey runtime is active (non-Windows: always false).
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
