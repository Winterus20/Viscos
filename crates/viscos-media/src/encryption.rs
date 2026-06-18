//! AES-256-GCM media encryption with OS-keyring-backed key storage.
//!
//! v1 targets Windows (Credential Manager via `keyring-core` +
//! `windows-native-keyring-store`). Cross-platform is intentionally a stub —
//! other backends (linux-secret-service, macOS Keychain) will be added in
//! later phases with their own `cfg` blocks.

use aes_gcm::aead::rand_core::{OsRng, RngCore};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

use crate::cache::{EncryptedMediaBlob, MediaError};

/// Wraps an `Aes256Gcm` cipher. Key material never leaves this struct
/// (zeroized on drop once we move to `Zeroizing` in a later patch).
pub struct MediaKey {
    cipher: Aes256Gcm,
}

impl MediaKey {
    /// Build a `MediaKey` from a 32-byte raw key. Used by tests and by
    /// `from_keyring` after hex-decoding the stored entry.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new(&bytes.into()),
        }
    }

    /// Generate a fresh 32-byte key using the OS RNG.
    pub fn generate() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Windows: load the encryption key from the OS Credential Manager,
    /// creating a fresh one on first use.
    #[cfg(target_os = "windows")]
    pub async fn from_keyring() -> Result<Self, MediaError> {
        use keyring_core::Entry;

        // keyring-core is sync; we wrap in spawn_blocking in the caller if needed.
        // For v1 we just call it inline — Credential Manager reads are <50 ms.
        let entry = Entry::new("Viscos", "cache_encryption_key")
            .map_err(|e| MediaError::Keyring(e.to_string()))?;

        let key_bytes = match entry.get_password() {
            Ok(hex_str) => {
                let bytes = hex::decode(&hex_str)
                    .map_err(|e| MediaError::Keyring(format!("hex decode: {e}")))?;
                if bytes.len() != 32 {
                    return Err(MediaError::Keyring(format!(
                        "invalid key length: {} (expected 32)",
                        bytes.len()
                    )));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                arr
            }
            Err(_) => {
                let new_key = Self::generate();
                let hex_str = hex::encode(new_key);
                entry
                    .set_password(&hex_str)
                    .map_err(|e| MediaError::Keyring(e.to_string()))?;
                new_key
            }
        };

        Ok(Self::from_bytes(key_bytes))
    }

    /// Non-Windows platforms: explicit stub. v1 ships Windows-only.
    #[cfg(not(target_os = "windows"))]
    pub async fn from_keyring() -> Result<Self, MediaError> {
        Err(MediaError::Keyring(
            "keyring not supported on this platform in v1".to_string(),
        ))
    }

    /// Encrypt `plaintext` with a fresh random nonce.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedMediaBlob, MediaError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| MediaError::Encryption(e.to_string()))?;
        Ok(EncryptedMediaBlob {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Decrypt `blob` back to plaintext.
    pub fn decrypt(&self, blob: &EncryptedMediaBlob) -> Result<Vec<u8>, MediaError> {
        let nonce = Nonce::from_slice(&blob.nonce);
        self.cipher
            .decrypt(nonce, blob.ciphertext.as_ref())
            .map_err(|e| MediaError::Encryption(e.to_string()))
    }
}
