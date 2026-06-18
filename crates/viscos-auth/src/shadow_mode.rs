//! 24h shadow mode (Faz 1.5 preview).
//!
//! **Senaryo:** Yeni kullanıcı ilk login olduğunda, ilk 24 saat içinde
//! Viscos **read-only** modunda çalışır — sadece REST okuma, mesaj yazma
//! **bloke**. Amaç: ani bir hesap hatası/yanlış kullanım ban tetiklemeden
//! önce kullanıcı UX'i öğrensin.
//!
//! **Faz 1.5'te:** Telemetry ile entegre olur (yazma denemeleri trace'lenir).
//!
//! **Faz 2.0'da:** Stub — kullanıcı `config.toml`'da opt-out yapabilir,
//! `ShadowMode::is_active()` her zaman `false` (default config: skip).

use std::time::{Duration, SystemTime};

/// Shadow mode süresi (24 saat).
pub const SHADOW_MODE_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

/// Shadow mode durumu — uygulama başlangıcında bir kez hesaplanır.
#[derive(Debug, Clone)]
pub struct ShadowMode {
    login_at: SystemTime,
    expires_at: SystemTime,
    /// v1'de her zaman `false` (Faz 1.5 telemetry entegrasyonu için hook).
    telemetry_armed: bool,
}

impl ShadowMode {
    /// Login anından itibaren 24 saatlik shadow mode penceresi aç.
    pub fn new(login_at: SystemTime) -> Self {
        let expires_at = login_at + SHADOW_MODE_DURATION;
        Self {
            login_at,
            expires_at,
            telemetry_armed: false,
        }
    }

    /// Opt-in shadow mode devre dışı (telemetry zaten bağlıyken veya
    /// kullanıcı bilinçli olarak atlamak istediğinde).
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            login_at: SystemTime::UNIX_EPOCH,
            expires_at: SystemTime::UNIX_EPOCH,
            telemetry_armed: false,
        }
    }

    /// Shadow mode aktif mi? (24 saat geçmemiş ve devre dışı bırakılmamış).
    pub fn is_active(&self) -> bool {
        if self.expires_at == SystemTime::UNIX_EPOCH {
            return false;
        }
        SystemTime::now()
            .duration_since(self.login_at)
            .map(|d| d < SHADOW_MODE_DURATION)
            .unwrap_or(false)
    }

    /// Write API'sine izin var mı?
    ///
    /// **Kural:** Shadow mode aktifken tüm write path'leri (`create_message`,
    /// `update_message`, `delete_message`, `create_reaction`, vs.) shell
    /// katmanında bloklanmalı.
    #[must_use]
    pub fn allows_write(&self) -> bool {
        !self.is_active()
    }

    /// Shadow mode bitişine kalan süre.
    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        if !self.is_active() {
            return None;
        }
        self.expires_at.duration_since(SystemTime::now()).ok()
    }

    /// Faz 1.5 telemetry entegrasyonu için hook (v1'de no-op).
    pub fn arm_telemetry(&mut self) {
        self.telemetry_armed = true;
    }

    /// Test amaçlı: login anını override et (24h boundary testleri).
    #[cfg(test)]
    pub fn with_login_at(mut self, login_at: SystemTime) -> Self {
        self.login_at = login_at;
        self.expires_at = login_at + SHADOW_MODE_DURATION;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_shadow_mode_is_active_at_t_zero() {
        let now = SystemTime::now();
        let sm = ShadowMode::new(now);
        assert!(sm.is_active());
        assert!(!sm.allows_write());
    }

    #[test]
    fn disabled_never_active() {
        let sm = ShadowMode::disabled();
        assert!(!sm.is_active());
        assert!(sm.allows_write());
        assert!(sm.remaining().is_none());
    }

    #[test]
    fn expired_shadow_allows_write() {
        let now = SystemTime::now();
        // 25 saat önce login → shadow mode süresi dolmuş.
        let past = now
            .checked_sub(Duration::from_secs(25 * 60 * 60))
            .unwrap_or(now);
        let sm = ShadowMode::new(past);
        assert!(!sm.is_active());
        assert!(sm.allows_write());
    }

    #[test]
    fn boundary_just_before_24h_still_active() {
        let now = SystemTime::now();
        // 23h 59m önce → hâlâ aktif.
        let past = now
            .checked_sub(Duration::from_secs((24 * 60 * 60) - 60))
            .unwrap_or(now);
        let sm = ShadowMode::new(past);
        assert!(
            sm.is_active(),
            "shadow mode should still be active at 23h59m"
        );
    }

    #[test]
    fn boundary_just_after_24h_expired() {
        let now = SystemTime::now();
        // 24h 1s önce → süresi dolmuş.
        let past = now
            .checked_sub(Duration::from_secs((24 * 60 * 60) + 1))
            .unwrap_or(now);
        let sm = ShadowMode::new(past);
        assert!(!sm.is_active());
    }

    #[test]
    fn remaining_decreases_within_window() {
        let now = SystemTime::now();
        let past = now
            .checked_sub(Duration::from_secs(60 * 60)) // 1 saat önce
            .unwrap_or(now);
        let sm = ShadowMode::new(past);
        let r = sm.remaining().expect("remaining in 23h window");
        // 22h 59m ile 23h 1m arasında olmalı.
        let lower = Duration::from_secs((22 * 60 * 60) + 59 * 60);
        let upper = Duration::from_secs((23 * 60 * 60) + 60);
        assert!(
            r >= lower && r <= upper,
            "remaining out of expected range: {r:?}"
        );
    }
}
