//! HTTP transport abstraction for Discord login endpoints.
//!
//! **Mimari (ADR-0011 §5):** Login akışları (`/auth/login`, `/auth/mfa/*`,
//! `/auth/qr-code/login/*`) Discord'un undocumented endpoint'leri. Twilight
//! bunları sağlamaz; Viscos `reqwest` ile manual HTTP yapar.
//!
//! **`reqwest` dependency'si henüz workspace'te YOK** (cargo.toml raporuna
//! bakın). Bu modül iki tür transport tanımlar:
//!
//! - [`Transport`] — async trait. Production'da [`ReqwestTransport`]
//!   (`live-http` feature'ı arkasında), test'te [`MockTransport`] (her zaman
//!   available).
//! - [`MockTransport`] — `Mutex<Vec<TransportResponse>>` queue'sinden response
//!   dispatch eder; wiremock'a gerek kalmadan `#[tokio::test]` içinde kullanılır.
//!
//! **Default build (`cargo build`):** `live-http` feature'ı kapalı → reqwest
//! dependency'si **compile edilmez** → CI cross-platform build hızlı kalır.
//! Production binary (`cargo build --features live-http`) reqwest'i çeker.
//!
//! ## Neden `async_trait` yerine native `async fn in trait`?
//!
//! Rust 1.75+ native `async fn in trait` destekliyor. Ancak `dyn Transport`
//! uyumluluğu (mock setup için) `#[async_trait]` macro'sunu zorunlu kılar.
//! workspace'te `async-trait = "0.1"` zaten var (viscos-cache); aynı
//! pattern'i uyguluyoruz.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::storage::AuthError;

/// Transport hata türü — `AuthError::NetworkError(String)` ile uyumlu.
///
/// `#[from]` ile `?` propagasyonu destekler; caller tarafında
/// `AuthError::Network` veya `AuthError::Mfa` ile map'lenebilir.
#[derive(Debug, Error)]
pub enum TransportError {
    /// JSON serialize / deserialize hatası.
    #[error("transport serialization error: {0}")]
    Serialization(String),
    /// Network / DNS / TLS / timeout — reqwest error wrapper.
    #[error("transport network error: {0}")]
    Network(String),
    /// HTTP 4xx/5xx response — status kodu korunur.
    #[error("transport HTTP {status}: {body}")]
    HttpStatus {
        /// HTTP status kodu (200-599).
        status: u16,
        /// Response body (parse edilmeden ham).
        body: String,
    },
    /// Mock transport queue boşaldı (testte kontrat ihlali).
    #[error("mock transport queue exhausted")]
    MockExhausted,
}

impl From<TransportError> for AuthError {
    fn from(err: TransportError) -> Self {
        match err {
            TransportError::Serialization(m) => AuthError::Serde(serde_json::Error::io(
                std::io::Error::new(std::io::ErrorKind::InvalidData, m),
            )),
            TransportError::Network(m) => AuthError::NetworkError(m),
            TransportError::HttpStatus { status, body } => {
                AuthError::NetworkError(format!("HTTP {status}: {body}"))
            }
            TransportError::MockExhausted => {
                AuthError::NetworkError("mock transport queue exhausted".to_string())
            }
        }
    }
}

/// HTTP request gövdesi.
///
/// `serde_json::Value` kullanmak typed struct'ların testte `json!({...})` ile
/// mock response vermesini kolaylaştırır. Production'da typed struct →
/// `serde_json::to_value` ile dönüştürülür.
#[derive(Debug, Clone)]
pub struct TransportRequest {
    /// Tam URL (ör. `https://discord.com/api/v9/auth/login`).
    pub url: String,
    /// HTTP method (`GET`, `POST`, ...).
    pub method: HttpMethod,
    /// Request body (POST için JSON). `None` GET için.
    pub body: Option<serde_json::Value>,
    /// Header'lar (User-Agent, X-Super-Properties, Content-Type).
    pub headers: Vec<(String, String)>,
}

/// HTTP method enum — `reqwest::Method`'a doğrudan map'lenebilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// HTTP response — body ham JSON (parse caller'da yapılır).
#[derive(Debug, Clone)]
pub struct TransportResponse {
    /// HTTP status kodu.
    pub status: u16,
    /// Response body — `application/json` parse edilebilir olmalı.
    pub body: String,
    /// Response header'ları — Discord `set-cookie` veya `retry-after` için.
    #[allow(dead_code)]
    pub headers: Vec<(String, String)>,
}

impl TransportResponse {
    /// 2xx success builder.
    #[must_use]
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            headers: Vec::new(),
        }
    }

    /// Belirli status + body builder.
    #[must_use]
    pub fn with_status(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            headers: Vec::new(),
        }
    }
}

/// Async HTTP transport trait.
///
/// Production implementasyonu ([`ReqwestTransport`]) `live-http` feature'ı
/// arkasında derlenir; test implementasyonu ([`MockTransport`]) her zaman
/// available. `dyn Transport` uyumluluğu `#[async_trait]` macro'su ile sağlanır.
#[async_trait]
pub trait Transport: Send + Sync {
    /// `TransportRequest` → `TransportResponse`.
    ///
    /// # Errors
    ///
    /// - [`TransportError::Network`] — DNS / TLS / connect / read timeout.
    /// - [`TransportError::Serialization`] — body JSON encode hatası.
    /// - [`TransportError::HttpStatus`] — 4xx/5xx response (caller parse eder).
    async fn send(&self, req: TransportRequest) -> Result<TransportResponse, TransportError>;
}

// ---------------------------------------------------------------------------
// MockTransport — her zaman available, wiremock bağımlılığı yok
// ---------------------------------------------------------------------------

/// In-memory mock transport — testlerde deterministic HTTP davranışı sağlar.
///
/// **Kullanım:**
/// ```ignore
/// use viscos_auth::transport::{MockTransport, TransportRequest, HttpMethod, TransportResponse};
/// use std::sync::Arc;
///
/// let mock = Arc::new(MockTransport::new());
/// mock.enqueue(TransportResponse::ok(r#"{"token": "abc"}"#));
///
/// let req = TransportRequest {
///     url: "https://discord.com/api/v9/auth/login".to_string(),
///     method: HttpMethod::Post,
///     body: None,
///     headers: vec![],
/// };
/// let resp = mock.send(req).await.unwrap();
/// assert_eq!(resp.status, 200);
/// ```
#[derive(Debug, Default)]
pub struct MockTransport {
    /// FIFO queue — `send()` her çağrıda bir response consume eder.
    queue: Mutex<VecDeque<TransportResponse>>,
    /// Tüm request'lerin log'u (assertion için).
    pub recorded: Mutex<Vec<TransportRequest>>,
}

impl MockTransport {
    /// Boş queue ile yeni mock.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            recorded: Mutex::new(Vec::new()),
        }
    }

    /// Queue'ya response ekle (FIFO).
    pub fn enqueue(&self, resp: TransportResponse) {
        self.queue
            .lock()
            .expect("MockTransport queue mutex not poisoned")
            .push_back(resp);
    }

    /// Queue'ya birden fazla response ekle (sırayla consume edilir).
    pub fn enqueue_many<I: IntoIterator<Item = TransportResponse>>(&self, responses: I) {
        let mut q = self
            .queue
            .lock()
            .expect("MockTransport queue mutex not poisoned");
        for r in responses {
            q.push_back(r);
        }
    }

    /// Şimdiye kadar kaydedilen request sayısı.
    #[must_use]
    pub fn recorded_count(&self) -> usize {
        self.recorded
            .lock()
            .expect("MockTransport recorded mutex not poisoned")
            .len()
    }

    /// İlk kaydedilen request'in body'sini döner (URL match zorunlu değil).
    #[must_use]
    pub fn first_body(&self) -> Option<serde_json::Value> {
        self.recorded
            .lock()
            .expect("recorded mutex")
            .first()
            .and_then(|r| r.body.clone())
    }

    /// İlk kaydedilen request'in URL'ini döner.
    #[must_use]
    pub fn first_url(&self) -> Option<String> {
        self.recorded
            .lock()
            .expect("recorded mutex")
            .first()
            .map(|r| r.url.clone())
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&self, req: TransportRequest) -> Result<TransportResponse, TransportError> {
        // Request'i kaydet (assertion için).
        self.recorded
            .lock()
            .expect("recorded mutex")
            .push(req.clone());

        // Queue'dan response consume et.
        let resp = self
            .queue
            .lock()
            .expect("queue mutex")
            .pop_front()
            .ok_or(TransportError::MockExhausted)?;
        Ok(resp)
    }
}

/// `Arc<dyn Transport>` newtype helper — login fonksiyonlarına parametre geçişi.
#[derive(Clone)]
pub struct SharedTransport(pub Arc<dyn Transport>);

impl SharedTransport {
    /// `Arc<MockTransport>`'u `SharedTransport`'a sar.
    #[must_use]
    pub fn mock(mock: Arc<MockTransport>) -> Self {
        let arc: Arc<dyn Transport> = mock;
        Self(arc)
    }

    /// Trait objesine referans döner (`transport.send(...)` çağrıları için).
    #[must_use]
    pub fn as_dyn(&self) -> &dyn Transport {
        &*self.0
    }
}

#[async_trait]
impl Transport for SharedTransport {
    async fn send(&self, req: TransportRequest) -> Result<TransportResponse, TransportError> {
        self.0.send(req).await
    }
}

// ---------------------------------------------------------------------------
// ReqwestTransport — `live-http` feature'ı arkasında, production binary
// ---------------------------------------------------------------------------

/// `reqwest::Client` sarmalayıcı — production HTTP transport.
///
/// `#[cfg(feature = "live-http")]` guard'ı ile default build'de **compile
/// edilmez**. `cargo build --features live-http` ile reqwest tabanlı implementasyon
/// aktive olur.
///
/// **Statik `reqwest::Client` reuse:** DNS connection pool'u her login'de
/// sıfırdan kurmamak için `OnceLock` ile process-global tek instance.
#[cfg(feature = "live-http")]
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

#[cfg(feature = "live-http")]
impl ReqwestTransport {
    /// Yeni transport — `reqwest::Client::builder()` ile timeout'lu client.
    ///
    /// **5 saniye timeout (ADR-0011):** Faz 2.0 user task'ları için makul.
    /// Faz 4'te config'ten override edilebilir.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .user_agent(concat!(
                "Viscos/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/viscos/viscos)"
            ))
            .build()
            .expect("reqwest::Client::builder with timeout always succeeds");
        Self { client }
    }
}

#[cfg(feature = "live-http")]
impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "live-http")]
#[async_trait]
impl Transport for ReqwestTransport {
    async fn send(&self, req: TransportRequest) -> Result<TransportResponse, TransportError> {
        let method = match req.method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
        };
        let mut builder = self.client.request(method, &req.url);
        for (k, v) in &req.headers {
            builder = builder.header(k, v);
        }
        if let Some(body) = req.body {
            builder = builder.json(&body);
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| TransportError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp
            .text()
            .await
            .map_err(|e| TransportError::Network(e.to_string()))?;
        Ok(TransportResponse {
            status,
            body,
            headers,
        })
    }
}

// ---------------------------------------------------------------------------
// Shared headers helper
// ---------------------------------------------------------------------------

/// Discord login request'lerinde kullanılan sabit header seti.
///
/// `User-Agent`, `X-Super-Properties`, `Content-Type` üretir; caller
/// login fonksiyonları `TransportRequest` oluştururken bu helper'ı çağırır.
///
/// **Detay:** `X-Super-Properties` base64-encoded JSON; `super_properties.rs`
/// üretir. ADR-0011 §3 fingerprint stability şartı.
#[must_use]
pub fn discord_login_headers() -> Vec<(String, String)> {
    let mut headers = Vec::with_capacity(3);
    headers.push((
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
    ));
    headers.push((
        "X-Super-Properties".to_string(),
        crate::super_properties::build_x_super_properties_header(),
    ));
    headers
}

/// Body builder — `T: Serialize` değerini `serde_json::Value`'ya çevirir.
pub fn json_body<T: Serialize>(value: &T) -> Result<serde_json::Value, TransportError> {
    serde_json::to_value(value).map_err(|e| TransportError::Serialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_transport_consumes_responses_in_order() {
        let mock = Arc::new(MockTransport::new());
        mock.enqueue(TransportResponse::ok(r#"{"first": true}"#));
        mock.enqueue(TransportResponse::ok(r#"{"second": true}"#));

        let r1 = mock
            .send(TransportRequest {
                url: "https://example.com/a".to_string(),
                method: HttpMethod::Get,
                body: None,
                headers: vec![],
            })
            .await
            .expect("first response");
        assert_eq!(r1.status, 200);
        assert!(r1.body.contains("first"));

        let r2 = mock
            .send(TransportRequest {
                url: "https://example.com/b".to_string(),
                method: HttpMethod::Get,
                body: None,
                headers: vec![],
            })
            .await
            .expect("second response");
        assert!(r2.body.contains("second"));
        assert_eq!(mock.recorded_count(), 2);
    }

    #[tokio::test]
    async fn mock_transport_returns_exhausted_when_queue_empty() {
        let mock = Arc::new(MockTransport::new());
        let result = mock
            .send(TransportRequest {
                url: "https://example.com".to_string(),
                method: HttpMethod::Get,
                body: None,
                headers: vec![],
            })
            .await;
        assert!(matches!(result, Err(TransportError::MockExhausted)));
    }

    #[test]
    fn transport_error_converts_to_auth_error() {
        let net: AuthError = TransportError::Network("dns".to_string()).into();
        assert!(matches!(net, AuthError::NetworkError(_)));
        let status: AuthError = TransportError::HttpStatus {
            status: 401,
            body: "x".to_string(),
        }
        .into();
        assert!(matches!(status, AuthError::NetworkError(_)));
    }

    #[test]
    fn discord_login_headers_contains_required_fields() {
        let headers = discord_login_headers();
        let keys: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"User-Agent"));
        assert!(keys.contains(&"X-Super-Properties"));
    }

    #[test]
    fn json_body_serializes_struct() {
        #[derive(Serialize)]
        struct Login<'a> {
            login: &'a str,
            password: &'a str,
        }
        let v = json_body(&Login {
            login: "a@b.c",
            password: "secret",
        })
        .expect("serialize");
        assert_eq!(v["login"], "a@b.c");
        assert_eq!(v["password"], "secret");
    }
}
