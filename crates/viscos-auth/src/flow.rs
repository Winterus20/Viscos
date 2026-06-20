//! Auth akış yöneticisi — C3 (token paste + MFA TOTP).
//!
//! **Tasarım (ADR-0011):**
//!
//! - [`AuthFlow::TokenPaste`]: token yapıştırma akışı — format validate →
//!   keyring store → [`AuthSession`] döner. Gerçek Discord doğrulaması
//!   (`GET /users/@me`) çağıran katmanda yapılır (REST crate, ADR-0008).
//! - [`AuthFlow::MfaChallenge`]: TOTP kodu üret → çağıran katman Discord
//!   `POST /api/v10/auth/mfa/totp`'a iletir (documented, ToS-safe endpoint).
//!
//! **Neden `/auth/login` YOK:**
//! Discord'un `POST /auth/login` endpoint'i undocumented, Cloudflare + hCaptcha
//! korumalı ve ToS ihlali riski yüksek. Twilight bilinçli olarak bu endpoint'i
//! sağlamaz. Faz 2.0 tasarımı bu riski tamamen elimine etmek için C3 (token
//! paste fallback) yaklaşımını benimsiyor.
//!
//! **`/auth/mfa/totp` ToS-safe:** Discord'un documented endpoint'i — kullanıcı
//! tokenı bu endpoint'i kullanabilir (Faz 3.0 gateway auth handshake'te kullanılır).

use secrecy::SecretString;
use tracing::info;

use crate::mfa::{generate_totp, is_valid_totp_format};
use crate::storage::{AuthError, AuthStorage};
use crate::token::{DiscordToken, TokenStore, WindowsCredentialStore};

// ---------------------------------------------------------------------------
// AuthFlow
// ---------------------------------------------------------------------------

/// Auth akış varyantları (C3 — token paste fallback + MFA TOTP).
///
/// `#[non_exhaustive]` — Faz 2.1'de `QrScan` varyantı eklenebilir.
#[non_exhaustive]
pub enum AuthFlow {
    /// Token yapıştırma — kullanıcı Discord'dan kopyaladığı token'ı verir.
    ///
    /// Ana happy path: captcha yönlendirmesi sonrası kullanıcı token'ı kopyalayıp
    /// yapıştırır. Format validate → keyring store → session döner.
    TokenPaste {
        /// Ham Discord auth token'ı.
        token: DiscordToken,
    },
    /// MFA TOTP challenge.
    ///
    /// Kullanıcının authenticator uygulamasındaki base32 kodlu TOTP secret
    /// üzerinden 6 haneli kod üretilir. Çağıran katman bu kodu
    /// `POST /api/v10/auth/mfa/totp`'a iletir.
    MfaChallenge {
        /// Base32 kodlu TOTP secret (kullanıcının authenticator'ından).
        totp_secret: String,
    },
}

// ---------------------------------------------------------------------------
// AuthSession
// ---------------------------------------------------------------------------

/// Başarılı auth akışının çıktısı.
///
/// `user_id` TokenPaste akışında başlangıçta boş — `GET /users/@me` sonrası
/// çağıran katman doldurur. MFA akışında `mfa_code` set, `token` dummy.
///
/// **`Debug` manuel impl:** `token` alanı `DiscordToken` içerdiği için otomatik
/// `Debug` derive yapılmaz; token sızıntısını önlemek için `[REDACTED]` gösterilir.
pub struct AuthSession {
    /// Discord user snowflake (TokenPaste: `GET /users/@me` sonrası doldurulur).
    pub user_id: String,
    /// Discord auth token (TokenPaste: dolu; MfaChallenge: dummy/boş).
    pub token: DiscordToken,
    /// Üretilen TOTP kodu (MfaChallenge: 6 haneli digit; TokenPaste: `None`).
    pub mfa_code: Option<String>,
}

impl std::fmt::Debug for AuthSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthSession")
            .field("user_id", &self.user_id)
            .field("token", &"[REDACTED]")
            .field("mfa_code", &self.mfa_code)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// AuthManager
// ---------------------------------------------------------------------------

/// Auth akışı yöneticisi.
///
/// Token paste ve MFA TOTP akışlarını orchestrate eder. Ağ çağrıları
/// ve gateway koordinasyonu çağıran katmanda (`viscos-shell`) yapılır.
///
/// # Examples
///
/// ```rust,no_run
/// use viscos_auth::flow::{AuthFlow, AuthManager};
/// use viscos_auth::token::DiscordToken;
///
/// let mgr = AuthManager::new();
/// let token = DiscordToken::new("viscos_example_one.two.three_part_dummy_token_placeholder_aa");
/// let session = mgr.authenticate(AuthFlow::TokenPaste { token })?;
/// # Ok::<_, viscos_auth::storage::AuthError>(())
/// ```
pub struct AuthManager {
    _storage: AuthStorage,
    token_store: Box<dyn TokenStore>,
}

impl AuthManager {
    /// Yeni `AuthManager` — production default: `WindowsCredentialStore`.
    ///
    /// `AuthStorage::install()` `main` başlangıcında çağrılmış olmalı.
    pub fn new() -> Self {
        Self {
            _storage: AuthStorage::new(),
            token_store: Box::new(WindowsCredentialStore::new()),
        }
    }

    /// Custom `TokenStore` ile `AuthManager` oluştur (test mock'ları için).
    pub fn with_store(store: impl TokenStore + 'static) -> Self {
        Self {
            _storage: AuthStorage::new(),
            token_store: Box::new(store),
        }
    }

    /// Auth akışını yürüt.
    ///
    /// # Errors
    ///
    /// - [`AuthError::InvalidTokenFormat`] — token format check başarısız
    /// - [`AuthError::Mfa`] — TOTP üretimi başarısız (geçersiz secret)
    /// - [`AuthError::Keyring`] — keyring yazma hatası (TokenPaste)
    pub fn authenticate(&self, flow: AuthFlow) -> Result<AuthSession, AuthError> {
        match flow {
            AuthFlow::TokenPaste { token } => self.handle_token_paste(token),
            AuthFlow::MfaChallenge { totp_secret } => self.handle_mfa_challenge(totp_secret),
        }
    }

    /// Oturumu kapat — token'ı keyring'den sil.
    ///
    /// # Errors
    ///
    /// [`AuthError::Keyring`] keyring silme hatalarında döner.
    pub fn logout(&self, user_id: &str) -> Result<(), AuthError> {
        self.token_store.clear(user_id)?;
        info!(user_id, "auth logout: token cleared from keyring");
        Ok(())
    }

    fn handle_token_paste(&self, token: DiscordToken) -> Result<AuthSession, AuthError> {
        if !token.validate_format() {
            return Err(AuthError::InvalidTokenFormat);
        }

        // Geçici user_id — çağıran katman GET /users/@me sonrası günceller.
        // "pending" sentinel değeri keyring'de staging entry olarak kullanılır.
        let placeholder_user_id = "pending";
        self.token_store.persist(placeholder_user_id, &token)?;
        info!("token paste: token stored (pending GET /users/@me validation)");

        Ok(AuthSession {
            user_id: String::new(),
            token,
            mfa_code: None,
        })
    }

    fn handle_mfa_challenge(&self, totp_secret: String) -> Result<AuthSession, AuthError> {
        let secret = SecretString::new(totp_secret.into_boxed_str());
        let code = generate_totp(&secret).map_err(|e| AuthError::Mfa(e.to_string()))?;

        if !is_valid_totp_format(&code) {
            return Err(AuthError::Mfa(format!(
                "generated TOTP has invalid format: {code}"
            )));
        }

        info!("mfa challenge: TOTP code generated (caller submits to POST /auth/mfa/totp)");

        Ok(AuthSession {
            user_id: String::new(),
            token: DiscordToken::new("".to_string().into_boxed_str()),
            mfa_code: Some(code),
        })
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    /// Bellek-içi `TokenStore` mock — keyring kurulumu gerektirmez.
    struct MockTokenStore {
        stored: Mutex<HashMap<String, String>>,
    }

    impl MockTokenStore {
        fn new() -> Self {
            Self {
                stored: Mutex::new(HashMap::new()),
            }
        }
    }

    impl TokenStore for MockTokenStore {
        fn persist(&self, user_id: &str, token: &DiscordToken) -> Result<(), AuthError> {
            self.stored
                .lock()
                .expect("mutex not poisoned")
                .insert(user_id.to_string(), token.expose().to_string());
            Ok(())
        }

        fn load(&self, user_id: &str) -> Result<Option<DiscordToken>, AuthError> {
            let map = self.stored.lock().expect("mutex not poisoned");
            Ok(map
                .get(user_id)
                .map(|raw| DiscordToken::new(raw.clone().into_boxed_str())))
        }

        fn clear(&self, user_id: &str) -> Result<(), AuthError> {
            self.stored
                .lock()
                .expect("mutex not poisoned")
                .remove(user_id);
            Ok(())
        }
    }

    /// Birim test için kukla token vektörü — Discord token regex'ini tetiklemez.
    /// Lowercase + underscore ile açıkça sahte; format check'i geçer (3 bölüm, 50-100 char).
    fn dummy_token() -> DiscordToken {
        DiscordToken::new(
            [
                "viscos_unit_test_placeholder_aaa",
                "xxxxxx",
                "zzzzzzzzzzzzzz_format_check_only",
            ]
            .join(".")
            .into_boxed_str(),
        )
    }

    #[test]
    fn token_paste_valid_succeeds() {
        let mgr = AuthManager::with_store(MockTokenStore::new());
        let session = mgr
            .authenticate(AuthFlow::TokenPaste {
                token: dummy_token(),
            })
            .expect("valid token paste should succeed");
        assert!(
            session.mfa_code.is_none(),
            "token paste must not produce mfa_code"
        );
        assert_eq!(session.user_id, "", "user_id pending GET /users/@me");
    }

    #[test]
    fn token_paste_invalid_format_returns_invalid_token_format() {
        let mgr = AuthManager::with_store(MockTokenStore::new());
        let token = DiscordToken::new("invalid-no-dots".to_string().into_boxed_str());
        let result = mgr.authenticate(AuthFlow::TokenPaste { token });
        assert!(
            matches!(result, Err(AuthError::InvalidTokenFormat)),
            "expected InvalidTokenFormat, got: {result:?}"
        );
    }

    #[test]
    fn token_paste_stores_to_keyring() {
        let mgr = AuthManager::with_store(MockTokenStore::new());
        // Verifies persist is called without error.
        mgr.authenticate(AuthFlow::TokenPaste {
            token: dummy_token(),
        })
        .expect("should store without error");
    }

    #[test]
    fn mfa_challenge_produces_6_digit_code() {
        let mgr = AuthManager::with_store(MockTokenStore::new());
        // RFC 6238 Appendix B test vector (SHA1 secret)
        let totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ".to_string();
        let session = mgr
            .authenticate(AuthFlow::MfaChallenge { totp_secret })
            .expect("mfa challenge should succeed with valid TOTP secret");
        let code = session
            .mfa_code
            .expect("mfa_code must be set for MfaChallenge");
        assert_eq!(
            code.len(),
            6,
            "TOTP code must be exactly 6 chars, got: '{code}'"
        );
        assert!(
            code.chars().all(|c| c.is_ascii_digit()),
            "TOTP code must be all digits, got: '{code}'"
        );
    }

    #[test]
    fn mfa_challenge_invalid_secret_returns_error() {
        let mgr = AuthManager::with_store(MockTokenStore::new());
        let result = mgr.authenticate(AuthFlow::MfaChallenge {
            totp_secret: "NOT-VALID-BASE32-!!!".to_string(),
        });
        assert!(result.is_err(), "invalid TOTP secret must produce error");
        assert!(
            matches!(result, Err(AuthError::Mfa(_))),
            "error must be AuthError::Mfa, got: {result:?}"
        );
    }

    #[test]
    fn logout_clears_pending_token() {
        let mgr = AuthManager::with_store(MockTokenStore::new());
        mgr.authenticate(AuthFlow::TokenPaste {
            token: dummy_token(),
        })
        .expect("paste should succeed");
        mgr.logout("pending").expect("logout should succeed");
    }

    #[test]
    fn auth_manager_default_constructs_without_panic() {
        // Smoke test: Default::default() dönmeli (keyring init olmadan da).
        let _mgr = AuthManager::default();
    }
}
