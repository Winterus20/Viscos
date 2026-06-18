//! Hybrid disk + RAM media cache (foyer), plus signed-URL TTL index (moka).
//!
//! API surface is intentionally narrow: `open`, `get`, `put`, `put_with_url`,
//! `get_url_meta`. All encryption / keyring concerns live in `encryption.rs`.

use std::path::Path;
use std::time::{Duration, SystemTime};

use foyer::HybridCache;
use foyer_storage::DirectFsDeviceOptions;
use moka::sync::Cache as MokaCache;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use viscos_error::ViscosError;

use crate::encryption::MediaKey;

/// Library boundary error for media operations. Convertible to [`ViscosError::Media`].
#[derive(Debug, Error)]
pub enum MediaError {
    /// Media blob not present in either memory or disk tier.
    #[error("media not found: {0}")]
    NotFound(u64),

    /// AES-GCM encrypt/decrypt failed (corrupted blob, wrong key, …).
    #[error("encryption error: {0}")]
    Encryption(String),

    /// foyer hybrid cache internal failure.
    #[error("foyer error: {0}")]
    Foyer(String),

    /// OS keyring read/write failed (missing entry, permissions, …).
    #[error("keyring error: {0}")]
    Keyring(String),

    /// Discord CDN transport failure (network, HTTP status, …).
    #[error("cdn error: {0}")]
    Cdn(String),

    /// Filesystem I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<MediaError> for ViscosError {
    fn from(e: MediaError) -> Self {
        ViscosError::Media(e.to_string())
    }
}

/// Discord signed-URL metadata tracked alongside each encrypted blob.
#[derive(Clone, Debug)]
pub struct CdnUrlMeta {
    /// Last known signed URL.
    pub url: String,
    /// Server-side expiry (unix epoch seconds → SystemTime).
    pub expires_at: SystemTime,
}

/// On-disk representation: AES-GCM nonce + ciphertext, no AAD.
///
/// `Serialize + Deserialize` are required by foyer's `StorageValue` bound
/// (the disk tier materializes blobs via serde).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedMediaBlob {
    /// 96-bit nonce (unique per blob, generated from `OsRng`).
    pub nonce: [u8; 12],
    /// AES-256-GCM ciphertext (includes Poly1305 tag at the end).
    pub ciphertext: Vec<u8>,
}

/// Hybrid cache: foyer (RAM + disk) for blobs, moka for URL metadata.
///
/// **Note:** foyer 0.11 builder API — `DirectFsDeviceOptions { dir, capacity,
/// file_size }` requires `file_size` and `capacity` to be 4 KiB-aligned
/// multiples. We size `file_size` at 64 MiB and round capacity to the nearest
/// multiple of `file_size`.
pub struct MediaCache {
    blobs: HybridCache<u64, EncryptedMediaBlob>,
    url_meta: MokaCache<u64, CdnUrlMeta>,
    key: MediaKey,
}

/// 4 KiB alignment floor required by foyer's direct-IO devices.
const ALIGN: usize = 4096;
/// Region file size for the on-disk tier. 64 MiB balances file-descriptor
/// count vs. write amplification.
const REGION_FILE_SIZE: usize = 64 * 1024 * 1024;

impl MediaCache {
    /// Open or create the hybrid cache rooted at `disk_path` (created if missing).
    ///
    /// # Errors
    ///
    /// [`MediaError::Keyring`] if the OS keyring refuses access; [`MediaError::Foyer`]
    /// if the disk tier cannot be initialized; [`MediaError::Io`] for directory creation.
    pub async fn open(disk_path: &Path) -> Result<Self, MediaError> {
        std::fs::create_dir_all(disk_path)?;

        let key = MediaKey::from_keyring().await?;

        // 32 MB RAM tier + 10 GB disk tier (ADR-0010 §B table).
        // Capacity must be a multiple of file_size; both must be 4 KiB-aligned.
        let memory_bytes: usize = 32 * 1024 * 1024;
        let disk_bytes: usize = 10 * 1024 * 1024 * 1024;
        let file_size = REGION_FILE_SIZE;
        // Round disk_bytes down to a multiple of file_size.
        let disk_capacity = (disk_bytes / file_size) * file_size;

        let device = DirectFsDeviceOptions {
            dir: disk_path.to_path_buf(),
            capacity: disk_capacity,
            file_size,
        };

        let blobs: HybridCache<u64, EncryptedMediaBlob> = foyer::HybridCacheBuilder::new()
            .memory(memory_bytes)
            .storage()
            .with_device_config(device)
            .build()
            .await
            .map_err(|e| MediaError::Foyer(e.to_string()))?;

        // Sync moka cache so `get_url_meta` can be a sync fn (per spec).
        let url_meta: MokaCache<u64, CdnUrlMeta> = MokaCache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(3600))
            .build();

        // Silence unused-import warning on ALIGN.
        let _ = ALIGN;

        Ok(Self {
            blobs,
            url_meta,
            key,
        })
    }

    /// Fetch and decrypt a blob by attachment id. `None` on cache miss.
    ///
    /// # Errors
    ///
    /// [`MediaError::Foyer`] on tier read failure; [`MediaError::Encryption`] if the
    /// stored blob was encrypted with a different key (or is corrupted).
    pub async fn get(&self, attachment_id: u64) -> Result<Option<Vec<u8>>, MediaError> {
        let Some(blob) = self
            .blobs
            .get(&attachment_id)
            .await
            .map_err(|e| MediaError::Foyer(e.to_string()))?
        else {
            return Ok(None);
        };
        let plaintext = self.key.decrypt(&blob)?;
        Ok(Some(plaintext))
    }

    /// Encrypt + insert (overwrites on conflict).
    ///
    /// # Errors
    ///
    /// [`MediaError::Encryption`] if RNG or AEAD seal fails (extremely rare;
    /// `OsRng` should not fail on a healthy OS).
    pub async fn put(&self, attachment_id: u64, plaintext: &[u8]) -> Result<(), MediaError> {
        let blob = self.key.encrypt(plaintext)?;
        self.blobs.insert(attachment_id, blob);
        Ok(())
    }

    /// Convenience: put the blob and record its current signed-URL metadata.
    pub async fn put_with_url(
        &self,
        attachment_id: u64,
        plaintext: &[u8],
        url: &str,
        expires_at: SystemTime,
    ) -> Result<(), MediaError> {
        self.put(attachment_id, plaintext).await?;
        self.url_meta.insert(
            attachment_id,
            CdnUrlMeta {
                url: url.to_string(),
                expires_at,
            },
        );
        Ok(())
    }

    /// Synchronous lookup of the latest signed URL + expiry for an attachment.
    pub fn get_url_meta(&self, attachment_id: u64) -> Option<CdnUrlMeta> {
        self.url_meta.get(&attachment_id)
    }
}
