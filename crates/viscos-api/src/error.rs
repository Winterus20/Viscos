//! Typed API hataları — `twilight_http::Error` ve
//! `twilight_http::response::DeserializeBodyError` türlerini Viscos'un
//! `ApiError` enum'una map eder.
//!
//! **Tasarım:** Bu enum `#[non_exhaustive]` — dış tüketiciler exhaustive match
//! yapamaz, AI PR'da yeni variant eklemek non-breaking olur. `From<twilight_http::Error>`
//! adaptörü otomatik `?` ile dönüşüm sağlar.

use thiserror::Error;

/// Viscos'un REST katmanı hata modeli. `twilight_http::Error` zaten zengin
/// (rate-limit, 401, JSON decode, network), biz onu iş katmanı tiplerine
/// çeviririz.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ApiError {
    /// Discord API'si 4xx/5xx döndü (status + body parse edilmiş).
    #[error("discord api error status={status}: {message}")]
    Discord {
        status: u16,
        message: String,
        /// `Retry-After` header'ı varsa (429 durumu), saniye cinsinden.
        retry_after_secs: Option<f64>,
    },

    /// Discord token'ı 401 ile geçersiz oldu. Twilight otomatik olarak
    /// sonraki request'leri reddetmeye başlar (ADR-0008, twilight docs §Unauthorized).
    #[error("unauthorized: token invalid or expired (re-login required)")]
    Unauthorized,

    /// Twilight internal rate-limit kuyruğu 429 döndü ama twilight zaten
    /// otomatik retry yapıyor olmalı. Görünürse twilight bug'ı olabilir.
    #[error("rate limited (twilight internal retry exhausted)")]
    RateLimited,

    /// Response body'si JSON decode edilemedi.
    #[error("json decode error: {0}")]
    Decode(String),

    /// Network / TLS / timeout hatası (twilight transport katmanı).
    #[error("transport error: {0}")]
    Transport(String),

    /// Twilight client kurulu değil (builder hatası) veya request inşa
    /// hatası (ör. 0 uzunluklu content).
    #[error("request build error: {0}")]
    Build(String),

    /// `expose_secret()` audit noktası dışındaki tüm call site'lar bu variant'a
    /// düşmemeli. Test amaçlı explicit.
    #[error("other twilight error: {0}")]
    Other(String),
}

impl From<twilight_http::Error> for ApiError {
    fn from(err: twilight_http::Error) -> Self {
        use twilight_http::api_error::ApiError as TwilightApiError;
        use twilight_http::error::ErrorType;
        match err.kind() {
            ErrorType::Unauthorized => ApiError::Unauthorized,
            ErrorType::Response { status, error, .. } => {
                let status = status.get();
                // 429'da twilight normalde otomatik retry yapar; buraya düşmesi
                // beklenmez. Yine de handle edelim.
                if status == 429 {
                    ApiError::RateLimited
                } else {
                    let (message, retry_after) = match error {
                        TwilightApiError::General(g) => {
                            (format!("{}: {}", g.code, g.message), None)
                        }
                        TwilightApiError::Ratelimited(r) => {
                            (r.message.clone(), Some(r.retry_after))
                        }
                        TwilightApiError::Message(_) => {
                            ("message validation error".to_string(), None)
                        }
                        _ => ("unknown api error".to_string(), None),
                    };
                    ApiError::Discord {
                        status,
                        message,
                        retry_after_secs: retry_after,
                    }
                }
            }
            ErrorType::Json | ErrorType::Parsing { .. } => ApiError::Decode(err.to_string()),
            ErrorType::RequestError | ErrorType::RequestTimedOut | ErrorType::RequestCanceled => {
                ApiError::Transport(err.to_string())
            }
            ErrorType::BuildingRequest
            | ErrorType::CreatingHeader { .. }
            | ErrorType::Validation => ApiError::Build(err.to_string()),
            _ => ApiError::Other(err.to_string()),
        }
    }
}

impl From<twilight_http::response::DeserializeBodyError> for ApiError {
    fn from(err: twilight_http::response::DeserializeBodyError) -> Self {
        ApiError::Decode(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_display_contains_status() {
        let err = ApiError::Discord {
            status: 404,
            message: "Unknown Channel".to_string(),
            retry_after_secs: None,
        };
        let s = err.to_string();
        assert!(s.contains("404"));
        assert!(s.contains("Unknown Channel"));
    }

    #[test]
    fn unauthorized_display_is_actionable() {
        let err = ApiError::Unauthorized;
        assert!(err.to_string().contains("re-login"));
    }

    #[test]
    fn non_exhaustive_compiles() {
        // `#[non_exhaustive]` sayesinde wildcard pattern geçerli.
        let err = ApiError::RateLimited;
        match err {
            ApiError::RateLimited => {}
            _ => panic!("unreachable"),
        }
    }
}
