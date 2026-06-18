//! MFA — TOTP + Backup Codes.
//!
//! **Discord TOTP modeli:** Viscos TOTP secret'ı **üretmez**, kullanıcının
//! kendi authenticator'ı üretir. Doğrulama **Discord'un API'si** tarafından
//! yapılır (`POST /auth/mfa/totp`). `totp-rs` burada yalnızca:
//!
//! 1. `Secret::Encoded` parse için kullanılır (kod okuma sırasında base32 → bytes).
//! 2. **Local TOTP secret'i saklanırsa** (opsiyonel) ileride native 2FA helper'ı
//!    için `generate_current` kullanılabilir.
//!
//! **Backup codes:** Discord 8 karakterli alphanumeric üretir. Plaintext
//! olarak keyring entry'sinde tutulur (Argon2 PHC v2.0'da opt-in, ADR-0011).
//!
//! **v1'de:** Argon2 PHC **yok** — kodlar plaintext. ADR-0011 §4.1 planına
//! göre v1'de MFA backup hash alanı reserve edildi ama hash fonksiyonu
//! inert (no-op stub).

use std::time::SystemTimeError;

use rand::seq::SliceRandom;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use totp_rs::{Algorithm, Secret as TotpSecret, TOTP};
use zeroize::Zeroize;

use crate::storage::AuthError;

/// MFA yöntemleri. Discord 2024'ten beri SMS'i kaldırdı; v1'de sadece TOTP
/// ve backup code aktif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MfaMethod {
    /// Time-based One-Time Password (RFC 6238). Kullanıcının authenticator'ı.
    Totp,
    /// 8-karakterli alphanumeric backup code (Discord resmi davranışı).
    BackupCode,
    /// SMS — Discord artık desteklemiyor. v1'de enum'da duruyor, implementasyon yok.
    Sms,
}

/// MFA-specific hatalar. `AuthError::Mfa` üzerinden propagate eder.
#[derive(Debug, Error)]
pub enum MfaError {
    #[error("invalid TOTP code format (expected 6 digits)")]
    InvalidTotpFormat,
    #[error("invalid backup code format (expected 8 alphanumeric chars)")]
    InvalidBackupFormat,
    #[error("TOTP secret decode error: {0}")]
    TotpSecretDecode(String),
    #[error("TOTP system time error: {0}")]
    TotpSystemTime(String),
    #[error("invalid user_id: {0}")]
    InvalidUserId(String),
}

impl From<MfaError> for AuthError {
    fn from(err: MfaError) -> Self {
        AuthError::Mfa(err.to_string())
    }
}

/// SHA1, 6 digits, step=30s, skew=1 — Discord ve tüm yaygın authenticator'lar
/// için uyumlu default.
///
/// **`otpauth` feature'ı kapalı** (`default-features = false`) → `TOTP::new` 5
/// arg alır. issuer/account_name isteyen 7-arg imza `otpauth` feature'ı
/// arkasında. ADR-0011 binary bütçesi nedeniyle `otpauth` kapalı.
fn build_totp(secret_b32: &str) -> Result<TOTP, MfaError> {
    let secret_bytes = TotpSecret::Encoded(secret_b32.to_string())
        .to_bytes()
        .map_err(|e| MfaError::TotpSecretDecode(e.to_string()))?;
    TOTP::new(Algorithm::SHA1, 6, 1, 30, secret_bytes)
        .map_err(|e| MfaError::TotpSecretDecode(e.to_string()))
}

/// Verilen TOTP secret'tan **şu anki 30s'lik dilimin kodunu** üret.
///
/// **Tasarım notu:** Viscos tarafında TOTP secret üretmiyoruz (kullanıcı
/// kendi authenticator'ıyla kurulum yapıyor). Bu fonksiyon yalnızca **opt-in
/// olarak** keyring'de saklanan TOTP secret'i doğrulamak için kullanılabilir.
/// Şu anda Discord tüm doğrulamayı kendi API'sinde yaptığı için bu fonksiyon
/// gelecekteki `local 2FA` senaryoları için altyapı.
pub fn generate_totp(secret_b32: &SecretString) -> Result<String, MfaError> {
    let exposed = secret_b32.expose_secret();
    let totp = build_totp(exposed)?;
    totp.generate_current()
        .map_err(|e| MfaError::TotpSecretDecode(e.to_string()))
}

/// Verilen TOTP secret ve 6-hanelik kodu karşılaştır.
///
/// **Uyarı:** `totp-rs` `check_current` skew=1 kullanır — yani önceki ve
/// sonraki 30s dilimlerini de kabul eder (network gecikmesi için). Discord
/// API'sine gönderimde **bunu kullanmıyoruz** (Discord kendi doğrular), bu
/// helper yalnızca local-side format validation için uygundur.
pub fn verify_totp(secret_b32: &SecretString, code: &str) -> Result<bool, MfaError> {
    if !is_valid_totp_format(code) {
        return Err(MfaError::InvalidTotpFormat);
    }
    let totp = build_totp(secret_b32.expose_secret())?;
    totp.check_current(code)
        .map_err(|e: SystemTimeError| MfaError::TotpSystemTime(e.to_string()))
}

/// TOTP kod format validasyonu — 6 ASCII digit.
pub fn is_valid_totp_format(code: &str) -> bool {
    code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
}

/// Backup code format validasyonu — Discord 8-karakterli alphanumeric
/// (büyük harf + rakam) **veya** `xxxx-xxxx` 9-karakterli tire-ayrılmış.
///
/// **Discord gerçek format:** Kullanıcıya gösterimde tire ile ayrılır
/// (`ABCD-1234`); doğrulama sırasında tire opsiyonel kabul edilir.
pub fn is_valid_backup_code_format(code: &str) -> bool {
    if code.len() == 8 {
        code.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    } else if code.len() == 9 {
        // 4 + tire + 4 formatı.
        let bytes = code.as_bytes();
        if bytes[4] != b'-' {
            return false;
        }
        bytes[..4]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            && bytes[5..]
                .iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    } else {
        false
    }
}

/// **Discord davranışı:** MFA kurulumunda kullanıcıya 10 adet 8-karakterli
/// alphanumeric backup code verir. `xxxx-xxxx` formatında tire ile ikiye
/// bölünmüş 8 karakter.
pub fn generate_backup_codes() -> Vec<String> {
    // ASCII büyük harf + rakam (Discord davranışı).
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    let mut rng = rand::thread_rng();
    (0..10)
        .map(|_| {
            let chars: Vec<u8> = ALPHABET.choose_multiple(&mut rng, 8).copied().collect();
            let raw = std::str::from_utf8(&chars).expect("ASCII subset");
            // Discord formatı: `xxxx-xxxx`
            format!("{}-{}", &raw[..4], &raw[4..])
        })
        .collect()
}

/// Backup code hash'le. **v1'de plaintext saklanır** (Argon2 PHC v2.0'da opt-in).
///
/// Fonksiyon imzası hazır: v2.0'da bu implementasyon Argon2id PHC string
/// üretecek, çağıran taraf değişmeyecek. Şu anda **inert** — sadece input'u
/// döndürür (encode yok, hash yok).
///
/// **ÖNEMLİ:** Fonksiyon imzası `hash_backup_code` ve `verify_backup_code`
/// (storage.rs'de) public API'de. v2.0 PR'ında sadece bu implementasyon
/// değişecek, çağıran taraf dokunmayacak.
pub fn hash_backup_code(code: &str) -> String {
    // v1 inert: plaintext. v2.0'da Argon2id PHC.
    let mut owned = code.to_string();
    owned.zeroize();
    code.to_string()
}

/// Backup code doğrula — v1'de sabit karşılaştırma, v2.0'da Argon2 verify.
///
/// **v1'de** bu fonksiyon **güvenli değildir**: timing attack'a açık.
/// Kullanıcı self-bot yapmıyorsa saldırı vektörü zayıf. v2.0'da
/// Argon2 verify ile constant-time olacak.
pub fn verify_backup_code(code: &str, hash: &str) -> bool {
    constant_time_eq(code.as_bytes(), hash.as_bytes())
}

/// Constant-time karşılaştırma — timing side-channel'ı azaltır.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 Appendix B test vector — SHA1 secret
    // secret = "12345678901234567890" (ASCII bytes)
    // T = 59 → 94287082
    const RFC6238_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn totp_format_validation() {
        assert!(is_valid_totp_format("123456"));
        assert!(is_valid_totp_format("000000"));
        assert!(!is_valid_totp_format("12345")); // 5 digit
        assert!(!is_valid_totp_format("1234567")); // 7 digit
        assert!(!is_valid_totp_format("abcdef")); // non-digit
    }

    #[test]
    fn backup_code_format_validation() {
        assert!(is_valid_backup_code_format("ABCD-1234"));
        assert!(is_valid_backup_code_format("12345678"));
        assert!(!is_valid_backup_code_format("ABCD-123")); // 7
        assert!(!is_valid_backup_code_format("abcd-1234")); // lowercase
        assert!(!is_valid_backup_code_format("ABCD12345")); // no dash, 9
    }

    #[test]
    fn backup_code_generation_produces_10() {
        let codes = generate_backup_codes();
        assert_eq!(codes.len(), 10);
        for c in &codes {
            assert!(is_valid_backup_code_format(c), "invalid code: {c}");
        }
        // Tüm kodlar benzersiz olmalı.
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), 10, "duplicate codes generated");
    }

    #[test]
    fn hash_backup_code_v1_is_identity() {
        // v1 inert: input == output (plaintext storage).
        let hashed = hash_backup_code("ABCD-1234");
        assert_eq!(hashed, "ABCD-1234");
    }

    #[test]
    fn verify_backup_code_roundtrip() {
        let code = "TEST-CODE";
        let hash = hash_backup_code(code);
        assert!(verify_backup_code(code, &hash));
        assert!(!verify_backup_code("OTHER-CODE", &hash));
    }

    #[test]
    fn totp_secret_can_be_built() {
        // RFC 6238 test vector secret ile TOTP kurulabiliyor mu kontrol.
        let secret = SecretString::new(RFC6238_SECRET.to_string().into_boxed_str());
        // Üretim hata vermemeli — gerçek üretilen kod zaman-bağımlı,
        // burada sadece "build_totp başarılı" olduğunu doğruluyoruz.
        let _totp = build_totp(secret.expose_secret()).expect("build totp");
    }

    #[test]
    fn generate_totp_returns_6_digits() {
        let secret = SecretString::new(RFC6238_SECRET.to_string().into_boxed_str());
        let code = generate_totp(&secret).expect("generate");
        assert!(
            is_valid_totp_format(&code),
            "generated code is not 6 digits: {code}"
        );
    }

    #[test]
    fn invalid_totp_secret_returns_error() {
        let secret = SecretString::new("NOT-A-VALID-BASE32-!!!".to_string().into_boxed_str());
        let result = generate_totp(&secret);
        assert!(matches!(
            result,
            Err(MfaError::TotpSecretDecode(_)) | Err(MfaError::InvalidTotpFormat)
        ));
    }

    #[test]
    fn verify_totp_rejects_wrong_format() {
        let secret = SecretString::new(RFC6238_SECRET.to_string().into_boxed_str());
        let result = verify_totp(&secret, "12345"); // 5 digit
        assert!(matches!(result, Err(MfaError::InvalidTotpFormat)));
    }
}
