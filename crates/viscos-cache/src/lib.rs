//! `viscos-cache` — persistent + in-memory cache (Faz 4.0 Dalga 1, ADR-0010).
//!
//! 3-katmanlı cache mimarisinin ilk iki katmanı:
//!
//! - SQLite WAL (rusqlite + r2d2 pool) — guild/channel/messages/members/attachments
//!   relational metadata için. WAL mode + `synchronous=NORMAL` + 256MB mmap
//!   + 64MB journal_size_limit. Refinery ile versiyonlu migration.
//! - moka in-memory — sıcak mesaj lookup (message_id → `Message`),
//!   TinyLFU admission policy + per-entry TTL.
//!
//! **Faz 4.0 Dalga 1 Polish (PR-3):**
//! - [`facade::Cache`] — unified entry point with config-driven path resolution.
//! - [`repository::MessageRepository`] / [`repository::SqliteMessageRepository`] —
//!   pluggable repository trait + SQLite v1 impl.
//! - [`viscos_config::CacheConfig`] integration — cache path resolved via
//!   `viscos_config::CacheConfig` (no hard-coded paths).
//!
//! **Dalga 2 stub'ları** (`CacheTiers::auto_tune`, `telemetry.rs`) sadece TODO
//! olarak bırakıldı; Faz 1.5 telemetry backend hazır olunca doldurulacak.
//!
//! **Dalga 3 kapsamı:** `viscos-media` crate foyer katmanını + AES-GCM encryption'ı
//! tutar; bu crate sadece relational metadata + RAM cache ile sınırlıdır.

#![deny(unsafe_code)]
#![allow(missing_docs)] // embed_migrations! emits internal items lacking rustdoc

pub mod cache;
pub mod db;
pub mod error;
pub mod facade;
pub mod repository;
pub mod tier;

#[cfg(test)]
mod facade_tests;

pub use cache::{Message, MessageCache};
pub use db::Db;
pub use error::CacheError;
pub use facade::{Cache, Result as CacheResult};
pub use repository::{
    Channel, ChannelRepository, Guild, GuildRepository, MessageRepository, SqliteChannelRepository,
    SqliteGuildRepository, SqliteMessageRepository,
};
pub use tier::{CacheTiers, TelemetryStats};

refinery::embed_migrations!("migrations");
