//! Keyring-backed token storage (ADR-0011).
//!
//! **Mimari:** `keyring-core 1.0` + `windows-native-keyring-store 1.1`
//! (DPAPI arkası). `default-features = false` → `regex` dependency'si
//! alınmaz (~1+ MB binary tasarrufu).
//!
//! **Multi-account v1'den itibaren:** `keyring user = user_id` (Discord
//! snowflake) ile her hesap ayrı entry. v1 UI single-account gösterir, v2.0
//! `keyring-core`'un `search` feature'ı açılarak list UI eklenir (0 refactor).
//!
//! **Bellek hijyeni:** `SecretString` (`secrecy::SecretBox<str>`) +
//! `ZeroizeOnDrop` tüm secret material.
//!
//! **Storage format:** Her entry bir JSON serialize edilmiş `SerializedAccount`.
//! İçinde: `token`, `mfa_backup_hashes` (v1'de plaintext, v2.0'da Argon2 PHC),
//! `super_properties` (varsa), `created_at`, `last_validated_at`.

use std::time::{SystemTime, UNIX_EPOCH};

use keyring_core::{Entry, Error as KeyringError, set_default_store, unset_default_store};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::SERVICE_NAME;

/// Auth katmanı hata modeli — `viscos_error::ViscosError::Auth`'a propagate eder.
///
/// `#[non_exhaustive]` — yeni varyant eklemek non-breaking. Tüketici `_ =>` kolu
/// bulundurmalı.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuthError {
    #[error("keyring error: {0}")]
    Keyring(#[from] KeyringError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("no account found for user_id {0}")]
    AccountNotFound(String),
    #[error("mfa error: {0}")]
    Mfa(String),
    #[error("token validation failed: {0}")]
    ValidationFailed(String),
    #[error("platform not yet supported: {0}")]
    UnsupportedPlatform(String),
    #[error("keyring store not initialized (call AuthStorage::install() at startup)")]
    StoreNotInitialized,
    /// Token format sanity check başarısız (format, geçerlilik değil).
    ///
    /// Gerçek geçerlilik `GET /users/@me` ile doğrulanır (ADR-0011 §2).
    #[error("invalid Discord token format: expected <base64>.<timestamp>.<hmac>")]
    InvalidTokenFormat,
    /// Ağ çağrısı başarısız (Discord REST veya MFA endpoint).
    #[error("network error: {0}")]
    NetworkError(String),
    /// Depolama operasyonu başarısız (keyring dışı storage katmanı için).
    #[error("store failed: {0}")]
    StoreFailed(String),
    /// MFA akışı başarısız (TOTP üretim veya doğrulama hatası).
    #[error("MFA failed: {0}")]
    MfaFailed(String),
}

/// Bellek-dwell hesap state'i. `ZeroizeOnDrop` ile drop anında zeroize.
///
/// **`Debug` derives YOK:** `SecretString` `Debug` impl'i var ama hassas veri
/// sızıntısı riskine karşı struct düzeyinde `Debug` yok. Print/log için
/// explicit `format!("user_id={}", acc.user_id)` kullanılır.
#[derive(ZeroizeOnDrop)]
pub struct StoredAccount {
    pub user_id: String, // Discord snowflake (string form)
    pub username: String,
    pub token: SecretString,
    /// MFA backup code hash'leri (v1: plaintext, v2.0: Argon2 PHC).
    pub mfa_backup_hashes: Vec<SecretString>,
    /// Opsiyonel: hesabın kendine özel X-Super-Properties override'ı.
    /// Hesap başına farklı fingerprint kullanılması discord tarafından
    /// suspicious olabilir; default None.
    #[zeroize(skip)]
    pub super_properties: Option<serde_json::Value>,
    #[zeroize(skip)]
    pub created_at: i64,
    #[zeroize(skip)]
    pub last_validated_at: i64,
}

/// Disk formatı — keyring entry'sinde JSON.
///
/// `ZeroizeOnDrop` derive'lanmıyor çünkü `serde_json::Value` `Zeroize` trait'ini
/// implemente etmiyor (iç içe `Map`/`Number` generic'lerini zeroize etmek
/// non-trivial). Bunun yerine manuel `Drop` impl'i primitive String/Vec
/// alanlarını zeroize eder. `serde_json::Value` (Option tarafı) zaten drop
/// anında recursive Drop çağırır, bu yeterli defense-in-depth.
#[derive(Serialize, Deserialize)]
struct SerializedAccount {
    user_id: String,
    username: String,
    token: String,
    mfa_backup_hashes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    super_properties: Option<serde_json::Value>,
    created_at: i64,
    last_validated_at: i64,
}

impl Drop for SerializedAccount {
    fn drop(&mut self) {
        // Primitive plaintext alanlarını zeroize et.
        self.token.zeroize();
        for h in &mut self.mfa_backup_hashes {
            h.zeroize();
        }
        // user_id, username: not secret, no zeroize.
        // super_properties: serde_json::Value — Drop otomatik recursive,
        // ama value içeriği String tutabilir. Best-effort: i32/f64/Null/Bool
        // branch'ler için no-op; String branch için explicit zeroize edemiyoruz
        // çünkü serde_json::Value private String alanı açmaz. Trade-off kabul.
    }
}

/// Keyring storage facade. Store runtime'da global; struct helper metotlar için.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthStorage {
    // Marker field — gelecekte per-instance config (ör. test mock store ref).
    _private: (),
}

impl AuthStorage {
    /// Platform-specific default store'u kur.
    ///
    /// **v1:** Sadece Windows native (DPAPI arkası). Linux/macOS v2.0'da.
    pub fn install() -> Result<(), AuthError> {
        #[cfg(target_os = "windows")]
        {
            use windows_native_keyring_store::Store;
            // Store::new() Result<Arc<Store>, Error>; set_default_store(Arc<Store>) -> ().
            let store = Store::new().map_err(AuthError::Keyring)?;
            set_default_store(store);
            info!("keyring-core: windows-native (DPAPI) store initialized");
            Ok(())
        }
        #[cfg(target_os = "macos")]
        {
            Err(AuthError::UnsupportedPlatform(
                "macos (apple-native-keyring-store) is v2.0 opt-in".to_string(),
            ))
        }
        #[cfg(target_os = "linux")]
        {
            Err(AuthError::UnsupportedPlatform(
                "linux (dbus-secret-service-keyring-store) is v2.0 opt-in".to_string(),
            ))
        }
    }

    /// Global store'u kaldır (test senaryolarında izolasyon).
    pub fn shutdown() {
        unset_default_store();
    }

    /// Yeni `AuthStorage` handle.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Hesabı keyring'e yaz (üzerine yazar).
    pub fn store_account(&self, account: &StoredAccount) -> Result<(), AuthError> {
        let entry = Entry::new(SERVICE_NAME, &account.user_id)?;
        let ser = SerializedAccount {
            user_id: account.user_id.clone(),
            username: account.username.clone(),
            token: account.token.expose_secret().to_string(),
            mfa_backup_hashes: account
                .mfa_backup_hashes
                .iter()
                .map(|s| s.expose_secret().to_string())
                .collect(),
            super_properties: account.super_properties.clone(),
            created_at: account.created_at,
            last_validated_at: account.last_validated_at,
        };
        let json = serde_json::to_string(&ser)?;
        entry.set_password(&json)?;
        info!(user_id = %account.user_id, "keyring account stored");
        Ok(())
    }

    /// Hesabı keyring'den oku (yoksa `Ok(None)`).
    pub fn load_account(&self, user_id: &str) -> Result<Option<StoredAccount>, AuthError> {
        let entry = Entry::new(SERVICE_NAME, user_id)?;
        match entry.get_password() {
            Ok(json) => {
                // Clone yerine std::mem::take ile taşı: `token` ve
                // `mfa_backup_hashes` Drop anında zeroize edilecek, biz onları
                // deserialize sonrası SecretString'e taşıyıp bırakmış oluyoruz.
                // user_id/username clone küçük string, kabul.
                let mut ser: SerializedAccount = serde_json::from_str(&json)?;
                Ok(Some(StoredAccount {
                    user_id: std::mem::take(&mut ser.user_id),
                    username: std::mem::take(&mut ser.username),
                    token: SecretString::new(std::mem::take(&mut ser.token).into_boxed_str()),
                    mfa_backup_hashes: std::mem::take(&mut ser.mfa_backup_hashes)
                        .into_iter()
                        .map(|s| SecretString::new(s.into_boxed_str()))
                        .collect(),
                    super_properties: ser.super_properties.take(),
                    created_at: ser.created_at,
                    last_validated_at: ser.last_validated_at,
                }))
            }
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    /// Hesabı keyring'den sil (yoksa no-op).
    pub fn delete_account(&self, user_id: &str) -> Result<(), AuthError> {
        let entry = Entry::new(SERVICE_NAME, user_id)?;
        match entry.delete_credential() {
            Ok(()) => {
                info!(user_id, "keyring account deleted");
                Ok(())
            }
            Err(KeyringError::NoEntry) => {
                // No-op: zaten yok.
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// 401 alındığında çağrılır. Hesabı siler, shell "tekrar giriş" UI'ı açar.
    pub fn handle_401(&self, user_id: &str) -> Result<(), AuthError> {
        warn!(user_id, "Discord 401 received — invalidating token");
        self.delete_account(user_id)
    }

    /// MFA backup code doğrula.
    ///
    /// **v1:** plaintext hash karşılaştırma (zaman-bağımsız).
    /// **v2.0:** Argon2 PHC verify.
    pub fn verify_backup_code(&self, user_id: &str, code: &str) -> Result<bool, AuthError> {
        let account = self
            .load_account(user_id)?
            .ok_or_else(|| AuthError::AccountNotFound(user_id.to_string()))?;
        for hash in &account.mfa_backup_hashes {
            if crate::mfa::verify_backup_code(code, hash.expose_secret()) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Multi-account altyapısı için stub. v1'de tek active account UI'da,
    /// `list_user_ids()` sadece kendisine parametre olarak verilen tek user_id'yi
    /// döner. v2.0'da `keyring-core`'un `search` feature'ı açılır.
    pub fn list_user_ids(&self, known_user_ids: &[String]) -> Vec<String> {
        known_user_ids
            .iter()
            .filter(|uid| {
                Entry::new(SERVICE_NAME, uid)
                    .and_then(|e| e.get_password())
                    .is_ok()
            })
            .cloned()
            .collect()
    }
}

impl Default for StoredAccount {
    fn default() -> Self {
        let now = now_unix();
        Self {
            user_id: String::new(),
            username: String::new(),
            token: SecretString::new(String::new().into_boxed_str()),
            mfa_backup_hashes: Vec::new(),
            super_properties: None,
            created_at: now,
            last_validated_at: now,
        }
    }
}

/// Unix timestamp (saniye).
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// Keyring-core 1.0 Result re-export (call sites'ı kısaltmak için).
#[allow(dead_code)]
type KeyringResultAlias = Result<(), KeyringError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_id_validation_helper() {
        use crate::user_id_is_valid;
        assert!(user_id_is_valid("123456789012345678"));
        assert!(!user_id_is_valid(""));
        assert!(!user_id_is_valid("abc"));
        // 21 chars — over the 20-char snowflake limit
        assert!(!user_id_is_valid("123456789012345678901"));
        // 20 chars (Discord snowflake max) — valid
        assert!(user_id_is_valid("12345678901234567890"));
    }

    #[test]
    fn now_unix_is_recent() {
        let t = now_unix();
        assert!(t > 1_700_000_000, "now_unix returned suspicious value: {t}");
    }

    #[test]
    fn serialized_account_roundtrip_json() {
        let ser = SerializedAccount {
            user_id: "111".to_string(),
            username: "tester".to_string(),
            token: "tok-123".to_string(),
            mfa_backup_hashes: vec!["ABCD-1234".to_string()],
            super_properties: None,
            created_at: 1_700_000_000,
            last_validated_at: 1_700_000_001,
        };
        let json = serde_json::to_string(&ser).expect("serialize");
        let back: SerializedAccount = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.user_id, "111");
        assert_eq!(back.username, "tester");
        assert_eq!(back.token, "tok-123");
        assert_eq!(back.mfa_backup_hashes, vec!["ABCD-1234".to_string()]);
        assert_eq!(back.created_at, 1_700_000_000);
    }

    #[test]
    fn default_stored_account_has_empty_token() {
        let acc = StoredAccount::default();
        assert_eq!(acc.user_id, "");
        assert_eq!(acc.token.expose_secret(), "");
    }

    // Real keyring test'i sadece Windows native store kuruluyken çalışır.
    #[test]
    #[cfg(target_os = "windows")]
    fn keyring_round_trip_real_dpapi() {
        // İlk olarak install() — bu test'te zaten kurulmuş olabilir.
        let _ = AuthStorage::install();

        let storage = AuthStorage::new();
        let user_id = format!("test_{}", now_unix());
        let account = StoredAccount {
            user_id: user_id.clone(),
            username: "round-trip-tester".to_string(),
            token: SecretString::new("real-dpapi-token-12345".to_string().into_boxed_str()),
            mfa_backup_hashes: vec![],
            super_properties: None,
            created_at: now_unix(),
            last_validated_at: now_unix(),
        };

        storage.store_account(&account).expect("store");
        let loaded = storage
            .load_account(&user_id)
            .expect("load")
            .expect("account should exist");
        assert_eq!(loaded.user_id, user_id);
        assert_eq!(loaded.token.expose_secret(), "real-dpapi-token-12345");

        // Cleanup
        storage.delete_account(&user_id).expect("delete");
        let after = storage.load_account(&user_id).expect("load after delete");
        assert!(after.is_none());
    }
}
