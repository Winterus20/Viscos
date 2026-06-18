# Implementation Packet — ADR-0007: Error Handling — thiserror (lib) + anyhow (app)

## Header

- **ADR:** ADR-0007
- **Başlık:** Error Handling — thiserror (lib) + anyhow (app)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0007](../../docs/DECISIONS.md#adr-0007-error-handling--thiserror-lib--anyhow-app)
- **Önceki plan:** `phase-0.0-foundation.md` § 2.2 (`error-types` todo)

## Hedef faz worker

**Foundation worker, Faz 0.0, Dalga 2.** `viscos-error` crate'i bu packet ile oluşturulur; `viscos-config` (ADR-0003) `ViscosError::Config` adaptörü için bu packet'e bağımlıdır — Dalga 2'de sıralı uygulanır (önce ADR-0007, sonra ADR-0003).

## Uygulama adımları

1. **`crates/viscos-error/Cargo.toml`**:
   - `thiserror = { workspace = true }`
   - `anyhow` **YOK** (library boundary'de sızıntı olmamalı).

2. **`crates/viscos-error/src/lib.rs`**:
   ```rust
   use thiserror::Error;

   #[derive(Error, Debug)]
   #[non_exhaustive]  // AI veya insan yeni variant ekleyebilir → breaking change yok
   pub enum ViscosError {
       #[error("config error: {0}")]
       Config(#[from] config::ConfigError),

       #[error("io error: {0}")]
       Io(#[from] std::io::Error),

       #[error("not yet implemented: {0}")]
       Unimplemented(&'static str),

       // ... ileride eklenecek variant'lar:
       // Auth(#[from] viscos_auth::AuthError),    // Faz 2'de
       // Api(ApiError),                            // Faz 2/3'te
       // Cache(#[from] viscos_cache::CacheError),  // Faz 4'te
   }

   pub type Result<T> = std::result::Result<T, ViscosError>;
   ```

3. **`viscos` binary `main.rs`** (application boundary):
   ```rust
   use anyhow::{Context, Result};

   fn main() -> Result<()> {
       tracing_subscriber::fmt::init();
       viscos_config::load()
           .context("config yüklenemedi")?;
       Ok(())
   }
   ```

4. **Public API kuralı** (workspace-wide):
   - Tüm library fonksiyonları `Result<T, ViscosError>` döner.
   - Yalnızca `viscos` binary'sinin `main` fonksiyonu `anyhow::Result<()>` kullanır.
   - `.context()` yalnızca main/glue code'da.

5. **Doğrulama**:
   - `cargo build --workspace` → 0 hata.
   - `grep -r "anyhow::Error" crates/` → 0 sonuç (library boundary kuralı).
   - `grep -r "thiserror" crates/viscos-error/src/` → `#[derive(Error)]` görünüyor.

## Kabul kriterleri

- ✅ `viscos-error` crate mevcut, `ViscosError` enum `#[non_exhaustive]`.
- ✅ `Result<T>` type alias mevcut.
- ✅ `From<config::ConfigError>` ve `From<std::io::Error>` impl'leri var.
- ✅ `anyhow::Error` variant **YOK**.
- ✅ `viscos` binary'si `anyhow::Result<()>` kullanıyor.
- ✅ Hiçbir library crate `anyhow` import etmiyor (`grep` ile doğrula).
- ✅ Yeni variant eklenirse (örn. Faz 2'de `Auth`) breaking change olmaz (`#[non_exhaustive]` sayesinde).

## Test stratejisi

- **Unit:**
  - `tests/error_types.rs`: Her variant için display fmt + Error trait testi.
  - `#[test] fn config_variant_propagates() { let e: ViscosError = config::ConfigError::NotFound("x".into()).into(); ... }`
- **Integration:**
  - `tests/non_exhaustive.rs`: `let _: ViscosError = match_failure(); match e { _ => unreachable!() }` → derlenmeli (`_` ile yakalanabilir, tüm variant'lar yazılamaz).
- **Manuel:**
  - `cargo doc --workspace` → rustdoc'ta `ViscosError` tüm variant'ları görünüyor.
  - `grep -rn "anyhow" crates/ --include="*.rs"` → yalnızca `viscos/src/main.rs` ve test dosyalarında.

## Sınır durumları ve riskler

- **Variant ekleme kuralı:** AI yeni variant eklerken `#[non_exhaustive]` korunmalı. CI: `cargo semver-checks` minor bump'ta uyarı verir.
- **Library sızıntısı:** `viscos-config` veya başka crate `anyhow` import ederse library boundary ihlali. Mitigation: `.cursorrules` Faz 0.5'te bu kural + grep tabanlı deny rule.
- **thiserror/anyhow versiyon drift:** Major versiyon çıkarsa (thiserror 2.x) migration packet gerekir.
- **Çift dönüşüm:** `ViscosError` → `anyhow::Error` → log. Internal glue'larında küçük boilerplate. ADR-0007'de kabul edildi.

## Review trigger'ları

- `thiserror` veya `anyhow` 1.0+ major versiyon çıkarsa.
- Yeni error kaynağı (örn. yeni crate) `ViscosError`'a variant olarak eklenirse (örn. Faz 2 Auth, Faz 3 Gateway, Faz 4 Cache).
- `eyre` veya `miette` endüstri standardı olursa (şu an değil).

## Cross-references

- **ADR:** ADR-0003 (config `ConfigError` adaptörü), ADR-0011 (`AuthError` ileride eklenecek).
- **Plan:** [`phase-0.0-foundation.md` § 2.2](../../.cursor/plans/phase-0.0-foundation.md).
- **AI Workflow:** `viscos_index.md` Bölüm 4.6 — `unwrap()` in production yasak.
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Hayır.** Standart endüstri pattern'i. AI yazar, CI doğrular (build + grep). **İstisna:** Yeni `ViscosError` variant'ı breaking sayılmaz (`#[non_exhaustive]`), ancak **public API'nin hata tipini değiştirmek** (örn. `Result<T, String>` döndürmek) mimari karar — insan onayı gerekir.
