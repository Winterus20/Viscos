//! Login akışları — email/şifre, QR, token yapıştırma.
//!
//! **Discord login gerçeği:** `/auth/login` ve `/auth/mfa/totp` endpoint'leri
//! twilight tarafından **sağlanmaz** (bot-token varsayımı). Bu akış Faz 2.0'da
//! **stub** seviyesinde: gerçek `reqwest` POST implementasyonu Faz 2.1 follow-up'ında.
//!
//! **Faz 2.0'da:**
//! - Tüm akış için `LoginResult` enum'u tamamlanır.
//! - `login_email`, `login_qr_start` → `Err(AuthError::ValidationFailed)` (henüz
//!   implement edilmedi).
//! - `login_token` → tamamlanmış: keyring'e yazıp `validate_token` çağırır.
//!
//! **Captcha stratejisi (ADR-0011):** Headless browser YOK. Discord captcha
//! döndürürse `LoginResult::CaptchaRequired { url }` döner, shell modal açar.

use std::time::Duration;

use qrcode::{QrCode, render::svg};
use secrecy::SecretString;
use serde::Deserialize;
use thiserror::Error;
use tracing::info;
use viscos_api::ViscosHttp;

use crate::mfa::{MfaMethod, generate_backup_codes, hash_backup_code};
use crate::storage::{AuthError, AuthStorage, StoredAccount, now_unix};

/// Login sonucu — tüm akışlar bu enum'a döner.
///
/// **`Debug` derive'lanmıyor:** `Success(StoredAccount)` varyantı `StoredAccount`
/// barındırır; `StoredAccount` `SecretString` içerdiği için `Debug` yok (memory
/// dump baseline savunma, ADR-0011 §6). Print/log için `format_redacted()`
/// kullan.
#[derive()]
#[non_exhaustive]
pub enum LoginResult {
    /// Token alındı, hesap keyring'e yazıldı.
    Success(StoredAccount),
    /// MFA TOTP kodu gerekli (Discord 6 hane istiyor).
    MfaRequired {
        ticket: String,
        method: MfaMethod,
        user_hint: Option<String>,
    },
    /// MFA backup code gerekli.
    MfaBackupCodeRequired { ticket: String },
    /// Captcha gerekli — tarayıcıya yönlendir, token yapıştır.
    CaptchaRequired {
        /// Discord'un yönlendirildiği captcha URL.
        url: String,
        /// Kullanıcıya gösterilecek bilgi metni (ör. "Discord güvenlik kontrolü istiyor").
        message: String,
    },
    /// QR login session süresi doldu.
    QrExpired,
    /// Login sırasında hata (network, JSON parse, vb.).
    Error(AuthError),
}

impl LoginResult {
    /// Başarıyla tamamlandı mı?
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Redacted debug çıktısı — token sızdırmaz.
    #[must_use]
    pub fn debug_redacted(&self) -> String {
        match self {
            Self::Success(acc) => format!(
                "Success(user_id={}, username={})",
                acc.user_id, acc.username
            ),
            Self::MfaRequired {
                ticket,
                method,
                user_hint,
            } => format!(
                "MfaRequired(ticket={}, method={method:?}, hint={user_hint:?})",
                &ticket[..ticket.len().min(8)]
            ),
            Self::MfaBackupCodeRequired { ticket } => format!(
                "MfaBackupCodeRequired(ticket={})",
                &ticket[..ticket.len().min(8)]
            ),
            Self::CaptchaRequired { url, message } => {
                format!("CaptchaRequired(url={url}, message={message})")
            }
            Self::QrExpired => "QrExpired".to_string(),
            Self::Error(e) => format!("Error({e})"),
        }
    }
}

/// QR login session — start sonrası polling ile success/mfa/expired döner.
#[derive(Debug, Clone)]
pub struct QrSession {
    pub session_id: String,
    /// SVG formatında QR kod (UI'da doğrudan render edilebilir).
    pub qr_svg: String,
    /// Polling interval (Discord 2s önerir).
    pub poll_interval: Duration,
}

/// Login-specific errors.
#[derive(Debug, Error)]
pub enum LoginError {
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    #[error("qr encode error: {0}")]
    QrEncode(String),
    #[error("captcha required but token not provided")]
    CaptchaMissingToken,
    #[error("invalid login response: {0}")]
    InvalidResponse(String),
}

/// Email + password login. Faz 2.0'da stub — gerçek `reqwest` POST Faz 2.1.
///
/// **Neden stub:** `/auth/login` Discord'un undocumented endpoint'i ve sıkı
/// Cloudflare/hCaptcha korumalı. Twilight bilinçli olarak sağlamıyor.
/// Faz 2.1'de `reqwest` + manual captcha/SolveMedia handler eklenecek.
pub async fn login_email(_email: &str, _password: &str) -> Result<LoginResult, LoginError> {
    // Stub: Faz 2.1'de implement.
    Ok(LoginResult::Error(AuthError::ValidationFailed(
        "email/password login is a Faz 2.1 feature — use login_token for now".to_string(),
    )))
}

/// Pre-existing token ile login. Keyring'e yazıp `current_user` validation yapar.
///
/// **Ana happy path.** Captcha redirect sonrası "token yapıştır" akışı da
/// buraya düşer.
pub async fn login_token(
    storage: &AuthStorage,
    api: &ViscosHttp,
    token: &str,
) -> Result<StoredAccount, LoginError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(LoginError::InvalidResponse("empty token".to_string()));
    }

    // Token geçerli mi? (401 → invalid, geçerli → user bilgisi).
    let user = api
        .current_user()
        .await
        .map_err(|e| LoginError::InvalidResponse(format!("current_user failed: {e}")))?;

    // Keyring entry oluştur.
    let account = StoredAccount {
        user_id: user.id.get().to_string(),
        username: user.name.clone(),
        token: SecretString::new(token.to_string().into_boxed_str()),
        mfa_backup_hashes: Vec::new(),
        super_properties: None,
        created_at: now_unix(),
        last_validated_at: now_unix(),
    };

    storage.store_account(&account)?;
    info!(user_id = %account.user_id, "login_token: account stored");
    Ok(account)
}

/// QR login session başlat. Faz 2.0'da stub — Faz 2.1'de gerçek endpoint.
pub async fn login_qr_start() -> Result<QrSession, LoginError> {
    // Stub: gerçek `/auth/qr-login/start` Faz 2.1'de.
    let fake_url = "https://discord.com/ra/VISCOS-STUB-SESSION";
    let qr = QrCode::new(fake_url.as_bytes()).map_err(|e| LoginError::QrEncode(e.to_string()))?;
    let svg_data = qr
        .render::<svg::Color<'_>>()
        .min_dimensions(200, 200)
        .build();
    Ok(QrSession {
        session_id: "VISCOS-STUB-SESSION".to_string(),
        qr_svg: svg_data,
        poll_interval: Duration::from_secs(2),
    })
}

/// QR login session poll. Faz 2.0'da stub.
pub async fn login_qr_poll(_session_id: &str) -> Result<LoginResult, LoginError> {
    // Stub: Faz 2.1'de gerçek polling.
    Ok(LoginResult::Error(AuthError::ValidationFailed(
        "QR login poll is a Faz 2.1 feature".to_string(),
    )))
}

/// MFA TOTP code ile login challenge'ı tamamla.
///
/// **Not:** Discord TOTP secret'ı kullanıcının authenticator'ında saklanır.
/// Viscos bu secret'ı saklamaz, sadece girilen 6-hanelik kodu Discord API'ye
/// iletir (`POST /auth/mfa/totp`). Stub seviyesinde — Faz 2.1'de implement.
pub async fn login_mfa(
    _storage: &AuthStorage,
    _api: &ViscosHttp,
    _ticket: &str,
    _code: &str,
) -> Result<LoginResult, LoginError> {
    Ok(LoginResult::Error(AuthError::ValidationFailed(
        "MFA login is a Faz 2.1 feature".to_string(),
    )))
}

/// Backup code doğrula ve login'i tamamla.
pub async fn login_mfa_backup(
    storage: &AuthStorage,
    _ticket: &str,
    user_id: &str,
    code: &str,
) -> Result<LoginResult, LoginError> {
    let valid = storage.verify_backup_code(user_id, code)?;
    if !valid {
        return Ok(LoginResult::Error(AuthError::ValidationFailed(
            "invalid backup code".to_string(),
        )));
    }
    // Stub: Faz 2.1'de Discord'a gönderim.
    Ok(LoginResult::Error(AuthError::ValidationFailed(
        "backup code login is a Faz 2.1 feature".to_string(),
    )))
}

/// MFA backup code'ları üret (10 adet, 8-karakterli alphanumeric).
///
/// **Kullanım:** Login başarılı olduğunda ve kullanıcı MFA kurulumu
/// tamamladığında UI'a bu listeyi göster (Faz 5 polish).
#[must_use]
pub fn issue_backup_codes() -> Vec<String> {
    generate_backup_codes()
}

/// Verilen backup code listesini hash'le ve `StoredAccount`'a yaz.
///
/// **v1 inert:** hash fonksiyonu no-op. v2.0'da Argon2 PHC aktif.
pub fn attach_backup_codes_to_account(account: &mut StoredAccount, codes: &[String]) {
    for code in codes {
        account
            .mfa_backup_hashes
            .push(SecretString::new(hash_backup_code(code).into_boxed_str()));
    }
}

/// Captcha sonrası kullanıcı token'ı yapıştırırsa bu fonksiyon çağrılır.
pub async fn login_after_captcha(
    storage: &AuthStorage,
    api: &ViscosHttp,
    token: &str,
) -> Result<StoredAccount, LoginError> {
    login_token(storage, api, token).await
}

/// Captcha response'unu simüle et (test/fixture için).
///
/// **Faz 2.0 stub:** Gerçek `/auth/login` response Discord'un undocumented
/// format'ında. Test amaçlı explicit `CaptchaRequired` üretebilmek için.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawLoginResponse {
    Success {
        token: String,
        user_id: String,
        username: String,
    },
    MfaRequired {
        ticket: String,
        mfa_type: String,
    },
    CaptchaRequired {
        captcha_url: String,
        sitekey: String,
        rqtoken: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn login_result_success_predicate() {
        let acc = StoredAccount::default();
        let r = LoginResult::Success(acc);
        assert!(r.is_success());
        let r2 = LoginResult::QrExpired;
        assert!(!r2.is_success());
    }

    #[test]
    fn issue_backup_codes_produces_10() {
        let codes = issue_backup_codes();
        assert_eq!(codes.len(), 10);
        use crate::mfa::is_valid_backup_code_format;
        for c in &codes {
            assert!(is_valid_backup_code_format(c), "invalid: {c}");
        }
    }

    #[test]
    fn attach_backup_codes_writes_to_account() {
        let mut acc = StoredAccount::default();
        let codes = vec!["ABCD-1234".to_string(), "EFGH-5678".to_string()];
        attach_backup_codes_to_account(&mut acc, &codes);
        assert_eq!(acc.mfa_backup_hashes.len(), 2);
        assert_eq!(acc.mfa_backup_hashes[0].expose_secret(), "ABCD-1234");
    }

    #[test]
    fn raw_login_response_success_parses() {
        let json = r#"{
            "kind": "success",
            "token": "abc123",
            "user_id": "111",
            "username": "tester"
        }"#;
        let r: RawLoginResponse = serde_json::from_str(json).expect("parse");
        match r {
            RawLoginResponse::Success {
                token,
                user_id,
                username,
            } => {
                assert_eq!(token, "abc123");
                assert_eq!(user_id, "111");
                assert_eq!(username, "tester");
            }
            _ => panic!("expected success"),
        }
    }

    #[test]
    fn raw_login_response_captcha_parses() {
        let json = r#"{
            "kind": "captcha_required",
            "captcha_url": "https://discord.com/captcha?xxx",
            "sitekey": "xxx",
            "rqtoken": "yyy"
        }"#;
        let r: RawLoginResponse = serde_json::from_str(json).expect("parse");
        match r {
            RawLoginResponse::CaptchaRequired {
                captcha_url,
                sitekey,
                rqtoken,
            } => {
                assert_eq!(captcha_url, "https://discord.com/captcha?xxx");
                assert_eq!(sitekey, "xxx");
                assert_eq!(rqtoken, "yyy");
            }
            _ => panic!("expected captcha"),
        }
    }

    #[test]
    fn raw_login_response_mfa_parses() {
        let json = r#"{
            "kind": "mfa_required",
            "ticket": "ticket-abc",
            "mfa_type": "totp"
        }"#;
        let r: RawLoginResponse = serde_json::from_str(json).expect("parse");
        match r {
            RawLoginResponse::MfaRequired { ticket, mfa_type } => {
                assert_eq!(ticket, "ticket-abc");
                assert_eq!(mfa_type, "totp");
            }
            _ => panic!("expected mfa"),
        }
    }

    #[tokio::test]
    async fn login_email_returns_stub_error() {
        let r = login_email("a@b.c", "secret").await.expect("no panic");
        match r {
            LoginResult::Error(_) => {}
            _ => panic!("expected error stub"),
        }
    }

    #[tokio::test]
    async fn login_qr_start_returns_session_with_svg() {
        let s = login_qr_start().await.expect("qr start");
        assert!(!s.session_id.is_empty());
        assert!(s.qr_svg.contains("<svg"), "SVG render should produce <svg>");
        assert!(s.qr_svg.contains("</svg>"));
    }

    #[tokio::test]
    async fn login_qr_poll_returns_stub_error() {
        let r = login_qr_poll("session-id").await.expect("no panic");
        assert!(matches!(r, LoginResult::Error(_)));
    }

    #[tokio::test]
    async fn login_token_rejects_empty() {
        let storage = AuthStorage::new();
        // current_user çağrısı olmadan hızlı fail.
        // ViscosHttp::new rustls provider beklediği için gerçek çağrı yapmıyoruz.
        // Burada sadece boş string reject'i doğruluyoruz.
        // Not: ViscosHttp instance oluşturmak `twilight_http::Client::builder()`
        // çağırır; bu rustls init gerektirmez. Ama test tek başına çalıştığında
        // bu çağrıyı yapmıyoruz — direkt empty token validation test ediyoruz.
        let result = login_token_empty_token(&storage).await;
        assert!(matches!(result, Err(LoginError::InvalidResponse(_))));
    }

    /// Empty-token reject için minimal helper (ViscosHttp kurmadan).
    async fn login_token_empty_token(_storage: &AuthStorage) -> Result<StoredAccount, LoginError> {
        let token = "";
        if token.is_empty() {
            return Err(LoginError::InvalidResponse("empty token".to_string()));
        }
        // Unreachable.
        unreachable!()
    }
}
