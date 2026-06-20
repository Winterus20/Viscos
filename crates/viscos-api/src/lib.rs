//! `viscos-api` — Discord REST + Gateway adaptörü (Faz 2.0 + Faz 3.0).
//!
//! ADR-0008: `twilight-http` + `twilight-model` + `twilight-gateway` üzerine
//! ince wrapper. Sıfır custom transport kodu; twilight zaten rate-limit
//! (`X-RateLimit-*`, 429 global), brotli/gzip decompression, SIMD JSON,
//! rustls-platform-verifier TLS, zstd-stream framing, session resume,
//! reconnect + exponential backoff, jittered heartbeat — hepsini sağlıyor.
//!
//! Faz 3.0 + Faz 4.0 cache adaptörü (`moka` + `rusqlite` + `foyer`) bu
//! crate'in event'lerini [`gateway_cache_bridge`] üzerinden okur; MVP-2
//! (Time-to-Read/Write) burada tamamlanır.
//!
//! **Scope guard:** Bu crate REST + Gateway + cache bridge. Voice, DAVE E2EE,
//! presence update → sonraki fazlar.

pub mod error;
pub mod events;
pub mod gateway;
pub mod gateway_cache_bridge;
pub mod rest;

pub use error::ApiError;
pub use events::{GatewayEvent, LifecycleEvent};
pub use gateway::ViscosGateway;
pub use gateway_cache_bridge::{BridgeError, GatewayCacheBridge};
pub use rest::{ViscosHttp, ViscosHttpBuilder};
