//! Discord login HTTP flows — `live-http` feature-gated implementations.
//!
//! Bu modül yalnızca `live-http` Cargo feature'ı aktifken derlenir
//! (`#[cfg(feature = "live-http")]`). Default build (CI cross-platform)
//! dosyayı tamamen atlar; [`crate::login`] üzerindeki public fonksiyonlar
//! [`crate::login::LoginError::LiveHttpDisabled`] döner.
//!
//! **Discord endpoint'leri (undocumented, ToS riskli):**
//! - `POST /api/v9/auth/login` — email + password
//! - `POST /api/v9/auth/mfa/{totp,sms,backup}` — MFA submit
//! - `POST /api/v9/auth/qr-login/start` — QR session başlat
//! - `GET  /api/v9/auth/qr-login/{session_id}` — QR poll
//!
//! **Request gövdeleri:** JSON. `X-Super-Properties` + `User-Agent` header'ları
//! [`crate::transport::discord_login_headers`] helper'ından gelir.

#[cfg(feature = "live-http")]
use serde::Deserialize;
#[cfg(feature = "live-http")]
use tracing::{info, instrument};

#[cfg(feature = "live-http")]
use crate::login::{LoginError, LoginResult, QrSession};
#[cfg(feature = "live-http")]
use crate::mfa::MfaMethod;
#[cfg(feature = "live-http")]
use crate::storage::AuthError;
#[cfg(feature = "live-http")]
use crate::transport::{HttpMethod, Transport, TransportError, TransportRequest};

/// Discord response shape — `tag` discriminator yerine tüm alanları
/// `Option` ile parse ediyoruz; her status code için uygun alanı okuyoruz.
#[cfg(feature = "live-http")]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct DiscordLoginResponse {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub mfa: Option<MfaPayload>,
    #[serde(default)]
    pub captcha_key: Option<Vec<String>>,
    #[serde(default)]
    pub sitekey: Option<String>,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[cfg(feature = "live-http")]
#[derive(Debug, Deserialize)]
pub(crate) struct MfaPayload {
    pub ticket: String,
    #[serde(default)]
    pub mfa_type: Option<String>,
    #[serde(default)]
    pub sms: Option<bool>,
    #[serde(default)]
    pub totp: Option<bool>,
}

#[cfg(feature = "live-http")]
fn parse_mfa_type(
    mfa_type: Option<&str>,
    totp_flag: Option<bool>,
    sms_flag: Option<bool>,
) -> MfaMethod {
    match mfa_type {
        Some("totp") | Some("Totp") => MfaMethod::Totp,
        Some("sms") | Some("Sms") => MfaMethod::Sms,
        Some("backup") | Some("Backup") => MfaMethod::BackupCode,
        _ => {
            if totp_flag == Some(true) {
                MfaMethod::Totp
            } else if sms_flag == Some(true) {
                MfaMethod::Sms
            } else {
                MfaMethod::Totp
            }
        }
    }
}

#[cfg(feature = "live-http")]
fn parse_response(status: u16, body: &str) -> Result<DiscordLoginResponse, LoginError> {
    serde_json::from_str(body)
        .map_err(|e| LoginError::InvalidResponse(format!("status {status}: parse: {e}")))
}

#[cfg(feature = "live-http")]
fn mfa_to_login_result(mfa: MfaPayload) -> LoginResult {
    let method = parse_mfa_type(mfa.mfa_type.as_deref(), mfa.totp, mfa.sms);
    LoginResult::MfaRequired {
        ticket: mfa.ticket,
        method,
        user_hint: None,
    }
}

#[cfg(feature = "live-http")]
fn captcha_to_login_result() -> LoginResult {
    LoginResult::CaptchaRequired {
        url: "https://discord.com/captcha".to_string(),
        message: "Discord güvenlik kontrolü istiyor — captcha'yı tarayıcıda geçin, ardından token'ı yapıştırın.".to_string(),
    }
}

#[cfg(feature = "live-http")]
fn failure_to_login_result(parsed: &DiscordLoginResponse, status: u16) -> LoginResult {
    LoginResult::Error(AuthError::LoginFailed {
        code: parsed
            .code
            .clone()
            .unwrap_or_else(|| format!("http_{status}")),
        message: parsed
            .message
            .clone()
            .unwrap_or_else(|| "login failed".to_string()),
    })
}

/// `POST /api/v9/auth/login` — gerçek email/şifre login.
///
/// `transport.send()` üzerinden Discord'a POST atar. Yanıt ayrıştırılır ve
/// [`LoginResult`] varyantlarına dönüştürülür. Başarı durumunda token ve
/// `user_id` `LoginResult::PendingStore` içinde döner; caller (`login_email`)
/// keyring'e yazma işlemini yapar.
///
/// # Errors
///
/// - [`LoginError::Network`] — transport hatası (DNS, TLS, timeout).
/// - [`LoginError::InvalidResponse`] — Discord JSON parse hatası.
#[cfg(feature = "live-http")]
#[instrument(skip(transport, password), fields(email))]
pub(crate) async fn post_email_login(
    transport: &dyn Transport,
    email: &str,
    password: &str,
) -> Result<LoginResult, LoginError> {
    let body = serde_json::json!({
        "login": email,
        "password": password,
        "undelete": false,
        "login_source": null,
        "gift_code_sku_id": null,
    });
    let mut headers = crate::transport::discord_login_headers();
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    headers.push(("X-Discord-Locale".to_string(), "en-US".to_string()));

    let req = TransportRequest {
        url: "https://discord.com/api/v9/auth/login".to_string(),
        method: HttpMethod::Post,
        body: Some(body),
        headers,
    };
    let resp = transport.send(req).await.map_err(map_transport_err)?;
    let status = resp.status;
    let parsed = parse_response(status, &resp.body)?;
    Ok(discord_response_to_login_result(status, &parsed))
}

#[cfg(feature = "live-http")]
fn discord_response_to_login_result(status: u16, parsed: &DiscordLoginResponse) -> LoginResult {
    if (200..300).contains(&status) {
        if let (Some(token), Some(user_id)) = (parsed.token.as_ref(), parsed.user_id.as_ref()) {
            return LoginResult::PendingStore {
                token: token.clone(),
                user_id: user_id.clone(),
            };
        }
        if let Some(mfa) = parsed.mfa.as_ref() {
            return mfa_to_login_result(MfaPayload {
                ticket: mfa.ticket.clone(),
                mfa_type: mfa.mfa_type.clone(),
                sms: mfa.sms,
                totp: mfa.totp,
            });
        }
        return LoginResult::Error(AuthError::LoginFailed {
            code: "malformed_response".to_string(),
            message: "200 status without token or mfa payload".to_string(),
        });
    }
    if let Some(mfa) = parsed.mfa.as_ref() {
        return mfa_to_login_result(MfaPayload {
            ticket: mfa.ticket.clone(),
            mfa_type: mfa.mfa_type.clone(),
            sms: mfa.sms,
            totp: mfa.totp,
        });
    }
    if parsed.captcha_key.is_some() || parsed.sitekey.is_some() {
        return captcha_to_login_result();
    }
    failure_to_login_result(parsed, status)
}

/// `GET /api/v9/auth/qr-login/{session_id}` — QR session poll.
#[cfg(feature = "live-http")]
#[instrument(skip(transport))]
pub(crate) async fn get_qr_login(
    transport: &dyn Transport,
    session_id: &str,
) -> Result<LoginResult, LoginError> {
    let url = format!("https://discord.com/api/v9/auth/qr-login/{session_id}");
    let mut headers = crate::transport::discord_login_headers();
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    let req = TransportRequest {
        url,
        method: HttpMethod::Get,
        body: None,
        headers,
    };
    let resp = transport.send(req).await.map_err(map_transport_err)?;
    let status = resp.status;
    let parsed: DiscordLoginResponse = parse_response(status, &resp.body)?;

    if (200..300).contains(&status) {
        if let (Some(token), Some(user_id)) = (parsed.token.as_ref(), parsed.user_id.as_ref()) {
            return Ok(LoginResult::PendingStore {
                token: token.clone(),
                user_id: user_id.clone(),
            });
        }
        return Err(LoginError::InvalidResponse(format!(
            "200 without token/user_id: {}",
            resp.body
        )));
    }

    if status == 400 || status == 404 {
        let code = parsed
            .code
            .clone()
            .unwrap_or_else(|| "still_pending".to_string());
        if code == "still_pending" {
            return Ok(LoginResult::Error(AuthError::StillPending));
        }
        return Ok(failure_to_login_result(&parsed, status));
    }

    Ok(failure_to_login_result(&parsed, status))
}

/// `POST /api/v9/auth/qr-login/start` — QR session başlat.
#[cfg(feature = "live-http")]
#[instrument(skip(transport))]
pub(crate) async fn post_qr_start(transport: &dyn Transport) -> Result<QrSession, LoginError> {
    let url = "https://discord.com/api/v9/auth/qr-login/start".to_string();
    let mut headers = crate::transport::discord_login_headers();
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    let req = TransportRequest {
        url,
        method: HttpMethod::Post,
        body: Some(serde_json::json!({})),
        headers,
    };
    let resp = transport.send(req).await.map_err(map_transport_err)?;
    if resp.status != 200 {
        return Err(LoginError::InvalidResponse(format!(
            "qr start status {}: {}",
            resp.status, resp.body
        )));
    }
    #[derive(serde::Deserialize)]
    struct QrStartResp {
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        qr_code: Option<String>,
        #[serde(default)]
        qr_code_url: Option<String>,
        #[serde(default)]
        polling_interval_ms: Option<u64>,
    }
    let parsed: QrStartResp = serde_json::from_str(&resp.body)
        .map_err(|e| LoginError::InvalidResponse(format!("qr start parse: {e}")))?;
    let session_id = parsed
        .session_id
        .or(parsed.qr_code)
        .ok_or_else(|| LoginError::InvalidResponse("qr start: missing session_id".to_string()))?;
    let qr_url = parsed
        .qr_code_url
        .unwrap_or_else(|| format!("https://discord.com/ra/{session_id}"));
    let interval = parsed.polling_interval_ms.unwrap_or(2_000);
    let qr_svg = crate::login::qr_svg_for_url(&qr_url)?;
    info!(session_id = %session_id, interval_ms = interval, "qr session started");
    Ok(QrSession {
        session_id,
        qr_svg,
        poll_interval: std::time::Duration::from_millis(interval),
    })
}

/// `POST /api/v9/auth/mfa/{kind}` — TOTP/SMS/backup code ile MFA submit.
#[cfg(feature = "live-http")]
#[instrument(skip(transport, code), fields(kind = ?kind))]
pub(crate) async fn post_mfa(
    transport: &dyn Transport,
    ticket: &str,
    code: &str,
    kind: MfaMethod,
) -> Result<LoginResult, LoginError> {
    let path = match kind {
        MfaMethod::Totp => "totp",
        MfaMethod::Sms => "sms",
        MfaMethod::BackupCode => "backup",
    };
    let url = format!("https://discord.com/api/v9/auth/mfa/{path}");
    let body = serde_json::json!({
        "ticket": ticket,
        "code": code,
    });
    let mut headers = crate::transport::discord_login_headers();
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    let req = TransportRequest {
        url,
        method: HttpMethod::Post,
        body: Some(body),
        headers,
    };
    let resp = transport.send(req).await.map_err(map_transport_err)?;
    let status = resp.status;
    let parsed: DiscordLoginResponse = parse_response(status, &resp.body)?;
    if (200..300).contains(&status) {
        if let (Some(token), Some(user_id)) = (parsed.token.as_ref(), parsed.user_id.as_ref()) {
            return Ok(LoginResult::PendingStore {
                token: token.clone(),
                user_id: user_id.clone(),
            });
        }
        if let Some(mfa) = parsed.mfa.as_ref() {
            return Ok(mfa_to_login_result(MfaPayload {
                ticket: mfa.ticket.clone(),
                mfa_type: mfa.mfa_type.clone(),
                sms: mfa.sms,
                totp: mfa.totp,
            }));
        }
        return Err(LoginError::InvalidResponse(format!(
            "mfa 200 without token/mfa: {}",
            resp.body
        )));
    }
    Ok(failure_to_login_result(&parsed, status))
}

/// `LoginResult::PendingStore` → keyring store. Bu fonksiyon `login.rs`'de
/// tanımlıdır (her iki build'de kullanılır); burada tekrar tanımlamıyoruz.

#[cfg(feature = "live-http")]
fn map_transport_err(e: TransportError) -> LoginError {
    match e {
        TransportError::Network(m) => LoginError::Network(m),
        TransportError::HttpStatus { status, body } => {
            LoginError::InvalidResponse(format!("HTTP {status}: {body}"))
        }
        TransportError::Serialization(m) => LoginError::InvalidResponse(m),
        TransportError::MockExhausted => {
            LoginError::InvalidResponse("mock transport queue exhausted".to_string())
        }
    }
}
