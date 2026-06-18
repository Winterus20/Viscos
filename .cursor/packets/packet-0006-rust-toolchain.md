# Implementation Packet — ADR-0006: Rust Toolchain 1.89 + Edition 2024

## Header

- **ADR:** ADR-0006
- **Başlık:** Toolchain 1.89 + Edition 2024 (twilight-rs uyumu)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0006](../../docs/DECISIONS.md#adr-0006-toolchain-189--edition-2024-twilight-rs-uyumu)
- **Önceki plan:** `phase-0.0-foundation.md` § 1.2 (`rust-toolchain.toml`)

## Hedef faz worker

**Foundation worker, Faz 0.0, Dalga 1.** ADR-0001 ile aynı PR'da uygulanır; toolchain dosyası workspace kurulumunun parçasıdır. **MSRV revizyonu (1.85 → 1.89) Haziran 2026'da yapıldı** — twilight-rs 0.17.x serisiyle uyum için zorunlu.

## Uygulama adımları

1. **`rust-toolchain.toml`** (workspace kökünde):
   ```toml
   [toolchain]
   channel = "stable"
   components = ["rustfmt", "clippy", "rust-analyzer"]
   profile = "minimal"
   ```

2. **`Cargo.toml` `[workspace.package]`** — `rust-version = "1.89"` (1.85 değil; ADR-0006 revizyonu).

3. **Tüm mevcut/gelecek crate'lerin `Cargo.toml`'unda edition/rust-version yok** (workspace'ten inherit). Sadece istisna: bir crate başka bir MSRV gerektiriyorsa, kendi `Cargo.toml`'unda override.

4. **CI entegrasyonu (ADR-0004)**:
   - `actions/setup-rust` step'inde `toolchain: stable` zaten yeterli (`rust-toolchain.toml` override eder).
   - `cargo check --workspace` MSRV uyumunu otomatik doğrular.

5. **Doğrulama**:
   - `rustc --version` → 1.89.0+ (stable kanalda).
   - `cargo check --workspace` → 0 uyarı.
   - ADR-0008 dependency'leri (`twilight-model = "0.17"`) `cargo check` ile compile oluyor (Faz 2 öncesi dry-run).

## Kabul kriterleri

- ✅ `rust-toolchain.toml` mevcut, `channel = "stable"`, 3 component listelemiş.
- ✅ Kök `Cargo.toml`'da `rust-version = "1.89"` ve `edition = "2024"`.
- ✅ `rustc --version` çıktısı 1.89 veya üstü.
- ✅ `cargo check --workspace` 0 hata, 0 uyarı.
- ✅ ADR-0008'deki twilight-rs 0.17 dependency'leri (ileride eklenecek) buildable (dry-run OK).
- ✅ Edition 2024 prelude özellikleri (`Future`, `IntoFuture`, `Box<[T]>: IntoIterator`) kullanılabilir.

## Test stratejisi

- **Unit:** `cargo check --workspace` MSRV uyumu.
- **Integration:** `cargo test --workspace` tüm crate'lerde.
- **Manuel:**
  - `rustc --version --verbose` → host + release channel doğrulama.
  - `rustup show` → active toolchain "stable" + 3 component.
  - `cargo +nightly check` (opsiyonel, nightly varsa) → nightly kanalında da buildable.

## Sınır durumları ve riskler

- **MSRV 1.85'e geri dönüş:** Viscos yeni proje, geri dönüş gerektirmez. Kullanıcılar kendi toolchain'lerini güncellemek zorunda (sadece contributor'lar, end-user rust kurmaz).
- **Cold build 5-7 dakika artışı:** `lto = "fat"` + Rust 1.89 + twilight derive'ları. Mitigation: sccache (ADR-0004).
- **Kütüphane MSRV çakışması:** Bir bağımlılık 1.89'dan yüksek MSRV isterse (örn. 1.92). Mitigation: İncele, gerekiyorsa toolchain bump + insan onayı.
- **Edition 2024 syntax drift:** AI agent'ın `cargo fix --edition` ile yanlışlıkla edition upgrade etmesi. Mitigation: `.cursorrules` Faz 0.5'te bu kuralı içerir; şu an insan review.

## Review trigger'ları

- Rust yeni edition çıkarsa (2027) — adoption değerlendirmesi.
- `twilight-rs` MSRV'yi 1.95'e çıkarırsa — toolchain bump.
- Başka bir bağımlılık (örn. `cef-rs`) MSRV 1.95+ isterse.
- `unsafe extern` syntax zorunluluğu nedeniyle Faz 1'de yoğun C-FFI'da hata artışı.

## Cross-references

- **ADR:** ADR-0001 (workspace package), ADR-0008 (twilight MSRV), ADR-0011 (keyring-core MSRV 1.88 — uyumlu).
- **Plan:** [`phase-0.0-foundation.md` § 1.2](../../.cursor/plans/phase-0.0-foundation.md).
- **Revizyon:** 1.80 → 1.85 → 1.89 (Haziran 2026, twilight-rs uyumu).
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Hayır.** Toolchain dosyası + 2 satır workspace package konfigürasyonu. CI doğrulaması otomatik. **İstisna:** Gelecekte MSRV bump (örn. 1.95) olursa ADR-0006'ya yeni revizyon eklenir, insan onayı gerekir.
