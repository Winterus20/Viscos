//! Login akışları — email/şifre, QR, token yapıştırma.
//!
//! `/auth/login` + `/auth/mfa/{totp,sms,backup}` + `/auth/qr-login/*` twilight
//! tarafından sağlanmaz; Viscos kendi [`Transport`](crate::transport::Transport)
//! abstraction'ı ile manual HTTP yapar. `live-http` Cargo feature'ı kapalıyken
//! HTTP akışları `AuthError::LiveHttpDisabled` döner (CI default build
//! reqwest'sız compile). Gerçek HTTP impl: [`crate::login_http`]
//! (feature-gated). Captcha: headless browser YOK; `LoginResult::CaptchaRequired`
//! döner, shell modal açar (ADR-0011).

use std::time::Duration;

use qrcode::{QrCode, render::svg};
use secrecy::SecretString;
use serde::Deserialize;
use thiserror::Error;
use tracing::info;
use viscos_api::ViscosHttp;

use crate::mfa::{MfaMethod, generate_backup_codes, hash_backup_code};
use crate::storage::{AuthError, AuthStorage, StoredAccount, now_unix};
use crate::transport::SharedTransport;

/// Tüm login akışlarının döndüğü enum. `Debug` derive YOK:
/// `Success(StoredAccount)` `SecretString` barındırır (memory dump baseline
/// savunma, ADR-0011 §6). Print/log için `debug_redacted()`.
#[derive()]
#[non_exhaustive]
pub enum LoginResult {
    /// Token alındı, hesap keyring'e yazıldı.
    Success(StoredAccount),
    /// Internal intermediate — `login_*` fonksiyonu keyring'e yazıp `Success`'e çevirir.
    PendingStore { token: String, user_id: String },
    /// MFA TOTP kodu gerekli.
    MfaRequired {
        ticket: String,
        method: MfaMethod,
        user_hint: Option<String>,
    },
    /// MFA backup code gerekli.
    MfaBackupCodeRequired { ticket: String },
    /// Captcha gerekli — tarayıcıya yönlendir, token yapıştır.
    CaptchaRequired { url: String, message: String },
    /// QR login session süresi doldu.
    QrExpired,
    /// Login sırasında hata (network, JSON parse, vb.).
    Error(AuthError),
}

impl LoginResult {
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Token sızdırmayan debug çıktısı.
    #[must_use]
    pub fn debug_redacted(&self) -> String {
        match self {
            Self::Success(acc) => format!(
                "Success(user_id={}, username={})",
                acc.user_id, acc.username
            ),
            Self::PendingStore { user_id, .. } => format!("PendingStore(user_id={user_id})"),
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

impl std::fmt::Debug for LoginResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.debug_redacted())
    }
}

/// QR login session.
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
    /// `live-http` Cargo feature'ı kapalıyken Discord login çağrıldı. Default
    /// build reqwest-free; production binary `--features live-http` ile derlenir.
    /// Bu bir hata kodu yolu — stub değil.
    #[error(
        "Discord login requires the `live-http` Cargo feature; default build is reqwest-free for fast CI. Enable feature to use real auth."
    )]
    LiveHttpDisabled,
    /// QR login poll sırasında Discord hâlâ oturum açılmadı.
    #[error("QR login still pending; poll again after `poll_interval`")]
    StillPending,
    /// Network seviyesinde hata (DNS, TLS, timeout).
    #[error("network error: {0}")]
    Network(String),
}

/// `live-http` feature'ı kapalıyken döndürülen standart cevap. `Ok(...)`
/// içinde dönüyoruz çünkü shell `LoginResult::Error` kolunu zaten handle ediyor.
#[cfg(not(feature = "live-http"))]
fn live_http_disabled() -> LoginResult {
    LoginResult::Error(AuthError::LiveHttpDisabled)
}

/// Captcha response fixture — gerçek Discord `/auth/login` response'unun
/// simülasyonu (test/fixture için). `DiscordLoginResponse` (login_http.rs
/// içinde, `live-http` feature-gated) production parse için kullanılır;
/// bu tip **yalnızca test/fixture amaçlıdır** ve her iki build'de
/// derlenir (integration test'ler de kullanır).
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

/// URL için SVG QR kod üret (login_http.rs tarafından da çağrılır).
pub(crate) fn qr_svg_for_url(url: &str) -> Result<String, LoginError> {
    let qr = QrCode::new(url.as_bytes()).map_err(|e| LoginError::QrEncode(e.to_string()))?;
    Ok(qr
        .render::<svg::Color<'_>>()
        .min_dimensions(200, 200)
        .build())
}

/// Email + password login. `live-http` feature açıkken `transport` üzerinden
/// Discord `POST /api/v9/auth/login` çağrısı yapar. Kapalıyken
/// `LoginResult::Error(LiveHttpDisabled)` döner. Mock transport ile test
/// edilebilir — production HTTP dokunmadan.
///
/// # Errors
/// - [`LoginError::Network`] — DNS / TLS / timeout.
/// - [`LoginError::InvalidResponse`] — Discord JSON parse hatası.
/// - [`LoginError::Auth`] — keyring `store_account` başarısız.
pub async fn login_email(
    transport: &SharedTransport,
    storage: &AuthStorage,
    email: &str,
    password: &str,
) -> Result<LoginResult, LoginError> {
    if email.trim().is_empty() || password.is_empty() {
        return Ok(LoginResult::Error(AuthError::ValidationFailed(
            "email and password must be non-empty".to_string(),
        )));
    }
    #[cfg(feature = "live-http")]
    {
        let result =
            crate::login_http::post_email_login(transport.as_dyn(), email, password).await?;
        return finalize_login_result(storage, result);
    }
    #[cfg(not(feature = "live-http"))]
    {
        let _ = (transport, storage, email, password);
        Ok(live_http_disabled())
    }
}

/// QR login session başlat. `live-http` feature açıkken gerçek
/// `POST /auth/qr-login/start`, kapalıyken lokal SVG üretir.
pub async fn login_qr_start(transport: &SharedTransport) -> Result<QrSession, LoginError> {
    #[cfg(feature = "live-http")]
    {
        return crate::login_http::post_qr_start(transport.as_dyn()).await;
    }
    #[cfg(not(feature = "live-http"))]
    {
        let _ = transport;
        let session_id = format!("viscos-local-{}", now_unix());
        let url = format!("https://discord.com/ra/{session_id}");
        let qr_svg = qr_svg_for_url(&url)?;
        Ok(QrSession {
            session_id,
            qr_svg,
            poll_interval: Duration::from_secs(2),
        })
    }
}

/// QR login session poll. `live-http` feature açıkken gerçek Discord polling,
/// aksi halde `LiveHttpDisabled`.
pub async fn login_qr_poll(
    transport: &SharedTransport,
    storage: &AuthStorage,
    session_id: &str,
) -> Result<LoginResult, LoginError> {
    if session_id.is_empty() {
        return Ok(LoginResult::Error(AuthError::ValidationFailed(
            "session_id must be non-empty".to_string(),
        )));
    }
    #[cfg(feature = "live-http")]
    {
        let result = crate::login_http::get_qr_login(transport.as_dyn(), session_id).await?;
        return finalize_login_result(storage, result);
    }
    #[cfg(not(feature = "live-http"))]
    {
        let _ = (transport, storage, session_id);
        Ok(live_http_disabled())
    }
}

/// MFA TOTP/SMS code ile login challenge'ı tamamla (`POST /auth/mfa/{totp,sms}`).
/// Viscos TOTP secret'ı saklamaz; sadece 6-hanelik kodu Discord'a iletir.
pub async fn login_mfa(
    transport: &SharedTransport,
    storage: &AuthStorage,
    ticket: &str,
    code: &str,
    kind: MfaMethod,
) -> Result<LoginResult, LoginError> {
    if ticket.is_empty() || code.is_empty() {
        return Ok(LoginResult::Error(AuthError::ValidationFailed(
            "mfa ticket and code must be non-empty".to_string(),
        )));
    }
    #[cfg(feature = "live-http")]
    {
        let result = crate::login_http::post_mfa(transport.as_dyn(), ticket, code, kind).await?;
        return finalize_login_result(storage, result);
    }
    #[cfg(not(feature = "live-http"))]
    {
        let _ = (transport, storage, ticket, code, kind);
        Ok(live_http_disabled())
    }
}

/// Backup code doğrula ve login'i tamamla. 1) Önce local keyring kontrolü.
/// 2) Local match yoksa `LoginResult::Error(AuthError::ValidationFailed)`.
/// 3) Local match varsa `live-http` feature açıksa `POST /auth/mfa/backup`
/// Discord'a, kapalıysa local verify tek başına yeterli (offline test).
/// v1'de MFA backup code'lar keyring'de plaintext (Argon2 v2.0'da).
pub async fn login_mfa_backup(
    transport: &SharedTransport,
    storage: &AuthStorage,
    ticket: &str,
    user_id: &str,
    code: &str,
) -> Result<LoginResult, LoginError> {
    if !storage.verify_backup_code(user_id, code)? {
        return Ok(LoginResult::Error(AuthError::ValidationFailed(
            "invalid backup code".to_string(),
        )));
    }
    #[cfg(feature = "live-http")]
    {
        let result =
            crate::login_http::post_mfa(transport.as_dyn(), ticket, code, MfaMethod::BackupCode)
                .await?;
        return finalize_login_result(storage, result);
    }
    #[cfg(not(feature = "live-http"))]
    {
        let _ = (transport, ticket);
        match storage.load_account(user_id)? {
            Some(account) => Ok(LoginResult::Success(account)),
            None => Ok(LoginResult::Error(AuthError::AccountNotFound(
                user_id.to_string(),
            ))),
        }
    }
}

/// `LoginResult::PendingStore` → keyring store + `LoginResult::Success`.
/// Yalnızca `live-http` build'inde kullanılır (default build `LiveHttpDisabled`
/// döner ve `PendingStore` hiç üretilmez).
#[cfg(feature = "live-http")]
fn finalize_login_result(
    storage: &AuthStorage,
    result: LoginResult,
) -> Result<LoginResult, LoginError> {
    match result {
        LoginResult::PendingStore { token, user_id } => {
            let account = store_pending(storage, &token, &user_id)?;
            Ok(LoginResult::Success(account))
        }
        other => Ok(other),
    }
}

/// `LoginResult::PendingStore` ham token bilgisinden keyring'e yazıp
/// `StoredAccount` döndürür. Yalnızca `live-http` build'inde `finalize_login_result`
/// tarafından çağrılır.
#[cfg(feature = "live-http")]
fn store_pending(
    storage: &AuthStorage,
    token: &str,
    user_id: &str,
) -> Result<StoredAccount, LoginError> {
    let account = StoredAccount {
        user_id: user_id.to_string(),
        username: String::new(),
        token: SecretString::new(token.to_string().into_boxed_str()),
        mfa_backup_hashes: Vec::new(),
        super_properties: None,
        created_at: now_unix(),
        last_validated_at: now_unix(),
    };
    storage.store_account(&account)?;
    info!(user_id = %account.user_id, "login: account stored");
    Ok(account)
}

/// Pre-existing token ile login. Keyring'e yazıp `current_user` validation yapar.
/// Captcha redirect sonrası "token yapıştır" akışı da buraya düşer.
pub async fn login_token(
    storage: &AuthStorage,
    api: &ViscosHttp,
    token: &str,
) -> Result<StoredAccount, LoginError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(LoginError::InvalidResponse("empty token".to_string()));
    }
    let user = api
        .current_user()
        .await
        .map_err(|e| LoginError::InvalidResponse(format!("current_user failed: {e}")))?;
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

/// MFA backup code'ları üret (10 adet, 8-karakterli alphanumeric).
#[must_use]
pub fn issue_backup_codes() -> Vec<String> {
    generate_backup_codes()
}

/// Verilen backup code listesini hash'le ve `StoredAccount`'a yaz.
/// v1 inert: hash fonksiyonu no-op. v2.0'da Argon2 PHC aktif.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;
    use secrecy::ExposeSecret;
    use std::sync::Arc;

    fn mock_transport() -> SharedTransport {
        SharedTransport::mock(Arc::new(MockTransport::new()))
    }

    fn run<F: std::future::Future<Output = Result<LoginResult, LoginError>>>(f: F) -> LoginResult {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        rt.block_on(f).expect("no panic")
    }

    fn run_qr<F: std::future::Future<Output = Result<QrSession, LoginError>>>(f: F) -> QrSession {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        rt.block_on(f).expect("no panic")
    }

    fn assert_live_http_disabled(r: &LoginResult) {
        let redacted = r.debug_redacted();
        assert!(
            matches!(r, LoginResult::Error(AuthError::LiveHttpDisabled)),
            "got {redacted}"
        );
    }

    #[test]
    fn login_result_success_predicate() {
        assert!(LoginResult::Success(StoredAccount::default()).is_success());
        assert!(!LoginResult::QrExpired.is_success());
    }

    #[test]
    fn issue_backup_codes_produces_10() {
        use crate::mfa::is_valid_backup_code_format;
        let codes = issue_backup_codes();
        assert_eq!(codes.len(), 10);
        for c in &codes {
            assert!(is_valid_backup_code_format(c), "invalid: {c}");
        }
    }

    #[test]
    fn attach_backup_codes_writes_to_account() {
        let mut acc = StoredAccount::default();
        attach_backup_codes_to_account(
            &mut acc,
            &["ABCD-1234".to_string(), "EFGH-5678".to_string()],
        );
        assert_eq!(acc.mfa_backup_hashes.len(), 2);
        assert_eq!(acc.mfa_backup_hashes[0].expose_secret(), "ABCD-1234");
    }

    #[test]
    fn login_email_default_build_returns_live_http_disabled() {
        let r = run(login_email(
            &mock_transport(),
            &AuthStorage::new(),
            "a@b.c",
            "p",
        ));
        #[cfg(not(feature = "live-http"))]
        assert_live_http_disabled(&r);
        #[cfg(feature = "live-http")]
        {
            let _ = r;
        }
    }

    #[test]
    fn login_email_rejects_empty_inputs() {
        let r = run(login_email(&mock_transport(), &AuthStorage::new(), "", "p"));
        assert!(matches!(
            r,
            LoginResult::Error(AuthError::ValidationFailed(_))
        ));
    }

    #[test]
    fn login_qr_start_returns_session_with_svg() {
        let s = run_qr(login_qr_start(&mock_transport()));
        assert!(!s.session_id.is_empty());
        assert!(s.qr_svg.contains("<svg") && s.qr_svg.contains("</svg>"));
    }

    #[test]
    fn login_qr_poll_default_build_returns_live_http_disabled() {
        let r = run(login_qr_poll(
            &mock_transport(),
            &AuthStorage::new(),
            "session",
        ));
        #[cfg(not(feature = "live-http"))]
        assert_live_http_disabled(&r);
        #[cfg(feature = "live-http")]
        {
            let _ = r;
        }
    }

    #[test]
    fn login_qr_poll_rejects_empty_session() {
        let r = run(login_qr_poll(&mock_transport(), &AuthStorage::new(), ""));
        assert!(matches!(
            r,
            LoginResult::Error(AuthError::ValidationFailed(_))
        ));
    }

    #[test]
    fn login_mfa_totp_default_build_returns_live_http_disabled() {
        let r = run(login_mfa(
            &mock_transport(),
            &AuthStorage::new(),
            "ticket",
            "123456",
            MfaMethod::Totp,
        ));
        #[cfg(not(feature = "live-http"))]
        assert_live_http_disabled(&r);
        #[cfg(feature = "live-http")]
        {
            let _ = r;
        }
    }

    #[test]
    fn login_mfa_rejects_empty_inputs() {
        let r = run(login_mfa(
            &mock_transport(),
            &AuthStorage::new(),
            "",
            "123456",
            MfaMethod::Totp,
        ));
        assert!(matches!(
            r,
            LoginResult::Error(AuthError::ValidationFailed(_))
        ));
    }

    #[test]
    fn login_mfa_backup_no_account_returns_error() {
        // Keyring entry yoksa `verify_backup_code` `AccountNotFound` döner
        // (Windows native store kuruluyken) veya keyring kurulmamışsa
        // `Keyring(NoDefaultStore)` döner (test ortamı). Her iki durum da
        // `LoginResult::Error` veya `Err(LoginError::...)` ile sonuçlanır.
        let transport = mock_transport();
        let storage = AuthStorage::new();
        let f = login_mfa_backup(&transport, &storage, "ticket", "no-such-user", "BADC0DE");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let res = rt.block_on(f);
        match res {
            Ok(LoginResult::Error(_)) => {}
            Err(_) => {}
            _other => panic!("expected error path, got LoginResult"),
        }
    }

    #[test]
    fn login_error_display_includes_human_readable_message() {
        assert!(
            LoginError::LiveHttpDisabled
                .to_string()
                .contains("live-http")
        );
        assert!(LoginError::StillPending.to_string().contains("poll"));
        assert_eq!(
            LoginError::Network("dns".to_string()).to_string(),
            "network error: dns"
        );
    }

    #[test]
    fn login_result_debug_redacted_does_not_leak_token() {
        let acc = StoredAccount {
            user_id: "123".to_string(),
            username: "tester".to_string(),
            token: SecretString::new("super-secret-token-12345".to_string().into_boxed_str()),
            mfa_backup_hashes: vec![],
            super_properties: None,
            created_at: 0,
            last_validated_at: 0,
        };
        let s = LoginResult::Success(acc).debug_redacted();
        assert!(s.contains("user_id=123"));
        assert!(!s.contains("super-secret-token"));
    }
}
