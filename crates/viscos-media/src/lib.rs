//! `viscos-media` — content-addressable hybrid media cache with AES-GCM encryption.
//!
//! Faz 4.0 (ADR-0010 §B): the third cache tier — opaque encrypted blobs
//! (images, video, voice waveforms) keyed by Discord attachment snowflake.
//! Tier composition:
//!
//! - **foyer HybridCache** — 32 MB RAM blob cache + 10 GB on-disk tier,
//!   content-addressable (foyer chooses key-dispatch internally).
//! - **AES-256-GCM** — every blob encrypted at-rest with a per-install key
//!   held in the OS keyring (Windows Credential Manager on Win10+). Nonce
//!   unique per blob, generated from `OsRng`.
//! - **moka URL metadata** — signed-URL TTL index for the CDN refresh worker
//!   (`CdnRefreshWorker` — Dalga 2 stub).
//!
//! **Note:** foyer APIs evolved across the 0.x series. The exact
//! `HybridCacheBuilder` signature may need adjustment per foyer release;
//! see `cache.rs` for the live config.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod cache;
pub mod encryption;
pub mod refresh;

pub use cache::{CdnUrlMeta, EncryptedMediaBlob, MediaCache, MediaError};
pub use encryption::MediaKey;
pub use refresh::CdnRefreshWorker;
