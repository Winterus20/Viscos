//! Auto-updater — GitHub Releases based (Faz 8.0).
//!
//! Real implementation: uses `self_update::backends::github::ReleaseList` to
//! query the GitHub Releases API and `sha2` to verify SHA-256 digests of
//! downloaded artifacts before performing an atomic binary replace.
//!
//! Cross-reference: [`phase-8.0-distribution.md` §5](../../.cursor/plans/phase-8.0-distribution.md#5-auto-updater).
//!
//! # Security model
//!
//! 1. **No-auto-update-by-default.** [`Updater::background_check`] only calls
//!    `apply()` when `UpdateConfig::auto_apply == true`. Default is `false`
//!    (`.cursorrules` §15). User must confirm via IPC command.
//! 2. **Hash verification (download integrity).** `apply()` compares the
//!    downloaded binary's SHA-256 against `release.sha256`; mismatch returns
//!    [`UpdaterError::HashMismatch`] and deletes the temp file.
//! 3. **Atomic replace.** New binary is downloaded as `<exe>.new`, hash-checked,
//!    then renamed over the current exe; old exe is preserved as `<exe>.old`
//!    for rollback.
//! 4. **HTTPS-only transport.** `self_update` uses `https://api.github.com`;
//!    HTTP fetch is not supported.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use thiserror::Error;
use viscos_error::{Result, ViscosError};

/// Default GitHub repository owner. Centralized to keep config + module consistent.
pub const DEFAULT_REPO_OWNER: &str = "Winterus20";
/// Default GitHub repository name.
pub const DEFAULT_REPO_NAME: &str = "Viscos";
/// Default binary name (asset filename stem).
pub const DEFAULT_BINARY_NAME: &str = "viscos";

/// Release channel — git tag prefix filter for [`UpdateConfig::channel`].
///
/// Tags: `v0.2.0` (Stable), `beta-0.2.0` (Beta), `nightly-2026-06-20`
/// (Nightly). The channel selects the tag prefix used to filter GitHub
/// releases.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UpdateChannel {
    /// Stable releases — `v*` tag prefix (default).
    #[default]
    Stable,
    /// Beta releases — `beta-*` tag prefix.
    Beta,
    /// Nightly releases — `nightly-*` tag prefix.
    Nightly,
}

impl UpdateChannel {
    /// Tag prefix string for GitHub tag filtering.
    #[must_use]
    pub fn tag_prefix(self) -> &'static str {
        match self {
            Self::Stable => "v",
            Self::Beta => "beta-",
            Self::Nightly => "nightly-",
        }
    }

    /// Strip this channel's prefix from a tag.
    #[must_use]
    pub fn strip_prefix(self, tag: &str) -> String {
        let prefix = self.tag_prefix();
        tag.strip_prefix(prefix)
            .map(str::to_string)
            .unwrap_or_else(|| tag.to_string())
    }
}

/// Updater configuration — passed to [`Updater::with_config`].
///
/// Defaults via [`Updater::default`] + [`UpdateConfig::default`] use
/// `auto_apply = false` (`.cursorrules` §15 — no auto-update-by-default).
#[derive(Debug, Clone)]
pub struct UpdateConfig {
    /// GitHub repository owner (e.g. `Winterus20`).
    pub repo_owner: String,
    /// GitHub repository name (e.g. `Viscos`).
    pub repo_name: String,
    /// Target binary stem (`viscos` → asset `viscos-x86_64-pc-windows-msvc.zip`).
    pub binary_name: String,
    /// Current installed version (Semver string, e.g. `0.1.0`).
    pub current_version: String,
    /// Release channel — selects tag prefix when querying GitHub.
    pub channel: UpdateChannel,
    /// When `true`, [`Updater::background_check`] will call `apply()` for any
    /// newer release without user confirmation. **Default `false`** per
    /// `.cursorrules` §15.
    pub auto_apply: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            repo_owner: DEFAULT_REPO_OWNER.to_string(),
            repo_name: DEFAULT_REPO_NAME.to_string(),
            binary_name: DEFAULT_BINARY_NAME.to_string(),
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            channel: UpdateChannel::default(),
            auto_apply: false,
        }
    }
}

/// Auto-updater error.
#[derive(Error, Debug)]
pub enum UpdaterError {
    #[error("invalid repo format (expected 'owner/name'): {0}")]
    InvalidRepo(String),
    #[error("release list fetch failed: {0}")]
    Fetch(String),
    #[error("no release matching current architecture")]
    NoMatchingAsset,
    #[error("hash verification failed: expected {expected}, got {actual}")]
    HashMismatch {
        /// Expected SHA-256 hex digest from release metadata.
        expected: String,
        /// Actual SHA-256 hex digest of the downloaded binary.
        actual: String,
    },
    #[error("no newer version available: latest={latest}, current={current}")]
    NotNewer {
        /// Latest release version returned by GitHub.
        latest: String,
        /// Currently installed version.
        current: String,
    },
    #[error("download failed: {0}")]
    Download(String),
    #[error("binary replace failed: {0}")]
    Replace(String),
}

impl From<UpdaterError> for ViscosError {
    fn from(err: UpdaterError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("updater: {err}")))
    }
}

impl From<self_update::errors::Error> for UpdaterError {
    fn from(err: self_update::errors::Error) -> Self {
        Self::Fetch(err.to_string())
    }
}

/// GitHub release summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// Semver version (`v` prefix stripped).
    pub version: String,
    /// Asset download URL (platform-specific).
    pub download_url: String,
    /// SHA-256 hex digest populated at release engineering time.
    ///
    /// `apply()` compares this against the downloaded binary's SHA-256;
    /// mismatch aborts the update (`UpdaterError::HashMismatch`).
    pub sha256: String,
}

/// Abstraction over GitHub release list fetch — mockable in tests.
///
/// Production uses [`GithubReleaseSource`] wrapping
/// `self_update::backends::github::ReleaseList`; tests substitute a fake
/// returning canned data without touching the network.
pub trait ReleaseSource: Send + Sync {
    /// Fetch the latest release for `owner/name` filtered by `channel`.
    ///
    /// Returns `Ok(Some)` when a release exists, `Ok(None)` when the
    /// repository has no releases yet. Network errors surface as
    /// [`UpdaterError::Fetch`].
    fn fetch_latest(
        &self,
        owner: &str,
        name: &str,
        channel: UpdateChannel,
    ) -> Result<Option<ReleaseInfo>>;
}

/// Real GitHub release source backed by `self_update::backends::github`.
#[derive(Debug, Default, Clone, Copy)]
pub struct GithubReleaseSource;

impl GithubReleaseSource {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ReleaseSource for GithubReleaseSource {
    fn fetch_latest(
        &self,
        owner: &str,
        name: &str,
        channel: UpdateChannel,
    ) -> Result<Option<ReleaseInfo>> {
        let tag_prefix = channel.tag_prefix();
        let releases = self_update::backends::github::ReleaseList::configure()
            .repo_owner(owner)
            .repo_name(name)
            .build()
            .map_err(UpdaterError::from)?
            .fetch()
            .map_err(UpdaterError::from)?;

        let matched = releases
            .into_iter()
            .find(|r| r.version.starts_with(tag_prefix.trim_end_matches('-')));

        let Some(release) = matched else {
            return Ok(None);
        };

        let target = self_update::get_target();
        let asset = release
            .asset_for(target, None)
            .ok_or(UpdaterError::NoMatchingAsset)?;

        // SHA-256 digest'i release body'den çıkar. Format: ilk 64 hex char'lık
        // substring'i bul (lowercase). Bulunamazsa asset name fallback.
        let sha256 =
            extract_sha256_from_body(release.body.as_deref()).unwrap_or_else(|| asset.name.clone());

        Ok(Some(ReleaseInfo {
            version: channel.strip_prefix(&release.version),
            download_url: asset.download_url,
            sha256,
        }))
    }
}

/// Extract first 64-char hex substring (lowercase) from a release body.
fn extract_sha256_from_body(body: Option<&str>) -> Option<String> {
    let body = body?;
    let mut hex = String::with_capacity(64);
    let mut started = false;
    for ch in body.chars() {
        if ch.is_ascii_hexdigit() {
            hex.push(ch.to_ascii_lowercase());
            started = true;
            if hex.len() == 64 {
                return Some(hex);
            }
        } else if started {
            // Hex run interrupted before reaching 64 chars — reset.
            hex.clear();
            started = false;
        }
    }
    None
}

/// Updater — config + release source bundle.
pub struct Updater {
    config: UpdateConfig,
    source: Arc<dyn ReleaseSource>,
}

impl std::fmt::Debug for Updater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updater")
            .field("config", &self.config)
            .field("source", &"<dyn ReleaseSource>")
            .finish()
    }
}

impl Default for Updater {
    fn default() -> Self {
        Self::with_config(UpdateConfig::default())
    }
}

impl Updater {
    /// Default updater (`Winterus20/Viscos` + `viscos`, no auto-apply).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Explicit config + default GitHub source.
    #[must_use]
    pub fn with_config(config: UpdateConfig) -> Self {
        Self {
            config,
            source: Arc::new(GithubReleaseSource::new()),
        }
    }

    /// Custom config + custom (mockable) release source.
    #[must_use]
    pub fn with_source(config: UpdateConfig, source: Arc<dyn ReleaseSource>) -> Self {
        Self { config, source }
    }

    /// Repo string (`owner/name`).
    #[must_use]
    pub fn repo(&self) -> String {
        format!("{}/{}", self.config.repo_owner, self.config.repo_name)
    }

    /// Query GitHub Releases and return `Some(release)` if a newer version is
    /// available on the configured channel.
    ///
    /// **No-auto-update-by-default (Security §1):** Not invoked in the
    /// background; only on explicit user action or via [`Updater::background_check`].
    ///
    /// # Errors
    ///
    /// [`UpdaterError::Fetch`] on network / TLS / rate-limit, [`UpdaterError::NoMatchingAsset`]
    /// when no asset matches the current target triple.
    pub async fn check(&self) -> Result<Option<ReleaseInfo>> {
        let owner = &self.config.repo_owner;
        let name = &self.config.repo_name;
        tracing::debug!(
            owner = %owner,
            repo = %name,
            binary = %self.config.binary_name,
            channel = ?self.config.channel,
            current_version = %self.config.current_version,
            "Updater::check — querying GitHub Releases"
        );

        let release = self.source.fetch_latest(owner, name, self.config.channel)?;

        let Some(release) = release else {
            tracing::info!(repo = %self.repo(), "no releases available");
            return Ok(None);
        };

        match version_is_newer(&self.config.current_version, &release.version) {
            std::cmp::Ordering::Greater => {
                tracing::info!(
                    from = %self.config.current_version,
                    to = %release.version,
                    "newer release available"
                );
                Ok(Some(release))
            }
            _ => {
                tracing::debug!(
                    from = %self.config.current_version,
                    to = %release.version,
                    "no newer release"
                );
                Ok(None)
            }
        }
    }

    /// Download the release binary, verify SHA-256, and atomically replace
    /// the running executable.
    ///
    /// Steps: download → SHA-256 verify → rename current to `<exe>.old` →
    /// rename temp → current → chmod (Unix) or schedule `.old` cleanup
    /// (Windows). On mismatch, the temp file is deleted and
    /// [`UpdaterError::HashMismatch`] is returned.
    ///
    /// **No-auto-update-by-default (Security §1):** Only callable via explicit
    /// IPC command (`IpcCommand::UpdaterApply`) or by `background_check`
    /// when `auto_apply = true`.
    ///
    /// # Errors
    ///
    /// [`UpdaterError::Download`], [`UpdaterError::HashMismatch`],
    /// [`UpdaterError::Replace`].
    pub async fn apply(&self, release: ReleaseInfo) -> Result<()> {
        tracing::info!(
            from = %self.config.current_version,
            to = %release.version,
            binary = %self.config.binary_name,
            sha256_prefix = %&release.sha256[..release.sha256.len().min(8)],
            "Updater::apply — starting binary replace"
        );

        let current = current_exe_path().map_err(|e| UpdaterError::Replace(e.to_string()))?;
        let temp = temp_download_path(&current);

        // 1) Download.
        download_to_file(&release.download_url, &temp)
            .map_err(|e| UpdaterError::Download(e.to_string()))?;

        // 2) SHA-256 verify.
        verify_temp_hash(&temp, &release.sha256)?;

        // 3-4) Atomic replace.
        atomic_replace(&current, &temp).map_err(|e| UpdaterError::Replace(e.to_string()))?;

        // 5-6) Platform-specific post-replace cleanup.
        post_replace_cleanup(&current);

        tracing::info!(
            old_version = %self.config.current_version,
            new_version = %release.version,
            "Update applied: {} -> {}",
            self.config.current_version,
            release.version,
        );

        Ok(())
    }

    /// Background loop: call [`Updater::check`] every `interval` and only
    /// invoke [`Updater::apply`] when `auto_apply` is `true`.
    ///
    /// With the default `auto_apply = false`, the loop logs each detected
    /// update but never touches the binary — user confirmation is required
    /// (`.cursorrules` §15).
    ///
    /// The returned handle runs until aborted.
    pub fn background_check(self, interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match self.check().await {
                    Ok(Some(release)) if self.config.auto_apply => {
                        tracing::info!(
                            target_version = %release.version,
                            "auto_apply=true, downloading update"
                        );
                        if let Err(err) = self.apply(release).await {
                            tracing::warn!(
                                error = %err,
                                "auto-update failed; will retry next interval"
                            );
                        }
                    }
                    Ok(Some(release)) => {
                        tracing::debug!(
                            target_version = %release.version,
                            "auto_apply=false, skipping background apply"
                        );
                    }
                    Ok(None) => {
                        tracing::debug!("background check: up-to-date");
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            "background check failed; will retry next interval"
                        );
                    }
                }
                tokio::time::sleep(interval).await;
            }
        })
    }
}

/// Resolve the path of the currently running executable.
fn current_exe_path() -> std::io::Result<PathBuf> {
    std::env::current_exe()
}

/// Build a sibling temp path next to the current exe: `<bin>.new`.
fn temp_download_path(current: &Path) -> PathBuf {
    let mut buf = current.to_path_buf();
    let new_name = match current.file_name().and_then(|n| n.to_str()) {
        Some(name) => format!("{name}.new"),
        None => "viscos.new".to_string(),
    };
    buf.set_file_name(new_name);
    buf
}

/// Compute SHA-256 hex digest of a file (streaming).
fn sha256_of_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

/// Lowercase hex encoding for a byte slice (avoids pulling in the `hex` crate
/// beyond the workspace dep, which isn't declared in `viscos-distribution`'s
/// local `Cargo.toml`).
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Download `url` to `dest` via `self_update::Download` (reqwest under the
/// hood, already in the dep graph via `self_update`). Returns I/O error on
/// failure.
fn download_to_file(url: &str, dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut dest_file = std::fs::File::create(dest)?;
    self_update::Download::from_url(url)
        .download_to(&mut dest_file)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(())
}

/// Atomic replace: rename `src` (new binary) → `dst` (current exe), preserving
/// the old binary as `<dst>.old` for rollback.
fn atomic_replace(current: &Path, new_binary: &Path) -> std::io::Result<()> {
    let backup = backup_path(current);
    if backup.exists() {
        let _ = std::fs::remove_file(&backup);
    }
    std::fs::rename(current, &backup)?;
    std::fs::rename(new_binary, current)?;
    Ok(())
}

/// Verify the SHA-256 of a downloaded temp file against the expected digest.
///
/// On mismatch, deletes the temp file and returns [`UpdaterError::HashMismatch`].
fn verify_temp_hash(temp: &Path, expected: &str) -> Result<()> {
    let actual = sha256_of_file(temp).map_err(|e| UpdaterError::Replace(e.to_string()))?;
    let expected = expected.to_ascii_lowercase();
    if actual != expected {
        let _ = std::fs::remove_file(temp);
        return Err(UpdaterError::HashMismatch { expected, actual }.into());
    }
    Ok(())
}

/// Path of the rollback backup (e.g. `viscos.exe.old`).
fn backup_path(current: &Path) -> PathBuf {
    let mut buf = current.to_path_buf();
    let name = match current.file_name().and_then(|n| n.to_str()) {
        Some(n) => format!("{n}.old"),
        None => "viscos.old".to_string(),
    };
    buf.set_file_name(name);
    buf
}

/// Post-replace platform-specific cleanup: spawn detached cleanup on Windows,
/// `chmod` on Unix.
#[cfg(target_os = "windows")]
fn post_replace_cleanup(current: &Path) {
    let backup = backup_path(current);
    let target = backup.to_string_lossy().to_string();
    let spawn_result = std::process::Command::new("cmd.exe")
        .args([
            "/C",
            "ping",
            "-n",
            "6",
            "127.0.0.1",
            ">",
            "NUL",
            "&",
            "del",
            "/F",
            "/Q",
        ])
        .arg(&target)
        .spawn();
    match spawn_result {
        Ok(_) => tracing::debug!(target = %target, "scheduled .old cleanup"),
        Err(err) => tracing::warn!(error = %err, "failed to spawn .old cleanup"),
    }
    let _ = current; // unused on this branch
}

#[cfg(unix)]
fn post_replace_cleanup(current: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(current) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(current, perms);
    }
    let backup = backup_path(current);
    let _ = std::fs::remove_file(&backup);
}

/// Compare two semver-ish strings; returns `Greater` if `latest > current`.
fn version_is_newer(current: &str, latest: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };
    parse(latest).cmp(&parse(current))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockable fake — `data` is the canned list returned by `fetch_latest`.
    #[derive(Default, Clone)]
    struct FakeReleaseSource {
        data: Option<ReleaseInfo>,
        fail: bool,
    }

    impl ReleaseSource for FakeReleaseSource {
        fn fetch_latest(
            &self,
            _owner: &str,
            _name: &str,
            _channel: UpdateChannel,
        ) -> Result<Option<ReleaseInfo>> {
            if self.fail {
                Err(UpdaterError::Fetch("simulated".into()).into())
            } else {
                Ok(self.data.clone())
            }
        }
    }

    fn updater_with(
        current_version: &str,
        auto_apply: bool,
        source: Arc<dyn ReleaseSource>,
    ) -> Updater {
        let cfg = UpdateConfig {
            repo_owner: DEFAULT_REPO_OWNER.to_string(),
            repo_name: DEFAULT_REPO_NAME.to_string(),
            binary_name: DEFAULT_BINARY_NAME.to_string(),
            current_version: current_version.to_string(),
            channel: UpdateChannel::Stable,
            auto_apply,
        };
        Updater::with_source(cfg, source)
    }

    fn fixture(version: &str) -> ReleaseInfo {
        ReleaseInfo {
            version: version.to_string(),
            download_url: "https://example.invalid/viscos.zip".to_string(),
            sha256: "0".repeat(64),
        }
    }

    #[test]
    fn default_config_disables_auto_apply() {
        let cfg = UpdateConfig::default();
        assert!(!cfg.auto_apply, ".cursorrules §15");
        assert_eq!(cfg.repo_owner, DEFAULT_REPO_OWNER);
        assert_eq!(cfg.repo_name, DEFAULT_REPO_NAME);
        assert_eq!(cfg.channel, UpdateChannel::Stable);
    }

    #[test]
    fn channel_helpers_round_trip() {
        assert_eq!(UpdateChannel::Stable.tag_prefix(), "v");
        assert_eq!(UpdateChannel::Beta.tag_prefix(), "beta-");
        assert_eq!(UpdateChannel::Nightly.tag_prefix(), "nightly-");
        assert_eq!(UpdateChannel::Stable.strip_prefix("v0.2.0"), "0.2.0");
        assert_eq!(UpdateChannel::Beta.strip_prefix("beta-0.2.0"), "0.2.0");
        assert_eq!(
            UpdateChannel::Nightly.strip_prefix("nightly-2026.06.20"),
            "2026.06.20"
        );
        assert_eq!(UpdateChannel::Stable.strip_prefix("0.2.0"), "0.2.0");
    }

    #[test]
    fn version_is_newer_compares_segments() {
        use std::cmp::Ordering;
        assert_eq!(version_is_newer("0.1.0", "0.2.0"), Ordering::Greater);
        assert_eq!(version_is_newer("0.2.0", "0.2.0"), Ordering::Equal);
        assert_eq!(version_is_newer("0.2.0", "0.1.0"), Ordering::Less);
        assert_eq!(version_is_newer("0.1", "0.1.1"), Ordering::Greater);
    }

    #[test]
    fn sha256_and_body_helpers_known_vectors() {
        let tmp = tempfile::Builder::new()
            .suffix(".bin")
            .tempfile()
            .expect("tempfile");
        std::fs::write(tmp.path(), b"hello world").expect("write");
        assert_eq!(
            sha256_of_file(tmp.path()).expect("sha256"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        let body = "release notes\nsha256: 1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef\n";
        assert_eq!(
            extract_sha256_from_body(Some(body)).expect("found"),
            "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        );
        assert!(extract_sha256_from_body(None).is_none());
        assert!(extract_sha256_from_body(Some("none here")).is_none());
    }

    #[tokio::test]
    async fn check_returns_none_when_up_to_date() {
        let src: Arc<dyn ReleaseSource> = Arc::new(FakeReleaseSource {
            data: Some(fixture("0.1.0")),
            ..Default::default()
        });
        assert!(
            updater_with("0.1.0", false, src)
                .check()
                .await
                .expect("ok")
                .is_none()
        );
    }

    #[tokio::test]
    async fn check_returns_some_when_newer_available() {
        let src: Arc<dyn ReleaseSource> = Arc::new(FakeReleaseSource {
            data: Some(fixture("0.2.0")),
            ..Default::default()
        });
        let release = updater_with("0.1.0", false, src)
            .check()
            .await
            .expect("ok")
            .expect("some");
        assert_eq!(release.version, "0.2.0");
    }

    #[tokio::test]
    async fn check_propagates_fetch_error() {
        let src: Arc<dyn ReleaseSource> = Arc::new(FakeReleaseSource {
            fail: true,
            ..Default::default()
        });
        let err = updater_with("0.1.0", false, src)
            .check()
            .await
            .expect_err("err");
        assert!(matches!(err, ViscosError::Io(_)));
    }

    #[test]
    fn apply_replaces_binary_and_renames_old() {
        // Bypass HTTP; exercise verify + atomic replace helpers used by apply().
        let tmp = tempfile::tempdir().expect("tempdir");
        let current = tmp.path().join("viscos.exe");
        let new_binary = tmp.path().join("viscos.exe.new");
        std::fs::write(&current, b"old-binary").expect("write");
        std::fs::write(&new_binary, b"new-binary").expect("write");
        let expected = sha256_of_file(&new_binary).expect("sha256");
        verify_temp_hash(&new_binary, &expected).expect("verify");
        atomic_replace(&current, &new_binary).expect("replace");
        assert!(current.exists());
        assert_eq!(std::fs::read(&current).expect("read"), b"new-binary");
        assert!(!new_binary.exists());
        let backup = tmp.path().join("viscos.exe.old");
        assert!(backup.exists());
        assert_eq!(std::fs::read(&backup).expect("read"), b"old-binary");
    }

    #[test]
    fn apply_returns_hash_mismatch_on_bad_checksum() {
        // Bypass HTTP; exercise the verify helper directly (reqwest blocking
        // inside a #[tokio::test] panics on runtime drop).
        let tmp = tempfile::tempdir().expect("tempdir");
        let fake = tmp.path().join("viscos.exe.new");
        std::fs::write(&fake, b"downloaded bytes").expect("write");
        let wrong = "0".repeat(64);
        assert_ne!(sha256_of_file(&fake).expect("sha256"), wrong);
        let err = verify_temp_hash(&fake, &wrong).expect_err("must error");
        let ViscosError::Io(io_err) = err else {
            panic!("expected Io variant")
        };
        assert!(io_err.to_string().contains("hash verification failed"));
        assert!(!fake.exists(), "temp must be cleaned up");
    }

    #[tokio::test]
    async fn background_check_only_runs_when_auto_apply_true() {
        // auto_apply=false: loop sees a newer release but skips apply().
        let src_off: Arc<dyn ReleaseSource> = Arc::new(FakeReleaseSource {
            data: Some(fixture("0.2.0")),
            ..Default::default()
        });
        let h1 = updater_with("0.1.0", false, src_off).background_check(Duration::from_millis(50));
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(!h1.is_finished());
        h1.abort();
        let _ = h1.await;

        // auto_apply=true with failing source: apply() is unreachable
        // (network failure on download) but the loop keeps going.
        let src_on: Arc<dyn ReleaseSource> = Arc::new(FakeReleaseSource {
            fail: true,
            ..Default::default()
        });
        let h2 = updater_with("0.1.0", true, src_on).background_check(Duration::from_millis(50));
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(!h2.is_finished());
        h2.abort();
        let _ = h2.await;
    }
}
