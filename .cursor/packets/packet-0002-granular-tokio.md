# Implementation Packet — ADR-0002: Async Runtime — Tokio (granular features)

## Header

- **ADR:** ADR-0002
- **Başlık:** Async Runtime — Tokio (granular features)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0002](../../docs/DECISIONS.md#adr-0002-async-runtime--tokio-granular-features)
- **Önceki plan:** `phase-0.0-foundation.md` § 1 (Cargo workspace, granular tokio notu)

## Hedef faz worker

**Foundation worker, Faz 0.0, Dalga 1.** ADR-0001'den hemen sonra uygulanır; tüm `viscos-*` crate'lerinin `[dependencies]` kısmında `tokio` declare biçimini belirler.

## Uygulama adımları

1. **`[workspace.dependencies]` tablosunda `tokio` declare et** (kök `Cargo.toml`):
   ```toml
   tokio = { version = "1.40", default-features = false, features = [
       "rt-multi-thread", "macros", "sync", "time", "fs", "net", "io-util"
   ] }
   ```
   - `full` feature **YOK** (ADR-0002'deki gerekçe: process/signal/io-std derlenmiyor → %30-40 compile time kazancı, binary küçülmesi).

2. **Her crate'in `Cargo.toml`'unda `tokio` declare'unu workspace'den inherit et**:
   ```toml
   [dependencies]
   tokio = { workspace = true }
   ```
   - İhtiyaç varsa ek feature'lar crate-bazlı eklenir (örn. `viscos-api` Faz 2'de `signal` ekleyebilir).

3. **`viscos` binary main'inde `#[tokio::main]` macro'sunu kullan** (henüz Faz 0.0'da placeholder):
   ```rust
   #[tokio::main]
   async fn main() -> anyhow::Result<()> {
       tracing_subscriber::fmt::init();
       tracing::info!("Viscos başlatıldı");
       Ok(())
   }
   ```

4. **Doğrulama**:
   - `cargo tree -p viscos | grep tokio` → yalnızca declare edilen feature'lar görünüyor.
   - `cargo build --release --timings` → cold build 3 dakikanın altında (full feature ile 5 dakika).
   - `cargo bloat --release -p viscos` → `tokio` katkısı toplam binary'nin <%15'i.

## Kabul kriterleri

- ✅ Kök `Cargo.toml`'da `tokio` `default-features = false` + tam listelenen 7 feature ile declare edilmiş.
- ✅ Hiçbir crate `tokio = { version = "...", features = ["full"] }` veya `features = ["full"]` içermiyor.
- ✅ `cargo tree -p viscos` çıktısında `process`, `signal`, `io-std` feature'larına bağımlılık yok.
- ✅ `cargo build --release` süresi cold build'de < 3.5 dakika (EPYC 16-core, 32 GB RAM).
- ✅ `viscos` binary'si `tokio::main` ile ayağa kalkıyor (smoke test).

## Test stratejisi

- **Unit:** Her crate'in tokio feature setini test etmek için `#[cfg(test)] mod feature_check { #[test] fn uses_only_declared_features() { ... } }` (opsiyonel, basit grep yeterli).
- **Integration:** `cargo build --workspace --timings` çıktısı feature drift göstermemeli.
- **Manuel:**
  - `cargo build --release` süresini cold cache ile ölç (`cargo clean` + `cargo build --release`).
  - `cargo bloat --release -p viscos --crates -n 20` → tokio bileşenleri beklenen sırada.

## Sınır durumları ve riskler

- **Feature unutmak:** Crate'in ihtiyacı olan feature workspace'te yoksa compile error. Mitigation: Her crate'in `Cargo.toml`'unda `tokio = { workspace = true, features = ["..."] }` formunda ek feature ekleme.
- **`process` / `signal` ihtiyacı sonradan:** Faz 6 deep-link launcher (`process`), Unix-only runtime (`signal`) → ADR-0002 Consequences bölümünde "eklenir" denmiş, sorun yok. **Yeni feature eklemek yeni packet gerektirir**, doğrudan ekleme.
- **Feature unification:** Birden fazla crate farklı feature set isterse cargo birleştirir → yanlış feature'lar binary'ye girer. Mitigation: workspace'te tek noktada declare + her crate yalnızca ek feature ekler.
- **Runtime mixing:** `wry`/`tao` tokio featurelı; başka async runtime (smol, async-std) ile karıştırılırsa panik. ADR-0002'de reddedildi → sıkı kural.

## Review trigger'ları

- `tokio` major versiyon çıkarsa (1.x → 2.x).
- WebView backend değişirse (Servo gibi tokio-dışı bir runtime gerektiren).
- Compile time hedefi (>5 dakika) aşılırsa (`cargo build --release --timings` raporu).
- Discord API yeni transport gerektirirse (örn. HTTP/3, QUIC).

## Cross-references

- **ADR:** ADR-0001 (workspace), ADR-0006 (MSRV 1.85+).
- **Plan:** [`phase-0.0-foundation.md` § 1](../../.cursor/plans/phase-0.0-foundation.md), § 1 workspace dependencies.
- **Alternatifler:** `smol`, `glommio`, `async-std` — hepsi ADR-0002'de elendi.
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Hayır.** ADR-0002 net bir dependency seçim kararı, scope'u sınırlı, alternatifleri zaten elenmiş. AI yazar, CI doğrular (clippy + build süresi). İnsan yalnızca **feature ekleme** (örn. `process` Faz 6'da) durumunda bilgilendirilir.
