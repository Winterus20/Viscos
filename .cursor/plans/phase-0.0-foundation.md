---
name: Phase 0.0 — Foundation
overview: Cargo workspace kurulumu (1.85, edition 2024, granular tokio), GitHub Actions CI (7 job: fmt/clippy/nextest/build-sccache/audit/deny/geiger + size gate), temel logging/error/config altyapısı (config-rs, thiserror+anyhow, tracing), ilk derlenebilir boş binary. AI workflow henüz yok, sadece iskelet.
isProject: false
todos:
  - id: workspace-init
    content: Cargo workspace oluştur (crates/* dizin yapısı, edition 2024, rust-version 1.85)
    status: pending
  - id: rust-toolchain
    content: rust-toolchain.toml (1.85+, stable channel, rustfmt + clippy + rust-analyzer)
    status: pending
  - id: tracing-init
    content: tracing + tracing-subscriber kurulumu (env-filter, json, fmt, tracing-log)
    status: pending
  - id: tracing-appender
    content: tracing-appender bağımlılığı (Faz 1+'ta crash log dosyası için)
    status: pending
  - id: error-types
    content: thiserror + anyhow crate'leri ve viscos-error helper (#[non_exhaustive], anyhow sızıntısı yok)
    status: pending
  - id: config-system
    content: config-rs (config 0.14) ile TOML + env var layered loading
    status: pending
  - id: deny-toml
    content: .cargo/deny.toml (license allowlist, source kısıtı, ban kuralları)
    status: pending
  - id: ci-workflow
    content: GitHub Actions: 7 job (fmt, clippy, nextest, build+sccache, audit, deny, geiger) + size gate (25 MB)
    status: pending
  - id: empty-binary
    content: viscos (binary) crate — boş main, derlenebilir
    status: pending
  - id: smoke-test
    content: İlk smoke test: cargo run çalışıyor mu
    status: pending
---

# Phase 0.0 — Foundation

> **Süre:** 1–2 hafta
> **Hedef:** Cargo workspace, CI, temel altyapı. İlk derlenebilir boş binary.
> **Sonraki faz:** [`phase-0.5-ai-workflow-setup.md`](./phase-0.5-ai-workflow-setup.md)

---

## 1. Cargo Workspace

### 1.1 Kök `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/viscos-core",
    "crates/viscos-config",
    "crates/viscos-error",
    "crates/viscos-log",
    "crates/viscos",
]

# Henüz oluşmayan crate'ler (Faz 1+) yorum satırı olarak:
#    "crates/viscos-api",
#    "crates/viscos-cache",
#    "crates/viscos-media",
#    "crates/viscos-shell",
#    "crates/viscos-webview",
#    "crates/viscos-watchdog",
#    "crates/viscos-ipc",
#    "crates/viscos-auth",

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"           # Edition 2024 baseline (stable Feb 2025)
license = "GPL-3.0"
authors = ["Viscos Contributors"]
repository = "https://github.com/viscos/viscos"

[workspace.dependencies]
# Async runtime — GRANULAR (full değil)
# "full" gereksiz driver'ları (process, signal, io-std) çeker, compile time'ı %30-40
# artırır ve binary'yi büyütür. Her crate ihtiyacı kadar feature kullanır.
tokio = { version = "1.40", default-features = false, features = [
    "rt-multi-thread",
    "macros",
    "sync",
    "time",
    "fs",
    "net",
    "io-util",
] }

# Logging — async/structured
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json", "fmt", "tracing-log"] }
tracing-appender = "0.2"        # Faz 1+ crash log dosyası

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Config — config-rs (12-factor standard, aktif bakım, MIT/Apache)
# NOT figment: figment May 2024'ten beri stale, topluluk fork'u figment2 çıktı.
# config-rs layered, profile, dotenv, multi-format (TOML/JSON/YAML/INI/RON) destekler.
# Read-only (yazma yok) — Viscos için OK: kullanıcı ayarları DB'de, app config salt okunur.
config = { version = "0.14", default-features = false, features = ["toml", "convert-case"] }
serde = { version = "1.0", features = ["derive"] }

[profile.release]
opt-level = 3
lto = "fat"                    # thin yerine fat — 15-25 MB binary hedefi için kritik
codegen-units = 1
strip = true
panic = "abort"                # unwinding maliyeti yok, binary küçülür
```

### 1.2 `rust-toolchain.toml`

```toml
[toolchain]
# Edition 2024 + modern cargo resolver için minimum 1.85
channel = "stable"
components = ["rustfmt", "clippy", "rust-analyzer"]
profile = "minimal"
```

**Neden 1.85?**
- Edition 2024 stabil (Şubat 2025)
- `Future` / `IntoFuture` prelude'da (async ergonomics)
- `unsafe extern` zorunluluğu (C-FFI typo koruması)
- Cargo `rust-version`-aware resolver (default-features = false davranışı düzeltildi)

### 1.3 Dizin Yapısı (Faz 0.0'da oluşturulan)

```
viscos/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
├── LICENSE
├── .gitignore
├── .github/
│   └── workflows/
│       └── ci.yml
├── crates/
│   ├── viscos-core/         # types, traits, events (no I/O)
│   ├── viscos-config/       # config-rs wrapper (TOML + env layered)
│   ├── viscos-error/        # thiserror enum + #[non_exhaustive]
│   ├── viscos-log/          # tracing init
│   └── viscos/              # ana binary
└── config/
    ├── default.toml         # git'te
    └── local.toml.example   # git'te; local.toml gitignore
```

---

## 2. Crate Detayları

### 2.1 `viscos-core`

Domain types ve trait'ler. **I/O yapmaz.** Sadece std + serde.

```rust
// crates/viscos-core/src/lib.rs
pub mod types;
pub mod events;
pub mod traits;
```

**Bu fazda:**
- Boş modül yapısı
- `pub struct AppContext` placeholder (Faz 1'de doldurulacak)
- `pub trait Backend: Send + Sync` placeholder

### 2.2 `viscos-error`

```rust
// crates/viscos-error/src/lib.rs
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
    // NOT: anyhow::Error variant YOK. Library sınırında typed hata döndür;
    // anyhow yalnızca application boundary (main, glue code) içinde.
    // Gerekçe: tüketicinin match edebilmesi için somut tip kalmalı.
}

pub type Result<T> = std::result::Result<T, ViscosError>;
```

**`anyhow` nerede kullanılır:** yalnızca `viscos` binary'sinin `main`'inde ve internal glue katmanında. Kütüphane crate'lerinin public API'si `ViscosError` döner.

### 2.3 `viscos-log`

```rust
// crates/viscos-log/src/lib.rs
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("viscos=info,wry=warn"));
    // NOT: default olarak sessiz (info). Faz 1+'ta GDI leak debug'unda
    // `RUST_LOG=viscos=debug,wry=debug` ile override edilir.

    tracing_subscriber::registry()
        .with(filter)
        // tracing-log feature: log facade'i kullanan crate'lerden (wry, tao,
        // winapi wrapper'ları) gelen mesajları da yakalar.
        .with(fmt::layer().with_target(true))
        .init();
}
```

**Faz 1+'ta eklenecek (Faz 0.0'da değil):** `tracing-appender` ile non-blocking dosyaya yazma (24 saat soak test'inde log kaybı olmasın).

### 2.4 `viscos-config`

```rust
// crates/viscos-config/src/lib.rs
use config::{Config as ConfigRs, File, Environment};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String, // "json" veya "pretty"
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        ConfigRs::builder()
            // 1. Varsayılan: config/default.toml (içerikte gömülü fallback)
            .add_source(File::with_name("config/default").required(false))
            // 2. Platforma özel override: config/local.toml (gitignore)
            .add_source(File::with_name("config/local").required(false))
            // 3. Env var override: VISCOS_APP__NAME=foo, VISCOS_LOGGING__LEVEL=debug
            //    __ ayracı ile nested path; convert-case crate ile kebab-case <-> snake_case dönüşür.
            .add_source(
                Environment::with_prefix("VISCOS")
                    .separator("__")
                    .try_parsing(true)
            )
            .build()?
            .try_deserialize()
    }
}
```

**Neden `figment` değil `config-rs`?**
- `figment` Mayıs 2024'ten beri stale, topluluk fork'u `figment2` çıktı.
- `config-rs` aktif bakım, 12-factor standardı, daha geniş format desteği.
- 4 yıllık AI-agent projesi boyunca upstream güncellemesi alabilmek kritik.

**Trade-off:** `config-rs` read-only (yazma yok). Viscos için OK: kullanıcı ayarları SQLite'ta, app config salt okunur.

### 2.5 `viscos` (Binary)

```rust
// crates/viscos/src/main.rs
use viscos_log::init_logging;
use viscos_config::Config;

fn main() -> anyhow::Result<()> {
    init_logging();
    let config = Config::load()?;
    tracing::info!("Viscos starting up");
    tracing::info!(?config, "config loaded");
    Ok(())
}
```

---

## 3. Config Dosyaları

### `config/default.toml` (git'te, varsayılan)

```toml
[app]
name = "Viscos"
data_dir = "%APPDATA%/Viscos"

[logging]
level = "info"
format = "pretty"
```

### `config/local.toml.example` (git'te, şablon)

```toml
# Geliştirici override'ları için local.toml şablonu.
# Kullanım: cp config/local.toml.example config/local.toml
# config/local.toml .gitignore'dadır, kişisel ayar dosyasıdır.
[logging]
level = "debug"
# format = "json"  # production loglar için
```

### `.gitignore`

```
/target
**/*.rs.bk
Cargo.lock.bak
*.pdb
*.swp
.DS_Store

# Kişisel config override'ı (config/local.toml.example şablon olarak git'te)
config/local.toml

# AI debug çıktıları
.cursor/agent-tools/
```

### Env var override örnekleri

```bash
# Tüm log seviyesini debug yap
export VISCOS_LOGGING__LEVEL=debug
# Production log formatı
export VISCOS_LOGGING__FORMAT=json
# Custom data dizini
export VISCOS_APP__DATA_DIR=/tmp/viscos-dev
```

`__` separator config-rs'in nested key ayracı; `convert-case` feature otomatik kebab-case ↔ snake_case dönüşümü yapar.

---

## 4. GitHub Actions CI

> 2026 best-practice'i: **matrix job'lar** + **2 katmanlı cache** (registry + compiler artifact) + **security policy gate** (audit + deny) + **binary budget enforcement**.

### 4.1 `.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
  # Haftalık güvenlik taraması (yeni advisory'ler için)
  schedule:
    - cron: "0 6 * * 1"  # Pazartesi 06:00 UTC

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-Dwarnings"  # CI'da tüm warning'ler hard error

jobs:
  # ─── 1. Format & lint (hızlı geri bildirim) ──────────────────
  fmt:
    name: Format
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt }
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy (pedantic + nursery)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy }
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace --all-targets -- -D warnings

  # ─── 2. Test (cargo nextest, paralel + izole) ────────────────
  test:
    name: Test (nextest)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@nextest
      - uses: Swatinem/rust-cache@v2
      - name: Compile check (--locked, registry cache kullanır)
        run: cargo check --workspace --all-features --locked
      - name: Run tests
        run: cargo nextest run --workspace --all-features --retries 2
      # Doc test'ler nextest kapsamında değil → ayrı adım
      - name: Doc tests
        run: cargo test --workspace --doc

  # ─── 3. Release build (sccache ile compiler-artifact cache) ──
  build:
    name: Release build
    runs-on: windows-latest
    needs: [fmt, clippy, test]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Setup sccache
        uses: mozilla-actions/sccache-action@v0.0.9
        env:
          SCCACHE_GHA_ENABLED: "true"
      - name: Build
        env:
          RUSTC_WRAPPER: sccache
          CARGO_INCREMENTAL: "0"
        run: cargo build --workspace --release --locked

      # Binary bütçesi koruması (15-25 MB hedefi)
      - name: Check binary size
        shell: bash
        run: |
          SIZE=$(stat -c%s target/release/viscos.exe)
          MAX=$((25 * 1024 * 1024))  # 25 MB
          echo "Binary size: $SIZE bytes (max $MAX)"
          if [ "$SIZE" -gt "$MAX" ]; then
            echo "::error::Binary $SIZE bayt > 25 MB hedefini aştı"
            exit 1
          fi

  # ─── 4. Security & policy (haftalık + PR) ────────────────────
  audit:
    name: Security audit
    runs-on: ubuntu-latest  # Hız için (cargo-audit OS-agnostic)
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/install-action@cargo-audit
      - run: cargo audit

  deny:
    name: Cargo deny (license + source + ban)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check
          arguments: --all-features

  # ─── 5. Unsafe kullanım raporu (insan review yardımcısı) ───
  geiger:
    name: Unsafe report
    runs-on: ubuntu-latest
    continue-on-error: true  # Bilgi amaçlı, fail etmesin
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/install-action@cargo-geiger
      - run: cargo geiger --all-features
```

### 4.2 Neden bu işler?

| İş | Önceki planda | Şimdi | Neden |
|----|---------------|-------|-------|
| **fmt** | Var | Aynı + hızlı geri bildirim | İlk job, 30s'de biter |
| **clippy** | Var | Aynı | `RUSTFLAGS=-Dwarnings` ile strict |
| **test** | `cargo test` | **`cargo nextest`** | Paralel + izole, ~2-3x hızlı; `--retries 2` flaky network test'leri için |
| **build** | Tek job, cache'siz | **sccache + rust-cache** | Compiler-artifact cache: incremental build'lerde 30-50% hız |
| **size check** | ❌ Yok | **25 MB gate** | Binary bütçesi koruması (hedef metrikler) |
| **audit** | ❌ Yok | **Haftalık + PR** | Yeni RustSec advisory'lerini erken yakala |
| **deny** | ❌ Yok | **Her PR** | GPL-3.0 projesi: tüm transitive dependency'lerin lisans uyumunu garanti et |
| **geiger** | ❌ Yok | **PR** (warn-only) | `unsafe` kullanımını görünür kıl, code review'da zorla |

### 4.3 Cargo Cache Stratejisi (2 Katman)

1. **`Swatinem/rust-cache@v2`**: `~/.cargo/registry` + `target/` (tek blob, 10 GB limit, 145 MiB/s)
2. **`sccache`**: `RUSTC_WRAPPER` ile rustc interception, **object-level cache** (sadece gerekli artifact'ler, GH cache API'ye sızıyor)
- Birlikte: registry miss → Swatinem, hit → sccache devreye girer. **Kombine en hızlı.**

### 4.4 Neden Windows runner?

Viscos Windows-only (v1). Cross-platform Faz v2'de. v1'de `windows-latest` zorunlu.

**Gelecek iyileştirme (Faz 4+):** Depot / WarpBuild managed runner (AMD EPYC Genoa, 2-3x hızlı, per-second billing). v1 başlangıcı için `windows-latest` yeterli; build süreleri 10+ dakikaya çıkınca geçiş yapılır.

### 4.5 `.cargo/deny.toml` Örneği

```toml
[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked = "warn"

[licenses]
allow = [
    "MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "CC0-1.0",
    "Unicode-DFS-2016", "Unicode-3.0",
    # GPL/AGPL/LGPL — elle inceleme sonrası allow edilir (varsayılan deny)
]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "deny"
deny = []
# skip = []  # Bilinen geçici exception'lar (issue linki ile)

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

---

## 5. Test Stratejisi (Faz 0.0)

| Crate | Test Tipi | Kapsam | Runner |
|-------|-----------|--------|--------|
| `viscos-core` | (yok, henüz logic yok) | — | — |
| `viscos-error` | unit | Display, From conversions, `#[non_exhaustive]` uyumu | `cargo nextest` |
| `viscos-log` | integration | Init birden fazla kez çağrılırsa panic etmemeli | `cargo nextest` |
| `viscos-config` | unit | Default load, env override, malformed TOML hata, layered merge | `cargo nextest` |
| `viscos` | smoke | `cargo run` exit code 0, log satırı görünür | shell |

**Test komutu:** `cargo nextest run --workspace --all-features --retries 2`
**Doc test komutu:** `cargo test --workspace --doc` (nextest doc test çalıştırmaz)

**Pre-commit (yerel):** AI agent commit öncesi `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` çalıştırmalı (CI yükünü azaltır).

---

## 6. Kabul Kriterleri (Definition of Done)

- [ ] `cargo build --workspace` başarılı
- [ ] `cargo nextest run --workspace --all-features` tüm testler geçer
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0 warning
- [ ] `cargo fmt --all -- --check` temiz
- [ ] `cargo audit` 0 vulnerability (veya justifiye edilmiş ignore + issue linki)
- [ ] `cargo deny check` temiz (license + source + ban)
- [ ] `target/release/viscos.exe` ≤ 25 MB (size check job geçer)
- [ ] CI'da tüm 7 job (fmt, clippy, test, build, audit, deny, geiger) yeşil
- [ ] `cargo run -p viscos` exit code 0, log satırı görünür
- [ ] `Cargo.lock` commit'lendi
- [ ] `README.md` "kurulum" bölümündeki adımlar lokal'de çalışıyor

---

## 7. Karar Noktaları (Faz 0.0 Sonu)

> 🔵 **İNSAN:** Clippy lint seviyesi ne olsun?
> - ✅ **Önerilen: Pedantic + nursery + CI deny** (her crate `#![warn(clippy::pedantic)] #![warn(clippy::nursery)] #![deny(clippy::all)]` + CI `cargo clippy -- -D warnings`)
>   - Neden: AI agent %100 kod yazıyor → pedantic lint'ler AI'a rehber olur, tekrar eden hataları commit öncesi yakalar
>   - Trade-off: Ara sıra false-positive → `#[allow(...)]` ile justifiye ederek bastır, her `allow` PR review'da kontrol edilir
> - Seçenek A: `#![deny(clippy::all)]` (sıkı, pedantic yok)
> - Seçenek B: Sadece CI'da `cargo clippy -- -D warnings` (gevşek, default lint'ler)

> 🔵 **İNSAN:** Default features seti ne olsun?
> - ✅ **Önerilen: Granular** (rt-multi-thread + macros + sync + time + fs + net + io-util)
>   - Neden: `full` gereksiz driver'ları (process, signal, io-std) çeker, compile time'ı %30-40 artırır, binary büyütür
>   - Viscos ilk başta ihtiyaç duymadığı feature'ları: `process` (Faz 6 deep-link launcher), `signal` (Unix-only), `io-std` (GUI app gereksiz) — ihtiyaç olunca eklenir
> - Seçenek A: `features = ["full"]` (hızlı başlangıç, maliyeti düşük iterasyon hızı)
> - Seçenek B: Her crate kendi minimal setini seçsin (en hızlı, en karmaşık)

> 🔵 **İNSAN:** Config crate: `config-rs` (önerilen) mi, `figment2` (figment'in fork'u) mi, eski `figment` mi?
> - ✅ **Önerilen: `config-rs`**
>   - Neden: figment May 2024'ten beri stale; 4 yıllık proje boyunca upstream güncellemesi alamamak technical debt olur
>   - `config-rs` aktif bakım, 12-factor standardı, daha geniş format desteği (TOML/JSON/YAML/INI/RON/JSON5)
>   - Trade-off: write-back desteklemiyor (Viscos için OK: kullanıcı ayarları DB'de)
> - Seçenek A: `figment2` (drop-in replacement, figment API'si bire bir, risk minimum)
> - Seçenek B: Eski `figment` (mevcut plan, **önerilmez**)

> 🔵 **İNSAN:** CI runner: GitHub-hosted mı, Depot/WarpBuild managed mı, self-hosted mı?
> - ✅ **Önerilen: GitHub-hosted (`windows-latest`) Faz 0.0–2; gerektiğinde Depot'a geçiş**
>   - Neden: v1 başlangıcında CI dakika kullanımı düşük olur; GH-hosted yeterli. WebView2 build'leri ağırlaşınca (Faz 4+) Depot 2-3x hız + %50 maliyet düşüşü sağlar
>   - Self-hosted: Mart 2026'dan beri GH $0.002/dakika platform fee + bakım yükü → ancak aylık 3000+ dakika koşunca kârlı
> - Seçenek A: Hemen Depot (hız, maliyet avantajı, Faz 0.0'dan itibaren)
> - Seçenek B: Self-hosted (uzun vadede ucuz, operasyonel yük var)

> 🔵 **İNSAN:** LTO stratejisi: `lto = "thin"` (mevcut plan) mı, `lto = "fat"` (önerilen) mi?
> - ✅ **Önerilen: `lto = "fat"`**
>   - Neden: 15-25 MB binary hedefi var; fat LTO daha agresif dead-code elimination yapar, 1-3 MB ek kazanç + runtime %2-5 performans
>   - Trade-off: Build süresi %20-30 artar (sccache ile telafi edilir), debug zorlaşır (release için OK)
> - Seçenek A: `lto = "thin"` (hızlı build, daha büyük binary)
> - Seçenek B: `lto = "off"` (default, debug için iyi, release için kötü)

> 🔵 **İNSAN:** `panic = "abort"` kabul edilsin mi? (release profile)
> - ✅ **Önerilen: Evet, `panic = "abort"`**
>   - Neden: GUI app + Discord client; panic sonrası process yeniden başlatılır (watchdog Faz 1'de ekleniyor). Unwinding maliyeti gereksiz.
>   - Trade-off: Test'lerde unwind bekleyen assert macros'lar etkilenir → release'te abort, test'lerde default unwind (profile otomatik)
> - Seçenek A: `panic = "unwind"` (default, Rust geleneği)

---

## 8. Riskler ve Azaltma

| Risk | Azaltma |
|------|---------|
| Workspace compile time yavaş | `sccache` (compiler-artifact) + `Swatinem/rust-cache` (registry) — 2 katmanlı cache; granular tokio features |
| Clippy false-positive (pedantic + nursery) | `#[allow(...)]` sadece justifiye edilmiş yerlerde + PR review'da zorunlu kontrol |
| Config merge karmaşıklığı | `config-rs` testleri, env var dokümantasyonu (VISCOS_*__ nested __ separator) |
| Logging spam | EnvFilter default `info`, debug için `RUST_LOG=viscos=debug`; `tracing-log` ile `log` facade'ini de yakala |
| Stale dependency (örn. figment vakası) | `cargo audit` haftalık + `cargo deny` her PR; yeni dependency ekleme PR'ları insan onayı zorunlu (master index'te tanımlı) |
| Binary bütçesi aşımı (25 MB) | CI size check job: 25 MB üstünde fail; crate-level `[profile]` ince ayarı |
| `unsafe` birikimi | `cargo geiger` PR'da (warn-only); `unsafe_op_in_unsafe_fn` deny; WinAPI yoğunluğu biliniyor |
| LDFLAG / native dep sorunu (Windows) | Faz 1+'ta `windows-rs`, `webview2-com` gelince; Faz 0.0'da sadece saf Rust crate'ler → sorun riski düşük |

---

## 9. Çıkış → Faz 0.5

Bu faz tamamlandığında:
- Workspace derlenebilir (1.85, edition 2024, granular tokio)
- CI yeşil: 7 job (fmt, clippy, test (nextest), build (sccache), audit, deny, geiger)
- Temel altyapı (logging, error, config) var
- Tüm teknik kararlar `docs/DECISIONS.md` (ADR) altında kayıtlı
- Binary bütçesi (≤ 25 MB) CI'da korunuyor

Faz 0.5 → AI workflow kurulumu: `.cursorrules`, task template'leri, PR template, AI-validation CI.
