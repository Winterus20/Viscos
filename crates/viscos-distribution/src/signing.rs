//! Authenticode code signing (Faz 8.0 release engineering stub).
//!
//! Faz 8.0 kapsamı: API yüzeyi + stub. Gerçek `signtool.exe` invocation Faz 8.x
//! release engineering'inde.
//!
//! Production stratejisi (Faz 8.0 karar noktası, ADR-0006 ile uyumlu):
//! - v1: self-signed (ücretsiz, "Unknown publisher" uyarısı).
//! - v2: OV sertifika ($200-400/yıl, SmartScreen OK).
//! - v3: EV sertifika ($400-800/yıl, anında trust).
//!
//! Cross-reference:
//! - [`phase-8.0-distribution.md` §6](../../.cursor/plans/phase-8.0-distribution.md#6-code-signing-windows-authenticode)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use viscos_error::{Result, ViscosError};

/// Code signer konfigürasyonu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerConfig {
    /// `.pfx` sertifika path'i.
    pub cert_path: PathBuf,
    /// Sertifika parolası (env var'dan okunmalı — commit edilmez).
    pub cert_password_env: String,
    /// RFC 3161 timestamp authority URL'i.
    pub timestamp_url: String,
    /// Hash algoritması (`sha256` | `sha384` | `sha512`).
    pub hash_algorithm: String,
}

impl SignerConfig {
    /// Default `signtool` timestamp URL'i (DigiCert).
    #[must_use]
    pub const fn default_timestamp_url() -> &'static str {
        "http://timestamp.digicert.com"
    }

    /// v1 önerisi: self-signed, SHA-256.
    #[must_use]
    pub fn self_signed_default() -> Self {
        Self {
            cert_path: PathBuf::from("certs/viscos-dev.pfx"),
            cert_password_env: "VISCOS_CERT_PASSWORD".to_string(),
            timestamp_url: Self::default_timestamp_url().to_string(),
            hash_algorithm: "sha256".to_string(),
        }
    }
}

/// Code signing hatası.
#[derive(Error, Debug)]
pub enum SignerError {
    #[error("signtool executable not found in PATH")]
    SigntoolMissing,
    #[error("certificate file missing: {0}")]
    CertMissing(PathBuf),
    #[error("signtool invocation failed: {0}")]
    InvocationFailed(String),
}

impl From<SignerError> for ViscosError {
    fn from(err: SignerError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("signer: {err}")))
    }
}

/// Code signer handle (Faz 8.0 stub).
#[derive(Debug, Clone)]
pub struct CodeSigner {
    config: SignerConfig,
}

impl CodeSigner {
    /// Yeni code signer.
    #[must_use]
    pub fn new(config: SignerConfig) -> Self {
        Self { config }
    }

    /// Self-signed default config ile signer.
    #[must_use]
    pub fn self_signed() -> Self {
        Self::new(SignerConfig::self_signed_default())
    }

    /// Binary'yi `signtool.exe` ile imzala.
    ///
    /// Faz 8.0 stub: sadece `tracing::info!` ile bilgilendirir, gerçek imzalama YAPMAZ.
    /// Faz 8.x release engineering'inde:
    /// ```ignore
    /// let status = Command::new("signtool")
    ///     .args(["sign", "/tr", &timestamp_url, "/td", &hash_algorithm,
    ///             "/fd", &hash_algorithm, "/f", cert_path_str, "/p", password,
    ///             binary_path_str])
    ///     .status()?;
    /// ```
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda
    /// `SignerError` varyantları (`SigntoolMissing`, `CertMissing`,
    /// `InvocationFailed`) dönecek.
    pub fn sign(&self, binary: &Path) -> Result<()> {
        tracing::info!(
            binary = ?binary,
            cert = ?self.config.cert_path,
            timestamp = %self.config.timestamp_url,
            hash = %self.config.hash_algorithm,
            password_env = %self.config.cert_password_env,
            "CodeSigner::sign stub — signtool.exe invocation Faz 8.x release engineering'inde"
        );
        Ok(())
    }

    /// Config'i döndür.
    #[must_use]
    pub const fn config(&self) -> &SignerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_signed_default_has_safe_values() {
        let cfg = SignerConfig::self_signed_default();
        assert_eq!(cfg.hash_algorithm, "sha256");
        assert!(cfg.timestamp_url.starts_with("http"));
        assert!(!cfg.cert_password_env.is_empty());
    }

    #[test]
    fn default_timestamp_url_is_digicert() {
        assert_eq!(
            SignerConfig::default_timestamp_url(),
            "http://timestamp.digicert.com"
        );
    }

    #[test]
    fn sign_stub_returns_ok() {
        let signer = CodeSigner::self_signed();
        let result = signer.sign(Path::new("target/release/viscos.exe"));
        assert!(result.is_ok());
    }

    #[test]
    fn config_accessor_returns_same_reference() {
        let signer = CodeSigner::self_signed();
        let cfg1 = signer.config();
        let cfg2 = signer.config();
        assert!(std::ptr::eq(cfg1, cfg2));
    }
}
