//! Auto-updater — GitHub Releases tabanlı (Faz 8.0 stub).
//!
//! Faz 8.0 kapsamı: `Updater::check()` mocklanabilir release listesini parse eder;
//! `apply()` sadece `self_update::backends::github::Update::configure()` setup eder.
//! Faz 8.x release engineering'inde gerçek binary download / atomic replace.
//!
//! Cross-reference:
//! - [`phase-8.0-distribution.md` §5](../../.cursor/plans/phase-8.0-distribution.md#5-auto-updater)

use serde::{Deserialize, Serialize};
use thiserror::Error;
use viscos_error::{Result, ViscosError};

use crate::{DEFAULT_BINARY_NAME, DEFAULT_REPO};

/// Auto-updater hatası.
#[derive(Error, Debug)]
pub enum UpdaterError {
    #[error("invalid repo format (expected 'owner/name'): {0}")]
    InvalidRepo(String),
    #[error("release list fetch failed: {0}")]
    Fetch(String),
    #[error("no release matching current architecture")]
    NoMatchingAsset,
}

impl From<UpdaterError> for ViscosError {
    fn from(err: UpdaterError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("updater: {err}")))
    }
}

/// GitHub release özet bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// Semver tag (ör. `v0.2.0` veya `0.2.0` — `strip_v` ile normalize).
    pub version: String,
    /// Asset download URL'i (platforma uygun — `viscos-x86_64-pc-windows-msvc.zip`).
    pub download_url: String,
    /// SHA-256 hex digest (release engineering sırasında doldurulur).
    pub sha256: String,
}

/// Updater konfigürasyonu — repo + binary adı.
#[derive(Debug, Clone)]
pub struct Updater {
    /// `owner/name` formatında GitHub repository.
    pub repo: String,
    /// Hedef binary adı (asset filename'in stem'i).
    pub binary_name: String,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

impl Updater {
    /// Default config: Winterus20/Viscos + viscos binary.
    #[must_use]
    pub fn new() -> Self {
        Self {
            repo: DEFAULT_REPO.to_string(),
            binary_name: DEFAULT_BINARY_NAME.to_string(),
        }
    }

    /// Custom repo + binary ile override.
    #[must_use]
    pub fn with_repo(repo: impl Into<String>, binary_name: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            binary_name: binary_name.into(),
        }
    }

    /// Repo owner/name parçalarını döndür.
    ///
    /// # Errors
    ///
    /// Repo string `/` içermiyorsa `UpdaterError::InvalidRepo` döner.
    pub fn repo_parts(&self) -> Result<(&str, &str)> {
        let (owner, name) = self
            .repo
            .split_once('/')
            .ok_or_else(|| UpdaterError::InvalidRepo(self.repo.clone()))?;
        Ok((owner, name))
    }

    /// Mevcut sürümden yeni bir GitHub release var mı kontrol et.
    ///
    /// Faz 8.0 stub: gerçek `ReleaseList::configure().build().fetch()` çağrısı yerine
    /// `Ok(None)` döner ve tracing ile bilgilendirir. Faz 8.x release engineering'inde
    /// `self_update::backends::github::Update::configure()` ile gerçek fetch yapılacak.
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda network/parse
    /// hataları `UpdaterError::Fetch` olarak dönecek.
    pub async fn check(&self) -> Result<Option<ReleaseInfo>> {
        let (owner, name) = self.repo_parts()?;
        tracing::debug!(
            owner = owner,
            repo = name,
            binary = %self.binary_name,
            "Updater::check stub — GitHub release listesi Faz 8.x'te çekilecek"
        );
        // TODO(Faz 8.x): self_update::backends::github::ReleaseList::configure()
        //   .repo_owner(owner).repo_name(name).build()?.fetch()? →
        //   bump_is_greater(&current_version, &release.version) karşılaştırması.
        Ok(None)
    }

    /// Belirtilen release'i indir + mevcut binary'yi atomik olarak değiştir.
    ///
    /// Faz 8.0 stub: sadece `self_update::backends::github::Update::configure()` setup
    /// çağrısının varlığını doğrular; binary replace YAPMAZ.
    ///
    /// # Errors
    ///
    /// Şu an stub olduğu için hata dönmez; gerçek implementasyonda download/verify
    /// hataları `UpdaterError` varyantları olarak dönecek.
    pub async fn apply(&self, release: ReleaseInfo) -> Result<()> {
        let (owner, name) = self.repo_parts()?;
        tracing::info!(
            owner = owner,
            repo = name,
            version = %release.version,
            binary = %self.binary_name,
            sha256_prefix = %&release.sha256[..8.min(release.sha256.len())],
            "Updater::apply stub — release engineering Faz 8.x'te"
        );

        // Stub: self_update backend setup'ın compile-time doğruluğu için burada bir
        // kez `Update::configure()` çağrısını builder olarak çalıştırıyoruz. Bu çağrı
        // network yapmaz; sadece config nesnesini validate eder.
        let _config = self_update::backends::github::Update::configure();
        tracing::debug!("self_update backend configured (no-op apply in Faz 8.0)");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uses_default_repo_and_binary() {
        let updater = Updater::new();
        assert_eq!(updater.repo, "Winterus20/Viscos");
        assert_eq!(updater.binary_name, "viscos");
    }

    #[test]
    fn with_repo_overrides() {
        let updater = Updater::with_repo("octocat/Hello-World", "hello");
        assert_eq!(updater.repo, "octocat/Hello-World");
        assert_eq!(updater.binary_name, "hello");
    }

    #[test]
    fn repo_parts_parses_owner_and_name() {
        let updater = Updater::new();
        let (owner, name) = updater.repo_parts().expect("parse");
        assert_eq!(owner, "Winterus20");
        assert_eq!(name, "Viscos");
    }

    #[test]
    fn repo_parts_errors_on_missing_slash() {
        let updater = Updater::with_repo("no-slash-here", "x");
        let err = updater.repo_parts().expect_err("must error");
        match err {
            ViscosError::Io(_) => {}
            other => panic!("expected Io variant, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn check_returns_none_in_phase_8_0_stub() {
        let updater = Updater::new();
        let result = updater.check().await.expect("check stub");
        assert!(result.is_none(), "stub must return Ok(None)");
    }

    #[tokio::test]
    async fn apply_succeeds_with_minimal_release_info() {
        let updater = Updater::new();
        let release = ReleaseInfo {
            version: "0.2.0".to_string(),
            download_url:
                "https://github.com/Winterus20/Viscos/releases/download/v0.2.0/viscos.zip"
                    .to_string(),
            sha256: "abcdef0123456789".repeat(4),
        };
        updater.apply(release).await.expect("apply stub");
    }
}
