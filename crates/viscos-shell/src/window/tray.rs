//! `TrayState` — OS tray icon handle + badge update.
//!
//! MVP-3 scaffold: wraps `tray-icon`'s `TrayIcon` on Windows so the rest of
//! the shell can update badge text without depending on `tray-icon` directly.
//! Full menu wiring is Faz 5.0+.

use std::path::{Path, PathBuf};

use viscos_core::VISCOS_VERSION;
use viscos_error::ViscosError;

use super::config::{TrayMenu, TrayMenuItem};

/// System tray state handle (MVP-3 scaffold).
///
/// `TrayState` wraps `tray-icon`'s `TrayIcon` so the rest of the shell can
/// poke at tray metadata (icon path, badge text) without depending on
/// `tray-icon` directly. MVP-3 only exposes badge text updates; full menu
/// wiring is Faz 5.0+.
///
/// On Windows the controller owns the live `tray_icon::TrayIcon` so badge
/// updates reach the OS shell. On other platforms the controller is inert:
/// `new` returns `Err(ViscosError::Unimplemented("tray-icon Windows-only MVP-3"))`
/// because `tray-icon`'s Windows backend is the only one we ship in MVP-3.
#[derive(Debug)]
pub struct TrayState {
    /// Icon path used at construction.
    icon_path: PathBuf,
    /// Current badge text (empty string = no badge).
    badge_text: String,
    /// Real tray icon (Windows only).
    #[cfg(target_os = "windows")]
    rt: Option<TrayIconRt>,
}

#[cfg(target_os = "windows")]
struct TrayIconRt {
    /// OS-owned tray icon handle. `tray-icon` keeps the Win32 icon alive
    /// while this value is live; dropping the `TrayIcon` removes it from
    /// the shell notification area.
    _icon: tray_icon::TrayIcon,
}

#[cfg(target_os = "windows")]
impl std::fmt::Debug for TrayIconRt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `TrayIcon` does not implement `Debug`; the handle is opaque to us
        // anyway. We expose only the structural marker so `Debug` works for
        // the outer `TrayState`.
        f.debug_struct("TrayIconRt").finish_non_exhaustive()
    }
}

impl TrayState {
    /// Construct a new tray state.
    ///
    /// # Errors
    ///
    /// - `ViscosError::Unimplemented("tray-icon Windows-only MVP-3")` on
    ///   non-Windows hosts (by design; Linux/macOS tray support is out of
    ///   MVP-3 scope per `COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md` §3.4).
    /// - `ViscosError::Io` on Windows when `tray-icon` fails to register the
    ///   icon with the shell (very rare; usually a broken explorer.exe).
    #[cfg(target_os = "windows")]
    pub fn new(icon_path: PathBuf) -> Result<Self, ViscosError> {
        use tray_icon::TrayIconBuilder;

        // SAFETY: `TrayIconBuilder::new` allocates a Win32 `NOTIFYICONDATAW`
        // structure and `Shell_NotifyIconW(NIM_ADD, ...)` registers it with
        // the Windows shell. The icon handle returned is owned by the
        // `TrayIcon` wrapper and is removed on `Drop` via the matching
        // `NIM_DELETE` call. We do not share the icon across threads, so
        // `Send + Sync` is satisfied trivially. The icon path is consumed
        // immediately to build the icon resource and is not retained as a
        // raw pointer.
        let icon = TrayIconBuilder::new()
            .with_title("Viscos")
            .build()
            .map_err(|e| {
                ViscosError::Io(std::io::Error::other(format!(
                    "tray-icon build failed: {e}"
                )))
            })?;

        Ok(Self {
            icon_path,
            badge_text: String::new(),
            rt: Some(TrayIconRt { _icon: icon }),
        })
    }

    /// Construct a new tray state (non-Windows placeholder).
    ///
    /// # Errors
    ///
    /// Always returns `ViscosError::Unimplemented("tray-icon Windows-only MVP-3")`.
    /// Callers should feature-gate this call and silently no-op on non-Windows.
    #[cfg(not(target_os = "windows"))]
    pub fn new(_icon_path: PathBuf) -> Result<Self, ViscosError> {
        Err(ViscosError::Unimplemented("tray-icon Windows-only MVP-3"))
    }

    /// Update the tray badge text (unread mention count, status string, etc.).
    ///
    /// On Windows the tray does not have a native "title" slot, so we surface
    /// the badge via the tooltip (`NIF_TIP` / `NOTIFYICONDATAW::szTip`),
    /// which Windows shows on hover — this is the same mechanism Discord
    /// uses for unread badges.
    ///
    /// # Errors
    ///
    /// - `ViscosError::Unimplemented("tray-icon Windows-only MVP-3")` on
    ///   non-Windows hosts.
    /// - `ViscosError::Io` on Windows when the underlying `set_tooltip` call
    ///   fails (very rare; usually a stale icon handle).
    #[cfg(target_os = "windows")]
    pub fn set_badge(&mut self, text: String) -> Result<(), ViscosError> {
        if let Some(rt) = &self.rt {
            // SAFETY: `set_tooltip` updates the `NOTIFYICONDATAW::szTip`
            // member through the `Shell_NotifyIconW(NIM_MODIFY, ...)` API.
            // The icon handle is owned by `rt._icon` for the lifetime of
            // `&self`, so the underlying pointer is guaranteed valid for
            // the duration of this call. We pass `Some(&text)` so the
            // tooltip is updated; passing `None` would clear it.
            rt._icon.set_tooltip(Some(&text)).map_err(|e| {
                ViscosError::Io(std::io::Error::other(format!(
                    "tray-icon set_tooltip failed: {e}"
                )))
            })?;
        }
        self.badge_text = text;
        Ok(())
    }

    /// Update the tray badge text (non-Windows placeholder).
    ///
    /// # Errors
    ///
    /// Always returns `ViscosError::Unimplemented`.
    #[cfg(not(target_os = "windows"))]
    pub fn set_badge(&mut self, _text: String) -> Result<(), ViscosError> {
        Err(ViscosError::Unimplemented("tray-icon Windows-only MVP-3"))
    }

    /// Icon path the controller was created with.
    #[must_use]
    pub fn icon_path(&self) -> &Path {
        &self.icon_path
    }

    /// Last-known badge text.
    #[must_use]
    pub fn badge_text(&self) -> &str {
        &self.badge_text
    }

    /// True iff the OS-level tray runtime is active.
    #[cfg(target_os = "windows")]
    #[must_use]
    pub const fn is_runtime_active(&self) -> bool {
        self.rt.is_some()
    }

    /// True iff the OS-level tray runtime is active (non-Windows: never).
    #[cfg(not(target_os = "windows"))]
    #[must_use]
    pub const fn is_runtime_active(&self) -> bool {
        false
    }
}

/// Default tray menü — status + quit.
///
/// Faz 1.0 stub: `tao::tray::TrayIconBuilder` ile gerçek inşa Faz 1.6'da.
#[must_use]
pub fn default_tray_menu() -> TrayMenu {
    let mut menu = TrayMenu::new();
    menu.push(TrayMenuItem::Label(format!("Viscos v{VISCOS_VERSION}")));
    menu.push(TrayMenuItem::Separator);
    menu.push(TrayMenuItem::Action {
        id: "status".to_string(),
        label: "Online".to_string(),
        enabled: false,
    });
    menu.push(TrayMenuItem::Separator);
    menu.push(TrayMenuItem::Action {
        id: "quit".to_string(),
        label: "Quit".to_string(),
        enabled: true,
    });
    menu
}
