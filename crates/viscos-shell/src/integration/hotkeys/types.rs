//! Hotkey type definitions — action enum, binding struct, defaults, parser.
//!
//! Faz 6.0'da kullanılan temel hotkey tipleri, varsayılan binding listesi ve
//! `parse_combo` yardımcı fonksiyonu.
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md`](../../../../.cursor/plans/phase-6.0-hotkeys.md)

use global_hotkey::hotkey::HotKey;
use serde::{Deserialize, Serialize};
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
