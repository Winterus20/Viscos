//! Discord token newtype + `TokenStore` abstraction.
//!
//! **Tasarım notu:** `DiscordToken` bir `SecretString` wrapper'dır. `SecretString`
//! içindeki `Box<str>` drop anında otomatik `zeroize` edilir — struct düzeyinde
//! ayrıca `ZeroizeOnDrop` derive gerekmez.
//!
//! **Token format:** Discord token'ları üç nokta (`.`) ile ayrılmış bölümden
//! oluşur: `base64(user_id).<timestamp>.<hmac>`. v1 validasyon yalnızca format
//! sanity check yapar — gerçek geçerlilik `GET /users/@me` ile çağıran katmanda
//! doğrulanır (ADR-0011 §2).
//!
//! **`TokenStore` trait:** Platform-agnostic interface — test mock'ları ve prod
//! impl'leri aynı interface'e uyar. `WindowsCredentialStore` prod impl'i
//! `keyring-core` + DPAPI arkasında saklar.

use secrecy::{ExposeSecret, SecretString};

use crate::storage::{AuthError, AuthStorage, StoredAccount, now_unix};

// ---------------------------------------------------------------------------
// DiscordToken
// ---------------------------------------------------------------------------

/// Discord auth token'ı güvenli bellekte tutan newtype.
///
/// İç `SecretString` drop anında `Box<str>`'i zeroize eder.
///
/// # Examples
///
/// ```rust
/// use viscos_auth::token::DiscordToken;
///
/// let token = DiscordToken::new("NTkwMjE4.abc.defghijklmnopqrstuvwxyz0123456789abcdefghij");
/// assert!(token.validate_format());
/// ```
pub struct DiscordToken(SecretString);

impl DiscordToken {
    /// Ham string'ten token oluştur.
    ///
    /// `into()` ile `Box<str>` dönüşümü — heap allocation tokenın ömrüyle sınırlı.
    pub fn new(raw: impl Into<Box<str>>) -> Self {
        Self(SecretString::new(raw.into()))
    }

    /// Token değerini düz metin olarak expose et.
    ///
    /// **Audit notu:** Her `expose` çağrısı kod incelemesinde gözden geçirilmeli.
    /// Token log'lanmamalı, yalnızca ağ/keyring çağrısında kullanılmalı.
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    /// Discord token format sanity check (ağ çağrısı yok).
    ///
    /// Discord formatı: `<base64(user_id)>.<timestamp>.<hmac>` — iki nokta,
    /// 50–100 karakter arası ASCII alfanümerik + `.`, `-`, `_`.
    ///
    /// Dönen `false` değerinde çağıran `AuthError::InvalidTokenFormat` döndürmeli.
    pub fn validate_format(&self) -> bool {
        let t = self.0.expose_secret();
        if t.is_empty() {
            return false;
        }
        let dot_count = t.chars().filter(|&c| c == '.').count();
        if dot_count != 2 {
            return false;
        }
        let valid_chars = t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        let len_ok = (50..=100).contains(&t.len());
        valid_chars && len_ok
    }
}

// ---------------------------------------------------------------------------
// TokenStore trait
// ---------------------------------------------------------------------------

/// Platform-agnostic token persistence interface.
///
/// # Errors
///
/// Implementasyonlar `AuthError` döndürür. Temel varyantlar:
/// - [`AuthError::InvalidTokenFormat`] — format check başarısız
/// - [`AuthError::Keyring`] — keyring I/O hatası
/// - [`AuthError::AccountNotFound`] — yükleme sırasında kayıt yok
pub trait TokenStore: Send + Sync {
    /// Token'ı kalıcı depoya yaz (varsa üzerine yazar).
    ///
    /// # Errors
    ///
    /// Keyring I/O veya serializasyon hatalarında `AuthError` döner.
    fn persist(&self, user_id: &str, token: &DiscordToken) -> Result<(), AuthError>;

    /// Token'ı kalıcı depodan oku.
    ///
    /// Kayıt yoksa `Ok(None)` döner (hata değil).
    ///
    /// # Errors
    ///
    /// Keyring okuma hatalarında `AuthError` döner.
    fn load(&self, user_id: &str) -> Result<Option<DiscordToken>, AuthError>;

    /// Token'ı kalıcı depodan sil (idempotent — yoksa no-op).
    ///
    /// # Errors
    ///
    /// Keyring silme hatalarında `AuthError` döner.
    fn clear(&self, user_id: &str) -> Result<(), AuthError>;
}

// ---------------------------------------------------------------------------
// WindowsCredentialStore
// ---------------------------------------------------------------------------

/// Windows Credential Manager (DPAPI) tabanlı `TokenStore` implementasyonu.
///
/// `AuthStorage` üzerinden `keyring-core` delegasyonu. DPAPI current-user scope'unda
/// AES-256 şifreler. `AuthStorage::install()` önceden çağrılmış olmalı.
///
/// # Errors
///
/// `AuthStorage::install()` çağrılmadan kullanılırsa `AuthError::StoreNotInitialized`.
pub struct WindowsCredentialStore {
    storage: AuthStorage,
}

impl WindowsCredentialStore {
    /// Yeni store handle oluştur.
    ///
    /// # Errors
    ///
    /// `AuthStorage::install()` önceden çağrılmamışsa ilk `persist`/`load`/`clear`
    /// çağrısında `AuthError::StoreNotInitialized` döner.
    pub fn new() -> Self {
        Self {
            storage: AuthStorage::new(),
        }
    }
}

impl Default for WindowsCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStore for WindowsCredentialStore {
    fn persist(&self, user_id: &str, token: &DiscordToken) -> Result<(), AuthError> {
        let account = StoredAccount {
            user_id: user_id.to_string(),
            username: String::new(),
            token: SecretString::new(token.expose().to_string().into_boxed_str()),
            mfa_backup_hashes: Vec::new(),
            super_properties: None,
            created_at: now_unix(),
            last_validated_at: now_unix(),
        };
        self.storage.store_account(&account)
    }

    fn load(&self, user_id: &str) -> Result<Option<DiscordToken>, AuthError> {
        match self.storage.load_account(user_id)? {
            Some(acc) => {
                let raw = acc.token.expose_secret().to_string();
                Ok(Some(DiscordToken::new(raw.into_boxed_str())))
            }
            None => Ok(None),
        }
    }

    fn clear(&self, user_id: &str) -> Result<(), AuthError> {
        self.storage.delete_account(user_id)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Dummy test token — 3 bölüm, geçerli karakterler, 50-100 char.
    /// Bilinçli olarak kısa/küçük harf ile Discord token regex'ini tetiklemez.
    fn dummy_valid_token() -> String {
        // Segments are clearly non-realistic (lowercase, short middle) to
        // avoid GitHub secret-scanning false positives.
        [
            "viscos_unit_test_placeholder_aaa",
            "xxxxxx",
            "zzzzzzzzzzzzzz_format_check_only",
        ]
        .join(".")
    }

    #[test]
    fn valid_token_passes_format_check() {
        let raw = dummy_valid_token();
        assert!(
            (50..=100).contains(&raw.len()),
            "test vector must be 50-100 chars, got {}",
            raw.len()
        );
        let tok = DiscordToken::new(raw.into_boxed_str());
        assert!(tok.validate_format(), "valid token should pass format");
    }

    #[test]
    fn empty_token_fails_format() {
        let tok = DiscordToken::new("".to_string().into_boxed_str());
        assert!(!tok.validate_format());
    }

    #[test]
    fn token_without_dots_fails_format() {
        let tok = DiscordToken::new("nodots_at_all_abc123".to_string().into_boxed_str());
        assert!(!tok.validate_format());
    }

    #[test]
    fn token_with_one_dot_fails_format() {
        let tok = DiscordToken::new("one.dot".to_string().into_boxed_str());
        assert!(!tok.validate_format());
    }

    #[test]
    fn token_expose_matches_input() {
        let raw = "aaaa_test_expose.bbbb.cccc";
        let tok = DiscordToken::new(raw.to_string().into_boxed_str());
        assert_eq!(tok.expose(), raw);
    }

    #[test]
    fn token_too_short_fails_format() {
        // Under 50 chars
        let tok = DiscordToken::new("short.a.b".to_string().into_boxed_str());
        assert!(!tok.validate_format());
    }

    #[test]
    fn windows_credential_store_can_be_constructed() {
        let _store = WindowsCredentialStore::new();
        let _default = WindowsCredentialStore::default();
    }
}
