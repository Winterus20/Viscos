//! Windows auto-start — `auto-launch 0.5` (Faz 6.0).
//!
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` altına
//! `Viscos --minimized` registry entry ekler. Settings UI'dan toggle
//! edilebilir; default: **disabled** (Faz 6.0 karar noktası — bilinçli
//! kullanıcı tercihi).
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md` §5 Auto-Start](../../../.cursor/plans/phase-6.0-hotkeys.md)

use viscos_error::ViscosError;

/// Windows auto-start controller.
///
/// Faz 6.0 stub: `auto-launch 0.5` `Cargo.toml`'a dependency olarak
/// eklendi; gerçek enable/disable Faz 1.6 event loop + tray sistemiyle
/// entegre olunca. Faz 1.0'da method'lar `Ok(())` döner (no-op stub).
#[derive(Debug, Clone, Default)]
pub struct AutoLaunch;

impl AutoLaunch {
    /// Yeni auto-start controller oluştur.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Auto-start'ı etkinleştir.
    ///
    /// Faz 6.0 stub: gerçek `AutoLaunchBuilder::set_app_name("Viscos")`
    /// çağrısı Faz 1.6'da. Şu an OK döner.
    ///
    /// # Errors
    ///
    /// Faz 1.0'da hiçbir zaman hata dönmez.
    pub fn enable() -> Result<(), ViscosError> {
        tracing::info!("AutoLaunch::enable() called (Faz 6.0 stub — no-op)");
        Ok(())
    }

    /// Auto-start'ı devre dışı bırak.
    ///
    /// # Errors
    ///
    /// Faz 1.0'da hiçbir zaman hata dönmez.
    pub fn disable() -> Result<(), ViscosError> {
        tracing::info!("AutoLaunch::disable() called (Faz 6.0 stub — no-op)");
        Ok(())
    }

    /// Auto-start etkin mi?
    ///
    /// Faz 1.0 stub'ında her zaman `false` döner. Faz 6.0'da `auto-launch`
    /// `is_enabled()` çağrısı yapılır.
    pub fn is_enabled() -> Result<bool, ViscosError> {
        tracing::debug!("AutoLaunch::is_enabled() called (Faz 6.0 stub — returns false)");
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_succeeds_in_stub() {
        assert!(AutoLaunch::enable().is_ok());
    }

    #[test]
    fn disable_succeeds_in_stub() {
        assert!(AutoLaunch::disable().is_ok());
    }

    #[test]
    fn is_enabled_returns_false_in_stub() {
        let result = AutoLaunch::is_enabled().unwrap();
        assert!(!result);
    }

    #[test]
    fn new_creates_default() {
        let _ = AutoLaunch::new();
    }
}
