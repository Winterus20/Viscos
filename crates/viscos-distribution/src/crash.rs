//! Opt-in crash reporting (Faz 8.0 stub).
//!
//! Faz 8.0 kapsamı: `minidumper` + `minidumper-cli` entegrasyonunun sadece API
//! yüzeyi. Gerçek crash dump üretimi + reporter URL'ye POST Faz 8.x'te.
//!
//! Privacy default'ları (ADR-0011 / Faz 1.5 telemetry policy ile uyumlu):
//! - Default `opt_in = false` (GDPR uyumlu, kullanıcı açmalı).
//! - Reporter URL boş ise crash dump lokal diske yazılır, gönderilmez.
//! - Metadata'da ASLA token, message content, PII yer almaz.
//!
//! Cross-reference:
//! - [`phase-8.0-distribution.md` §4](../../.cursor/plans/phase-8.0-distribution.md#4-crash-reporting-opt-in)

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use viscos_error::{Result, ViscosError};

/// Crash reporter konfigürasyonu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashConfig {
    /// Kullanıcı opt-in mi? Default false.
    pub opt_in: bool,
    /// Reporter endpoint URL'i. Boş = disabled (lokal diske yaz).
    pub reporter_url: String,
    /// Crash dump klasörü (default: `{data_dir}/crash-dumps`).
    pub dump_dir: PathBuf,
}

impl Default for CrashConfig {
    fn default() -> Self {
        Self {
            opt_in: false,
            reporter_url: String::new(),
            dump_dir: PathBuf::from("%APPDATA%/Viscos/crash-dumps"),
        }
    }
}

/// Opt-in durumu — UI ve log için.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrashOptInStatus {
    /// Reporter URL boş → lokal dump only, network gönderim yok.
    Disabled,
    /// Kullanıcı opt-in etti + reporter URL set.
    Enabled,
    /// Reporter URL var ama kullanıcı opt-out.
    OptedOut,
}

/// Crash reporter hatası.
#[derive(Error, Debug)]
pub enum CrashError {
    #[error("crash dump directory creation failed: {0}")]
    DumpDir(String),
    #[error("minidumper initialization failed: {0}")]
    Init(String),
}

impl From<CrashError> for ViscosError {
    fn from(err: CrashError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("crash: {err}")))
    }
}

/// Crash reporter handle (Faz 8.0 stub).
///
/// Faz 8.x'te `minidumper::Minidumper::new()` + `Loop::spawn()` + global panic
/// hook'u entegre edecek. Faz 8.0'da sadece config'i loglar ve opt-in durumunu raporlar.
#[derive(Debug, Clone)]
pub struct CrashReporter {
    config: CrashConfig,
}

impl CrashReporter {
    /// Yeni crash reporter.
    #[must_use]
    pub fn new(config: CrashConfig) -> Self {
        Self { config }
    }

    /// Config'ten default `CrashReporter` oluştur.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(CrashConfig::default())
    }

    /// Crash reporter'ı başlat.
    ///
    /// Faz 8.0 stub: `opt_in` false ise init'i skip eder, sadece loglar.
    /// Faz 8.x'te `minidumper` ile global panic hook kurulacak.
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda dump dir
    /// oluşturma hatası `CrashError::DumpDir` olarak döner.
    pub fn init(&self) -> Result<()> {
        let status = self.opt_in_status();
        tracing::info!(
            opt_in = self.config.opt_in,
            reporter_configured = !self.config.reporter_url.is_empty(),
            status = ?status,
            dump_dir = ?self.config.dump_dir,
            "CrashReporter::init stub — minidumper entegrasyonu Faz 8.x'te"
        );
        // TODO(Faz 8.x): minidumper::Minidumper::new(&self.config.dump_dir)?
        //   + std::panic::set_hook(Box::new(|info| { ... }));
        Ok(())
    }

    /// Config'ten opt-in durumunu hesapla.
    ///
    /// `reporter_url` boş → `Disabled`. `opt_in` true → `Enabled`. Diğer → `OptedOut`.
    #[must_use]
    pub fn opt_in_status(&self) -> CrashOptInStatus {
        if self.config.reporter_url.is_empty() {
            CrashOptInStatus::Disabled
        } else if self.config.opt_in {
            CrashOptInStatus::Enabled
        } else {
            CrashOptInStatus::OptedOut
        }
    }

    /// Mevcut config'i döndür.
    #[must_use]
    pub const fn config(&self) -> &CrashConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled_and_opted_out() {
        let cfg = CrashConfig::default();
        assert!(!cfg.opt_in);
        assert!(cfg.reporter_url.is_empty());
    }

    #[test]
    fn default_reporter_is_disabled_when_url_empty() {
        let reporter = CrashReporter::with_defaults();
        assert_eq!(reporter.opt_in_status(), CrashOptInStatus::Disabled);
    }

    #[test]
    fn reporter_with_url_and_opt_in_is_enabled() {
        let cfg = CrashConfig {
            opt_in: true,
            reporter_url: "https://crash.example.com/ingest".to_string(),
            dump_dir: PathBuf::from("/tmp/viscos-crashes"),
        };
        let reporter = CrashReporter::new(cfg);
        assert_eq!(reporter.opt_in_status(), CrashOptInStatus::Enabled);
    }

    #[test]
    fn reporter_with_url_but_opt_out_is_opted_out() {
        let cfg = CrashConfig {
            opt_in: false,
            reporter_url: "https://crash.example.com/ingest".to_string(),
            dump_dir: PathBuf::from("/tmp/viscos-crashes"),
        };
        let reporter = CrashReporter::new(cfg);
        assert_eq!(reporter.opt_in_status(), CrashOptInStatus::OptedOut);
    }

    #[test]
    fn init_stub_succeeds_without_io() {
        let reporter = CrashReporter::with_defaults();
        assert!(reporter.init().is_ok());
    }

    #[test]
    fn config_accessor_returns_same_reference() {
        let reporter = CrashReporter::with_defaults();
        let cfg1 = reporter.config();
        let cfg2 = reporter.config();
        assert!(std::ptr::eq(cfg1, cfg2));
    }
}
