//! `viscos-auth` — Discord kimlik doğrulama ve token storage katmanı.
//!
//! **Bileşenler (ADR-0011):**
//!
//! - [`storage`]: `keyring-core 1.0` + `windows-native-keyring-store 1.1`
//!   (DPAPI arkası) ile Discord token saklama. `service = "Viscos"`,
//!   `user = user_id` (Discord snowflake). Multi-account v2 için altyapı hazır.
//! - [`login`]: Email/şifre, QR ve token yapıştırma akışları.
//!   Discord captcha → `LoginResult::CaptchaRequired { url }`.
//! - [`mfa`]: TOTP (`totp-rs 5.7`) + backup codes (plaintext, 8-char alphanumeric).
//! - [`super_properties`]: X-Super-Properties Web client fingerprint üretimi.
//! - [`disclaimer`]: ToS disclaimer canonical metin.
//! - [`shadow_mode`]: Faz 1.5 24h shadow mode stub (writes blocked).
//!
//! **Bellek hijyeni:** Tüm secret material `SecretString`
//! (`secrecy::SecretBox<str>`) + `ZeroizeOnDrop` ile sarılmış.
//! `Secret::new` yerine `SecretString::new` veya `SecretString::from`
//! kullanılır.
//!
//! **Scope guard:** Faz 2.0 dalgası sadece login + REST auth handshake.
//! Gateway (Faz 3), cache (Faz 4), voice (Faz 7) → sonraki dalga.

pub mod disclaimer;
pub mod login;
pub mod mfa;
pub mod shadow_mode;
pub mod storage;
pub mod super_properties;

pub use login::{LoginResult, QrSession};
pub use shadow_mode::ShadowMode;
pub use storage::{AuthError, AuthStorage, StoredAccount};

/// `keyring-core` servis adı — Windows Credential Manager ve diğer native
/// store'lar bu string'i uygulama kimliği olarak gösterir (kullanıcı
/// "Viscos" entry'sini Credential Manager UI'da görebilir).
pub const SERVICE_NAME: &str = "Viscos";

/// Keyring entry yoksa dönen discriminator (Discord tarafında `id: 0` sentinel
/// değerinin aksine, keyring tarafında `Option<None>`).
pub fn user_id_is_valid(user_id: &str) -> bool {
    !user_id.is_empty() && user_id.chars().all(|c| c.is_ascii_digit()) && user_id.len() <= 20
}
