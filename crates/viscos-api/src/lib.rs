//! `viscos-api` — Discord REST adapter (Faz 2.0 Dalga 1).
//!
//! ADR-0008: `twilight-http` ve `twilight-model` üzerine ince wrapper. Sıfır
//! custom transport kodu; twilight zaten rate-limit (`X-RateLimit-*`, 429
//! global), brotli/gzip decompression, SIMD JSON ve rustls-platform-verifier
//! TLS'ini sağlıyor.
//!
//! Faz 3.0'da `pub mod gateway` (twilight-gateway wrapper) eklenecek; Faz 4'te
//! cache adaptörü (`moka` + `rusqlite` + `foyer`) bu crate'in event'lerini
//! okuyacak.
//!
//! **Scope guard:** Bu crate sadece REST. Gateway, voice, DAVE E2EE Hepsi sonraki
//! fazlarda.

pub mod error;
pub mod rest;

pub use error::ApiError;
pub use rest::{ViscosHttp, ViscosHttpBuilder};
