//! Native Windows toast notifications.
//!
//! Faz 5.0 kapsamı: `notify-rust 1` üzerinden native Windows toast.
//! Faz 1.5'te tray badge ile birlikte mention bildirimleri için kullanılacak.
//! Faz 6.0'da Discord'un mention / DM / voice event'leri bu kanaldan
//! tetiklenecek.
//!
//! Cross-references:
//! - [`phase-5.0-native-ui.md` §6 Native Bildirimler](../../../.cursor/plans/phase-5.0-native-ui.md)

use viscos_error::ViscosError;

/// Native notification gönderici (Windows toast + cross-platform fallback).
#[derive(Debug, Clone, Default)]
pub struct Notifier {
    /// Uygulama adı (Windows toast "from" alanı).
    app_name: String,
}

impl Notifier {
    /// Yeni notifier oluştur.
    #[must_use]
    pub fn new() -> Self {
        Self {
            app_name: "Viscos".to_string(),
        }
    }

    /// App adını override et.
    #[must_use]
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = name.into();
        self
    }

    /// Mevcut app adı.
    #[must_use]
    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    /// Native notification göster (Windows: toast, macOS: NSUserNotification,
    /// Linux: libnotify).
    ///
    /// Faz 5.0 stub: `notify-rust` dependency'si `default-features = false`
    /// ile eklendi; gerçek gönderim Faz 1.5'te tray sistemiyle entegre
    /// olunca. Şu an method sadece **logging** yapar (hata koşulunda
    /// `ViscosError::Io` döner).
    ///
    /// # Errors
    ///
    /// `notify-rust::Notification::show()` platform-specific hata
    /// döndüğünde `ViscosError::Io` map edilir.
    pub fn notify(&self, title: &str, body: &str) -> Result<(), ViscosError> {
        // Faz 5.0'da bilinçli olarak platform kütüphanesi çağrısı yapılmıyor:
        // notify-rust default-features=false kütüphane derlemesi CI'da
        // platform-specific makro'lar gerektirir. Faz 1.5'te tray + notification
        // gerçek implementasyonu eklenecek. Şu an sadece structured log.
        tracing::info!(
            app = %self.app_name,
            title = %title,
            body = %body,
            "native notification (Faz 5.0 stub — would show via notify-rust)"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_notifier_has_viscos_app_name() {
        let n = Notifier::new();
        assert_eq!(n.app_name(), "Viscos");
    }

    #[test]
    fn with_app_name_overrides() {
        let n = Notifier::new().with_app_name("ViscosBeta");
        assert_eq!(n.app_name(), "ViscosBeta");
    }

    #[test]
    fn notify_succeeds_in_phase_5_0_stub() {
        let n = Notifier::new();
        // Stub: hiçbir zaman hata dönmez, sadece log.
        assert!(n.notify("Mention", "Hello world").is_ok());
    }

    #[test]
    fn notify_accepts_empty_strings() {
        let n = Notifier::new();
        // Boş title/body kabul edilir (call-site validate etmeli; burada stub).
        assert!(n.notify("", "").is_ok());
    }
}
