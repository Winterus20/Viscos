//! Chromium flags config loader + deny-list (Faz 8.5).
//!
//! Power user'lar `config.toml`'da `[chromium].flags` altında CEF/WebView2
//! ortak Chromium flag'lerini override edebilir. Bilinen unstable / güvenlik-
//! bypass flag'leri deny-list'te (CI fail ile reddedilir).
//!
//! Cross-references:
//! - [`phase-8.5-cef-backend.md` §3](../../.cursor/plans/phase-8.5-cef-backend.md#3-ileri-seviye-chromium-flags)

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use viscos_error::{Result, ViscosError};

/// Default deny-list — bilinen unstable / güvenlik bypass flag'leri.
pub const DEFAULT_DENY_FLAGS: &[&str] = &[
    "--single-process",            // CEF multi-process zorunlu
    "--disable-gpu",               // GPU olmadan CEF düzgün çalışmaz
    "--disable-web-security",      // CORS bypass — XSS/CSRF riski
    "--ignore-certificate-errors", // MITM saldırılarına açık
    "--allow-running-insecure-content",
    "--no-sandbox",
    "--disable-features=WebOTP", // Faz 2.0'da geri açılabilir
];

/// Chromium flag listesi.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChromiumFlags {
    /// Uygulanacak Chromium komut satırı argümanları.
    pub flags: Vec<String>,
}

impl Default for ChromiumFlags {
    fn default() -> Self {
        Self {
            flags: vec!["--disable-features=msSmartScreenProtection".to_string()],
        }
    }
}

/// Chromium flags config loader hatası.
#[derive(Error, Debug)]
pub enum ChromiumFlagsError {
    #[error("denied flag in user config: {0}")]
    DeniedFlag(String),
    #[error("invalid flag format: {0}")]
    InvalidFormat(String),
}

impl From<ChromiumFlagsError> for ViscosError {
    fn from(err: ChromiumFlagsError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("chromium-flags: {err}")))
    }
}

/// Konfigürasyondaki `[chromium]` bölümünün typed temsili.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChromiumConfigSection {
    #[serde(default)]
    pub flags: Vec<String>,
}

impl ChromiumFlags {
    /// Default flag set ile başlat.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Custom flag listesi ile başlat.
    #[must_use]
    pub fn with_flags(flags: Vec<String>) -> Self {
        Self { flags }
    }

    /// `config.toml`'dan `[chromium]` bölümünü oku.
    ///
    /// Faz 8.5 stub: dosya parse + deny-list enforcement var, varsayılan
    /// `default()` ile başlar. Faz 8.x'te `Config::chromium` field'ı eklenecek.
    ///
    /// # Errors
    ///
    /// Deny-list'teki bir flag `flags` içinde varsa `ChromiumFlagsError::DeniedFlag`
    /// döner.
    pub fn load_from_config() -> Result<Self> {
        let flags = Self::default().flags;

        // Faz 8.5'te Config.chromium henüz opsiyonel; default ile başla.
        // Faz 8.x'te Config struct'ına `chromium: ChromiumConfigSection` eklenince
        // burada `config.chromium.flags` extend edilecek.
        let resolved = Self { flags };
        resolved.validate()?;
        Ok(resolved)
    }

    /// Flag listesinin deny-list'e uygunluğunu kontrol et.
    ///
    /// # Errors
    ///
    /// `ChromiumFlagsError::DeniedFlag` — flag deny-list'te.
    /// `ChromiumFlagsError::InvalidFormat` — flag `--` prefix'i ile başlamıyor.
    pub fn validate(&self) -> Result<()> {
        let deny: HashSet<&str> = DEFAULT_DENY_FLAGS.iter().copied().collect();
        for flag in &self.flags {
            if deny.contains(flag.as_str()) {
                return Err(ChromiumFlagsError::DeniedFlag(flag.clone()).into());
            }
            // Boş flag veya leading `--` eksik → invalid format
            if !flag.starts_with("--") {
                return Err(ChromiumFlagsError::InvalidFormat(flag.clone()).into());
            }
        }
        Ok(())
    }

    /// Flag listesini komut satırı arg vector'ü olarak döndür.
    #[must_use]
    pub fn as_args(&self) -> Vec<&str> {
        self.flags.iter().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_flags_loaded() {
        let flags = ChromiumFlags::default();
        assert!(
            flags
                .flags
                .iter()
                .any(|f| f.contains("msSmartScreenProtection")),
            "default must include msSmartScreenProtection disable flag"
        );
        assert!(
            flags.validate().is_ok(),
            "default flags must pass validation"
        );
    }

    #[test]
    fn load_from_config_returns_valid_default() {
        let flags = ChromiumFlags::load_from_config().expect("load");
        assert!(flags.validate().is_ok());
        assert!(!flags.flags.is_empty());
    }

    #[test]
    fn denied_flag_is_rejected() {
        let flags = ChromiumFlags::with_flags(vec!["--single-process".to_string()]);
        let err = flags.validate().expect_err("must reject");
        match err {
            ViscosError::Io(_) => {}
            other => panic!("expected Io variant, got {other:?}"),
        }
    }

    #[test]
    fn disable_web_security_denied() {
        let flags = ChromiumFlags::with_flags(vec!["--disable-web-security".to_string()]);
        assert!(flags.validate().is_err());
    }

    #[test]
    fn invalid_format_rejected() {
        let flags = ChromiumFlags::with_flags(vec!["no-dashes-flag".to_string()]);
        let err = flags.validate().expect_err("must reject");
        match err {
            ViscosError::Io(_) => {}
            other => panic!("expected Io variant, got {other:?}"),
        }
    }

    #[test]
    fn as_args_returns_string_slices() {
        let flags = ChromiumFlags::with_flags(vec![
            "--disable-features=X".to_string(),
            "--no-first-run".to_string(),
        ]);
        let args = flags.as_args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--disable-features=X");
        assert_eq!(args[1], "--no-first-run");
    }

    #[test]
    fn deny_list_includes_critical_security_flags() {
        let deny: HashSet<&str> = DEFAULT_DENY_FLAGS.iter().copied().collect();
        assert!(deny.contains("--disable-web-security"));
        assert!(deny.contains("--single-process"));
        assert!(deny.contains("--disable-gpu"));
        assert!(deny.contains("--no-sandbox"));
    }
}
