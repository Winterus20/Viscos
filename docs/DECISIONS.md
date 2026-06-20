# Viscos — Mimari Karar Kayıtları (ADR)

> **Format:** Michael Nygard ADR şablonu (Context → Decision → Consequences).
> **Durum:** ✅ Accepted = kabul edildi ve uygulanıyor · 🟡 Proposed = tartışılıyor · ❌ Superseded = geçersiz (yeni ADR'ye yönlendirir).
> **AI workflow:** AI yeni ADR **öneremez** (Hard Limit — mimari karar). İnsan yazar + review eder. AI PR ile kod değişikliği yaparken ilgili ADR'ye referans vermek **zorunlu**.

---

## ADR-0001: Cargo Workspace (Bazel/Buck2 değil)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 0.0

### Context

Viscos'un 11 crate'i (core, api, cache, media, shell, webview, watchdog, ipc, auth, config, viscos) tek bir Rust monorepo'da. Build system seçimi: Cargo, Bazel (+ rules_rust + crate_universe), Buck2.

### Decision

**Cargo workspace** (`[workspace]` root manifest, ortak `Cargo.lock`, ortak `target/`, `path` + `[workspace.dependencies]`).

### Consequences

**Olumlu:**
- Sıfır ek konfigürasyon; IDE (rust-analyzer) desteği problemsiz; topluluk standardı.
- Tek `Cargo.lock` → dependency version drift yok.
- Local `target/` cache incremental build için yeterli; CI'da `sccache` ile paylaşılabilir.

**Olumsuz / Kabul edilen riskler:**
- Remote execution / REAPI yok. Build cache sadece local + sccache üzerinden CI arasında paylaşılır.
- Polyglot (gelecekte TS frontend, Python tooling) eklenirse Bazel değerlendirmesi gerekir. v1 Rust-only olduğu için bu risk düşük.

**Gözden geçirme tetikleyicileri:**
- Crate sayısı > 30 olursa
- 15+ dakikalık full rebuild süreleri kronikleşirse
- Polyglot monorepo gereksinimi doğarsa

---

## ADR-0002: Async Runtime — Tokio (granular features)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 0.0

### Context

Async runtime seçimi: `tokio`, `async-std`, `smol`, `glommio`. Viscos Windows-only v1; `wry`/`tao` tokio'ya bağımlı; Discord Gateway WebSocket + REST + WebView2 IPC async I/O yoğun.

### Decision

**Tokio** (versiyon 1.40+) — **granular features** ile:

```toml
tokio = { version = "1.40", default-features = false, features = [
    "rt-multi-thread", "macros", "sync", "time", "fs", "net", "io-util"
] }
```

`full` feature **yok** (gerekçe aşağıda).

### Consequences

**Olumlu:**
- Compile time %30-40 iyileşme (ölçüm: `full` ≈ 5 dk cold build, granular ≈ 3 dk, EPYC 16-core).
- Binary daha küçük (`process`, `signal`, `io-std` driver'ları derlenmiyor).
- Ekosistem standardı: `reqwest`, `hyper`, `axum`, `wry`, `tao` hepsi tokio ile çalışır.
- Disqord/twilight/serenity benzeri tüm Discord kütüphaneleri tokio featurelı.

**Olumsuz / Kabul edilen riskler:**
- Her crate kendi feature setini declare etmeli → boilerplate. `[workspace.dependencies]` unified version sağlar; features crate-bazlı ayarlanır.
- `process` (Faz 6 deep-link launcher) veya `signal` (Unix-only) ihtiyacı doğunca eklenir.

**Alternatifler neden seçilmedi:**
- `smol`: minimal ama `wry`/`tao` tokio'ya bağımlı, mixing runtime imkansız.
- `glommio`: **Linux-only**, io_uring. Windows hedef dışı.
- `async-std`: ekosistem hareketi durmuş (2026'da ölü sayılır).

---

## ADR-0003: Config Kütüphanesi — `config-rs` (`figment` değil)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 0.0
- **Önceki plan:** `figment = "0.10"`

### Context

`figment` (`crates.io/crates/figment`) Mayıs 2024'ten beri güncellenmedi. Son sürüm 0.10.19, 2024-05-17 tarihli. Topluluk fork'u `figment2` (lmmx tarafından, 0.11.5 / Nisan 2026) çıktı. Viscos 4 yıllık proje; AI agent yeni PR'lar üretiyor; upstream güncellemesi alamamak technical debt olur.

### Decision

**`config-rs`** (versiyon 0.14, MIT/Apache), `default-features = false`, `features = ["toml", "convert-case"]`.

```rust
use config::{Config as ConfigRs, File, Environment};

ConfigRs::builder()
    .add_source(File::with_name("config/default").required(false))
    .add_source(File::with_name("config/local").required(false))
    .add_source(Environment::with_prefix("VISCOS").separator("__").try_parsing(true))
    .build()?
    .try_deserialize()
```

### Consequences

**Olumlu:**
- Aktif bakım, 12-factor standard, 3.1K+ star, endüstri standardı.
- TOML/JSON/YAML/INI/RON/JSON5/Corn desteği → format seçimi açık.
- `__` separator ile nested env override (`VISCOS_LOGGING__LEVEL=debug`).
- GPL-3.0 projeyle lisans uyumu (figment de OK, config-rs de OK; confy GPL-3.0+ → kontaminasyon riski).

**Olumsuz / Kabul edilen riskler:**
- Read-only (write-back yok). Viscos için OK: kullanıcı ayarları SQLite'ta, app config salt okunur.
- API figment'ten biraz daha verbose (`builder().add_source()` vs `Figment::new().merge()`). Çok küçük fark.

**Alternatifler neden seçilmedi:**
- `figment` (mevcut plan): stale, 2 yıldır güncelleme yok, topluluk terk etmiş.
- `figment2`: drop-in replacement, fork olması nedeniyle ana akım crate'in gerisinde; tek kişilik bakım.
- `confy`: write-only boilerplate ama layered config yok; lisans GPL-3.0+ (kendi lisansımızla çakışıyor).
- `serde_yaml` + manual: layer, env, profile yok, hepsini elle yaz.

---

## ADR-0004: GitHub Actions + 7-Job Matrix (Tek runner, 2 katmanlı cache)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 0.0
- **Önceki plan:** Tek `test` job + rust-cache

### Context

Viscos v1 Windows-only; CI dakika kullanımı düşük (tek kişi + AI agent); toplam 11 crate + native bağımlılıklar (WinAPI, WebView2, COM) build'i ağırlaştıracak (Faz 1+). Hız + maliyet + güvenlik + lisans uyumu + binary bütçesi koruması.

### Decision

**GitHub Actions** + `windows-latest` runner. **7 ayrı job:**

1. `fmt` — `cargo fmt --all -- --check`
2. `clippy` — `cargo clippy --workspace --all-targets -- -D warnings` (pedantic + nursery)
3. `test` — `cargo nextest run --workspace --all-features --retries 2` + ayrıca `cargo test --workspace --doc`
4. `build` — `cargo build --workspace --release --locked` (sccache + rust-cache); 25 MB size gate
5. `audit` — `cargo audit` (haftalık + her PR)
6. `deny` — `cargo deny check --all-features` (license + source + ban)
7. `geiger` — `cargo geiger --all-features` (warn-only, bilgi amaçlı)

**Cache stratejisi (2 katman):**
- `Swatinem/rust-cache@v2` — registry + build deps (tek blob, 10 GB GH cache limit)
- `sccache` (mozilla-actions/sccache-action) — compiler artifact (object-level, GH cache backend)

### Consequences

**Olumlu:**
- Hızlı geri bildirim: `fmt` ve `clippy` 30s-1dk; `test` 2-3 dk; full build 5-8 dk (sccache warm).
- `nextest` paralel + izole test execution → 2-3x hızlanma + flaky retry.
- `audit` + `deny` haftalık schedule ile transitive dependency güvenlik açıkları erken yakalanır.
- 25 MB size gate binary bütçesini (15-25 MB hedef) korur; release-to-release regression görünür olur.
- Lint + audit + deny = AI-PR'larının %80'i merge öncesi otomatik yakalanır, insan review sadece mimari + edge case'e odaklanır.

**Olumsuz / Kabul edilen riskler:**
- 7 job → daha fazla Actions dakikası. Ancak paralel job'lar (fmt/clippy/audit/deny) zaten paralel koşar, çoğu < 1 dk.
- `geiger` false-positive verebilir (WinAPI yoğun projede) → `continue-on-error: true` ile bilgi amaçlı.
- Self-hosted runner'a Mart 2026'dan beri $0.002/dakika platform fee var. Kârlılık eşiği: aylık 3000+ dakika.

**Gelecek:**
- Build süreleri 10+ dakikaya çıkınca (Faz 4+) **Depot veya WarpBuild** managed runner (AMD EPYC Genoa, 2-3x hızlı, per-second billing, %50 ucuz).
- Self-hosted: ancak aylık 3000+ dakika + operasyon ekibi kapasitesi varsa.

---

## ADR-0005: LTO `fat` + `panic = "abort"` (Release Profile)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 0.0
- **Önceki plan:** `lto = "thin"`

### Context

Viscos binary bütçesi **15–25 MB** (master index Bölüm 1). Faz 8'de MSI dağıtımı; kullanıcı disk ve indirme süresi önemli. `lto = "thin"` ile derlenmiş Rust binary'si tipik olarak 30-40 MB (release). Discord client GUI app; panic sonrası process watchdog (Faz 1) tarafından yeniden başlatılır.

### Decision

```toml
[profile.release]
opt-level = 3
lto = "fat"               # thin yerine fat → 1-3 MB ek kazanç, %2-5 runtime artışı
codegen-units = 1
strip = true
panic = "abort"           # unwind yok → binary küçülür, watchdog ile restart
```

### Consequences

**Olumlu:**
- Binary ~20-30% küçülür (tipik 30 MB → 20 MB aralığı).
- Runtime performans %2-5 artar (daha agresif inlining + dead code elimination).
- GUI app'te panic → abort → process exit → watchdog restart; unwinding maliyeti gereksiz.

**Olumsuz / Kabul edilen riskler:**
- Build süresi %20-30 artar (sccache ile telafi).
- Test'lerde `panic!`/`assert!` macros'lar unwind bekler; **default profile (test) zaten unwind**, sadece release'te abort — çakışma yok.
- Debug zorlaşır (stack trace unwind olmadan). Release'te bu trade-off kabul edildi.

---

## ADR-0006: Toolchain 1.89 + Edition 2024 (twilight-rs uyumu)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 0.0
- **Önceki plan:** `rust-version = "1.80"` (orijinal)
- **Önceki revizyon:** `1.85` (2026-06-18, Edition 2024 baseline)
- **Revizyon:** `1.89` (2026-06-18, twilight-rs MSRV uyumu — detay ADR-0008)

### Context

`rust-version = "1.80"` Edition 2024'ü destekler ama garanti etmez. Edition 2024'ün resmi stabil sürümü **Rust 1.85** (Şubat 2025). 1.80 → 1.85 arası 5 minor sürüm; modern cargo resolver, async closure, `unsafe extern` zorunluluğu, `rust-version`-aware dependency resolver bu sürümlerde geldi.

**Haziran 2026 revizyonu — MSRV 1.85 → 1.89:** `twilight-rs` 0.17.x serisi (Discord REST + Gateway kütüphanesi, ADR-0008) Aralık 2025'ten itibaren **MSRV 1.89** zorunluluğu getirdi. Twilight ekibi MSRV değişikliklerini minor sürümde yapabileceklerini açıkça belirtiyor. Viscos, Discord transport katmanında twilight'a bağımlı olacağı için (`twilight-model` + `twilight-http` + `twilight-gateway`), MSRV'yi twilight ile hizalamak zorunlu. 1.85 → 1.89 arası 4 minor sürüm; nightly'ye gerek yok, tamamen stable kanal.

### Decision

```toml
# rust-toolchain.toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy", "rust-analyzer"]
profile = "minimal"
```

```toml
# Cargo.toml [workspace.package]
edition = "2024"
rust-version = "1.89"
```

### Consequences

**Olumlu:**
- Edition 2024 prelude (`Future`, `IntoFuture`) → async ergonomics.
- `Box<[T]>: IntoIterator` → slice/vec geçişleri.
- `unsafe extern` zorunluluğu → C-FFI'da typo koruması (Faz 1+'ta `windows-rs`, `webview2-com` yoğun).
- Cargo `rust-version`-aware resolver → dependency uyumluluk kontrolü otomatik.
- **twilight-rs 0.17.x serisiyle uyumlu** (ADR-0008) → Discord transport katmanında AI-yazım riskini sıfırlar.

**Olumsuz:**
- 1.85'e geri dönülemez (kullanıcı toolchain'i güncellemeli). Viscos yeni proje, breaking OK.
- `lto = "fat"` + `rust 1.89` kombinasyonu: twilight-model derive'ları nedeniyle cold build 5-7 dk artabilir (sccache telafi eder, ADR-0004).

---

## ADR-0007: Error Handling — thiserror (lib) + anyhow (app)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 0.0

### Context

Endüstri standardı 2026: library'ler `thiserror` ile typed enum, application binary'ler `anyhow` ile context'li aggregation. Snafu / eyre / miette alternatifleri var ama Rust ekosistemi bu split'i netleştirdi.

### Decision

- **`viscos-error` crate** (library): `thiserror` ile `ViscosError` enum, `#[non_exhaustive]`, **NO** `anyhow::Error` variant.
- **`viscos` binary main**: `anyhow::Result<()>`, `.context()` ile glue.
- Public API her zaman `Result<T, ViscosError>` döner.

### Consequences

**Olumlu:**
- Library tüketicisi (internal crate'ler dahil) `match ViscosError::Config(...)` yapabilir.
- `#[non_exhaustive]` sayesinde AI veya insan yeni variant ekleyebilir → breaking change yok.
- `anyhow` yalnızca application boundary'de; test/debug context'leri temiz.

**Olumsuz:**
- Internal crate glue'larında iki kere dönüşüm gerekebilir (`ViscosError` → `anyhow::Error` → log). Küçük boilerplate.

---

## ADR-0008: Discord REST + Gateway — `twilight-rs` (sub-crate seçici entegrasyon)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 2.0 / 3.0
- **Önceki plan:** Sıfırdan `reqwest` + `tokio-tungstenite` + custom zstd-stream framing

### Context

Viscos Faz 2 (REST) ve Faz 3 (Gateway) planları, Discord HTTP ve WebSocket protokolünü sıfırdan yazmayı öneriyordu: `reqwest` + `tokio-tungstenite` + manual zstd-stream decompression + manual opcode dispatching + manual rate-limit + manual session resume + manual reconnect backoff. Bu yaklaşım:

1. **AI-yazılım projesi için yüksek hata riski taşır**: Discord Gateway'in edge case'leri (zstd-stream partial frame buffer yönetimi, opcode state machine, hello→identify→ready handshake, session resume correctness, intent validation) production'da ancak gerçek hesapla test edilir — `phase-3.0-gateway.md` mevcut taslağında `read_payload` fonksiyonu `zstd_buffer.clear()` her binary frame sonrası çağırıyor, bu zstd-stream formatını bozar (partial frame'ler biriktirilmeli).
2. **Discord protocol drift yükü**: Discord 6-12 ayda bir breaking change yapıyor (örn. davalarında compressed payload, intent değişiklikleri, opcode ekleme). Custom implementasyon her seferinde geriye düşer.
3. **Mevcut rakiplerin hepsi Discord API'ını sıfırdan yazmıyor**: Dissent C++/Qt ile yıllar harcadı, Equirust/Ventauri web client wrapper yaklaşımını (Vencord/Equicord) kullanıyor — Rust tarafı sadece host.
4. **Topluluk standardı iki seçenek**: `twilight-rs` (modüler, low-level) ve `serenity` (opinionated, batteries-included). İkisi de aktif (Twilight 0.17.1 Aralık 2025, Serenity 0.12.5 Aralık 2025).

**Mevcut plan zaten "twilight (low-level)" notunu içeriyor** (viscos_index.md Bölüm 6) — kodda kullanılmıyordu, sadece niyet vardı. Bu ADR niyeti somutlaştırır.

### Decision

**`twilight-rs` 0.17.x serisinin üç sub-crate'ini al, geri kalanını alma:**

```toml
[workspace.dependencies]
# Tip tanımları — User, Guild, Message, Channel, Role, vb.
twilight-model = { version = "0.17", default-features = false, features = ["tracing"] }

# REST client — rate-limit, X-Super-Properties, brotli dahil
twilight-http = { version = "0.17", default-features = false, features = ["rustls-platform-verifier", "simd-json"] }

# WebSocket Gateway — sharding, session resume, zstd-stream built-in
twilight-gateway = { version = "0.17", default-features = false, features = ["twilight-http", "rustls-platform-verifier", "zstd", "simd-json"] }
```

**Alınmayacak** (bilinçli dışlama):
- `twilight-cache-inmemory` ❌: Faz 4'te `moka + SQLite + foyer` planlanmış. twilight-cache event'leri generic tipte tutar, custom cache'lerimiz Discord API'sinden zengin bilgi (örn. son görülme, presence) çıkarabilir.
- `twilight-util` ❌: Slab allocator override'ları, gerekli değil.
- `twilight-standby` ❌: Bot prefix command'leri için, user client'ta kullanılmıyor.
- `twilight-mention` ❌: Parse kütüphanesi, bizim `@` parse'ımız simpler.
- `twilight-voice` ❌: Faz 7'de DAVE E2EE özel implementasyon (Sen kütüphanesi yeterli değil).

**Viscos'un kendi katmanı korunur:**
- `viscos-auth` (keyring-core, secrecy, zeroize, MFA TOTP+backup codes, X-Super-Properties) → Viscos'a özel, twilight'dan bağımsız. Detay: ADR-0011.
- `viscos-cache` (moka + SQLite + foyer) → twilight-cache'in yerine
- `viscos-events` (Gateway event dispatch) → twilight-model'in typed event'lerini `viscos-core::GatewayEvent` enum'una map eder
- IPC pull-based pattern → Twilight'tan etkilenmez, kendi crate'imiz

### Consequences

**Olumlu:**
1. **AI-yazım riski sıfırlanır**: Twilight ekibi Discord protocol değişikliklerine 24 saat içinde cevap veriyor (Discord davalarında custom implementasyonlar 1-2 hafta geri kalıyor).
2. **Modern transport bedava gelir**: Twilight 0.17 `tokio-tungstenite`'i bırakıp `tokio-websockets`'e geçti (Autobahn-strict, SIMD-accelerated masking/UTF-8, daha hızlı). Bizim planımızda hâlâ `tokio-tungstenite 0.24` yazıyordu.
3. **Type-safe model katmanı**: `twilight-model` Discord'un tüm objelerini typed enum'larla veriyor. Hand-maintained, Discord API değişikliklerinde güncelleniyor. `viscos-core`'daki kendi event'lerimiz için typed adaptör yazılır.
4. **Rate limiting ve fingerprinting dahil**: Twilight HTTP'si `X-RateLimit-*` header'larını doğru handle ediyor; bizim planımızda bu elle yazılıyor ve `governor` crate'i ile per-route limiter inşa ediliyordu (kod karmaşıklığı).
5. **Modüler feature flag'lerle binary kontrolü**: `default-features = false` + sadece ihtiyaç duyulanlar. twilight-voice (200+ KB) ve twilight-cache bizim feature'larımızda yok.
6. **Phase-3'teki zstd-stream bug'ı bertaraf edilir**: Twilight 0.17 production-tested zstd decoder ile geliyor, bizim manuel implementasyonumuzdaki buffer clear bug'ı yok.

**Olumsuz / Kabul edilen riskler:**
- **MSRV 1.85 → 1.89**: twilight-rs 0.17 Aralık 2025'ten beri MSRV 1.89 zorunluluğu getirdi. ADR-0006 güncellendi.
- **+1-2 MB binary**: twilight-model tüm Discord objelerini typed struct olarak tutuyor (Guild, Member, Channel, Role, Message, Embed, Attachment, ...). `lto = "fat"` ile dead-code eliminasyonu sonrası 1-2 MB artış. 25 MB binary bütçesine sığar (mevcut ~20 MB → ~22 MB).
- **+30-60s cold compile time**: twilight-model derive'ları yoğun. sccache telafi eder (ADR-0004).
- **Twilight kendi `DiscordHttpError` tipini kullanıyor**: `viscos-error::ViscosError::Api(ApiError)` adaptör katmanı gerekli (`From<twilight_http::Error>` impl).
- **Twilight'ın bot-merkezli bazı tipleri user client'ta kullanılmıyor**: `Interaction` event'leri bizim için yine de lazım (slash command için değil ama user settings sync için). Sorun yok, gereksiz değil.

**Alternatifler neden seçilmedi:**

- **`serenity` (0.12.5, Aralık 2025)**: MSRV 1.74 (daha düşük, iyi), ama monolithic yapı:
  - `temp_cache` (mini-moka) Faz 4 cache planıyla çakışır
  - `standard_framework` deprecated, Poise öneriliyor (Poise = komut framework'ü, user client'ta anlamsız)
  - Bot-centric `Client`/`Shard` modeli user-token akışına ergonomik değil
  - Default features açıkken twilight'dan 2-3x daha büyük binary
  - twilight-cache-inmemory çakışması yine var
  
- **`fastwebsockets` (direkt)**: Deno'nun altında, en hızlı (2-3x tokio-tungstenite), ama low-level — gateway logic'i kendin yazmak zorundasın. Twilight zaten üzerine kurulu. Tek başına kullanmak twilight kadar kod yazmak demek, faydası yok.

- **`discord-user-rs` / `discord-authority` (selfbot)**: Net ToS ihlali. Discord multi-layered detection (Canvas/WebGL fingerprinting, behavioral heuristics) ile tespit edip banlıyor. Viscos "user-token + native UI wrapper" kategorisinde, selfbot değil; bu kütüphaneler bizi yanlış kategoriye sokar.

- **`reqwest` + `tokio-tungstenite` (custom)**: Sıfır maliyet ama yüksek risk:
  - `phase-3.0-gateway.md` mevcut taslağında zaten zstd-stream framing bug'ı var
  - AI 500+ satır gateway kodu yazarken 1-2 PR'da doğru yazamayabilir
  - Discord protocol değişikliğinde her seferinde AI yeni PR açar
  - Ventauri/Equirust/Dorion hepsi "sıfırdan yazma" yerine "var olanı wrapper'la" yaklaşımını seçmiş

**Gözden geçirme tetikleyicileri:**
- Twilight 1.0 çıkarsa (şu an 0.17.x) → MSRV/karmaşıklık değişimi
- Discord yeni bir protocol major versiyonu çıkarırsa (örn. v11)
- twilight-cache-inmemory bizim cache planımızdan (Faz 4) üstün olursa (olası değil, moka+SQLite+foyer Discord özelinde optimize)
- Binary bütçesi 25 MB'ı aşarsa → twilight'i kaldırıp custom'e dönmek (en son çare)

---

## ADR-0010: Cache Stack — SQLite + moka + foyer (Varyant A, 2026 Q2)

- **Tarih:** 2026-06-18
- **Durum:** ✅ Accepted
- **Faz:** 4.0
- **Önceki plan:** [`phase-4.0-cache-media.md`](../../.cursor/plans/phase-4.0-cache-media.md) — `rusqlite 0.32 + moka 0.12 + foyer 0.10`
- **Araştırma dokümanı:** [`cache-stack-research.md`](../../.cursor/plans/cache-stack-research.md)
- **Revizyon:** 2026-06-18 (Haziran 2026 trade-off analizi — CDN content-addressable + adaptive tier eklendi, detay aşağıda)

### Context

Faz 4 planı (Haziran 2026) cache stack'i şu üç katmandan oluşuyor: SQLite (mesaj geçmişi, sunucu/kanal/üye metadata), moka (RAM cache, sıcak mesaj/üye lookup), foyer (medya blob hybrid disk cache). Plan aylar önce yazıldı; Rust ekosistemi 2026 başında önemli hareketler gördü:

1. **moka vs alternatifleri (cachebench OLTP trace, 16 concurrent client):** Stretto 0.9 OLTP workload'da %33–47 pp daha iyi hit ratio veriyor (Discord read-state pattern'iyle bire bir örtüşüyor). Ama scan-heavy trace'lerde admission policy %57 pp kayıp yaratabiliyor.
2. **SQLite Rust stack:** Delta Chat production'da SQLx'ten **rusqlite'a geri dönmüş** (async %20–30 perf regression). Rusqlite 0.32 → 0.38 (Ara 2025) minor versiyon birikimi. Redb pure Rust ama **SQL yok** — Viscos'un `WHERE channel_id + timestamp DESC` pagination query'leri imkansız.
3. **Hybrid disk cache:** foyer 0.10 → 0.22 (Ocak 2026) 12 minor versiyon, hybrid engine olgunlaşmış. RisingWave, Chroma, SlateDB production kanıtı. BlobCache (Go, disk-first FIFO) %20 daha hızlı ama Rust port'u yok.
4. **Discord CDN URL politikası (Ekim 2023'ten beri, Haziran 2026'da hâlâ aktif):** User-uploaded attachment'lar (resim, video, dosya) **24 saatlik signed URL** ile dağıtılıyor (`ex`, `is`, `hm` query params). İçerideki mesajlarda Discord URL'i otomatik yeniliyor ama **harici tüketiciler** `POST /channels/{channel_id}/attachments/refresh-urls` endpoint'ini kullanmalı. **Avatar, banner, guild icon, emoji, sticker imzasız** — asla expire olmaz.
5. **Moka v0.12 (Mart 2026) — background thread kaldırıldı:** v0.12.0 ile lock-free foreground maintenance'a geçti, CPU idle düştü. W-TinyLFU geçişi ve cache stats roadmap'te var ama yok.

Viscos'un iş yükü Discord mesaj erişimi → **karışık pattern**: sıcak kanallar OLTP-style (hot set + frequency skew), soğuk kanal scroll scan-style. Plan'daki üçlü stack doğru seçimleri içeriyor, sadece minor version güncellemeleri ve gereksiz dependency temizliği gerekli.

**Haziran 2026 trade-off revizyonu (bu karara eklenen):**
- **CDN URL stratejisi (yeni):** "URL-as-key" cache'leme 24 saat sonra tüm attachment cache'inin invalid olması demek. **Content-addressable** (Discord `attachment_id` snowflake → blob) stratejisi cache'in ömrünü sınırsız yapar, signed URL'i sadece fetch anında alır. Background worker 23h'de batch refresh yapar.
- **Adaptive tier sizing (yeni):** Faz 1.5 telemetry backend'inden hit ratio + restart trend → tier boyut tuning. RAM hit ratio <%70 → cache tier büyüt veya Stretto'ya geç; disk hit ratio <%40 → disk boyutu 10 GB → 25 GB.

### Decision

**Varyant A (önerilen) — mevcut stack korunur, 2 minor bump + 1 cleanup + 2 yeni strateji:**

```toml
[workspace.dependencies]
# DB — rusqlite 0.32 → 0.38 (Aralık 2025, 2 yıllık patch birikimi)
rusqlite = { version = "0.38", features = ["bundled", "blob"] }
r2d2 = "0.8"
r2d2_sqlite = "0.24"

# Migrations
refinery = { version = "0.8", features = ["rusqlite"] }

# In-memory cache — moka korunur
moka = { version = "0.12", features = ["future"] }

# Hybrid disk cache — foyer 0.10 → 0.22 (Ocak 2026, olgunlaşma)
foyer = "0.22"

# Encryption — sadece AES-GCM (chacha20poly1305 çıkarıldı)
aes-gcm = "0.10"
argon2 = "0.5"

# Allocator (koşullu benchmark)
tikv-jemallocator = "0.5"
```

**Cleanup:** `chacha20poly1305` dependency kaldırıldı (Win10/11 x86_64'te AES-NI yaygın, software fallback gereksiz; +60 KB binary tasarruf).

**Encryption anahtar yönetimi:** Varyant A — `keyring-core` + `windows-native-keyring-store` (DPAPI arkası) default. **Tek secret store** hem `viscos-auth` token'larını hem `viscos-cache` encryption anahtarını yönetir. Varyant B (Argon2id passphrase) v2.0'da opt-in. Detay: **ADR-0011**.

**Allocator:** System default v1 için; tikv-jemallocator **≥%15 RSS azalması** koşuluyla adopt (Faz 4 sonu benchmark).

---

### Decision Eki (Haziran 2026 trade-off revizyonu)

**A) Discord CDN URL Stratejisi — Content-Addressable + Lazy Refresh:**

Disk medya cache key stratejisi **Discord `attachment_id` snowflake** (u64) olur, tam signed URL değil. Çünkü Discord user-uploaded attachment URL'leri 24 saat signed (`ex` + `is` + `hm` HMAC). İmzalar expire olduğunda cache invalid olur, kullanıcı yeniden indirmek zorunda kalır. Content-addressable stratejisi cache'in ömrünü sınırsız yapar.

```rust
// crates/viscos-media/src/cache.rs (Faz 4'te eklenecek)
pub struct MediaCache {
    /// Disk-first hybrid cache. Key = Discord snowflake (u64 → 64-bit hash),
    /// Value = encrypted blob + metadata. URL asla key'e girmez.
    blobs: foyer::HybridCache<u64, EncryptedMediaBlob>,

    /// RAM-only metadata cache (signed URL + expiry). 1 saat TTL.
    /// Cache miss → arka planda refresh-urls çağrısı.
    url_meta: moka::future::Cache<u64, CdnUrlMeta>,
}

pub struct CdnUrlMeta {
    pub signed_url: String,           // ex + is + hm params dahil
    pub expires_at: SystemTime,        // genelde NOW + 24h
    pub mime: String,
    pub size: u32,
}

pub struct EncryptedMediaBlob {
    pub ciphertext: Vec<u8>,           // nonce || AES-GCM(plaintext)
    pub mime: String,
    pub size_plain: u32,
}
```

**Refresh worker:**
```rust
// crates/viscos-media/src/refresh.rs (Faz 4'te eklenecek)
pub struct CdnRefreshWorker {
    /// 23. saatte olan URL'leri batch'le (50 per call, Discord limit).
    /// POST /channels/{channel_id}/attachments/refresh-urls
    pending: HashMap<u64, (ChannelId, AttachmentId)>,
}

impl CdnRefreshWorker {
    pub async fn tick(&mut self, media: &MediaCache) -> anyhow::Result<()> {
        // 1. moka'dan 23h < expires_at < 24h olanları seç
        // 2. channel_id bazlı grupla
        // 3. Her grup için refresh-urls çağır (rate-limit aware)
        // 4. Yeni signed URL'i moka'ya yaz (encrypted blob cache'inde değişiklik yok)
    }
}
```

**Avantajları:**
- Disk cache ömrü sınırsız (signed URL sadece fetch anında alınır)
- 24 saatte cache invalid yok → cold start sonrası kullanıcının eski attachment'ları anında görünür
- Discord rate-limit'e uyumlu (50 URL per call)
- Avant-garde cache invalidation yok — "cache key = stable identity" prensibi

**Signed URL olmayan kaynaklar (avatar, banner, guild icon, emoji, role icon):**
- Bunlar **asla expire olmaz**. Key stratejisi basit: tam URL'i SHA-256 hash'le, moka'da tut (RAM only, çünkü küçük ve sıcak). Disk'te tutmaya gerek yok — CDN cache'i zaten kendi cache'inde tutuyor.
- Alternatif: foyer'a koy ama **boyut küçük (80KB-256KB), erişim sıcak, RAM yeterli**.

**B) Adaptive Tier Sizing (Faz 1.5 Telemetry-Driven):**

Cache tier boyutları **runtime'da telemetry verisine göre tune** edilir. İlk default değerler v1'de statik; v1.5'te adaptive.

| Tier | v1 Default | Adaptive Trigger (Faz 1.5 telemetry) |
|---|---|---|
| moka (sıcak metadata) | 64 MB | Hit ratio <%70 ise 128 MB; >%95 ise 32 MB (RAM geri kazan) |
| foyer memory tier | 32 MB | Disk hit ratio >%60 ise 64 MB (RAM'de daha fazla) |
| foyer disk tier | 10 GB | Disk hit ratio <%40 ise 25 GB kullanıcı onayı ile |

Tuning **background'da** olur, kullanıcı transparan (config dosyasında `cache.tier.auto_tune = true` opt-out). Telemetry SQLite'a yazılır (Faz 1.5 backend), her saat aggregate → adaptive algorithm tier'ları ayarlar.

### Consequences

**Olumlu:**
1. **Industry standard kanıtı:** moka (Caffeine-Java ekolü, crates.io %85 hit ratio üretim kanıtı) + rusqlite (Delta Chat, Tauri apps) + foyer (RisingWave, Chroma, SlateDB) üçlüsü en geniş üretim kanıtına sahip kombinasyon.
2. **AI-yazar riski düşük:** Üçü de tanıdık API, örnek kod plan'da mevcut. AI'ın yanlış implement etme olasılığı minimum.
3. **Lisans uyumu:** Hepsi MIT/Apache-2.0 → GPL-3.0 ile uyumlu (cargo-deny check licenses yeşil).
4. **Foyer minor bump faydaları:** 0.10 → 0.22 arası API kararlılığı artmış, plug-in policy (LRU/S3-FIFO/w-TinyLFU), zero-copy in-memory abstraction, RisingWave production deployment kanıtı.
5. **rusqlite minor bump faydası:** 0.38'de performans iyileştirmeleri, blob feature zaten var, API uyumlu.
6. **chacha20poly1305 çıkarma:** +60 KB binary tasarruf (binary bütçesi 15–25 MB'yi korur), kod sadeleşmesi.
7. **Encryption uyumu:** AES-GCM + AES-NI hardware acceleration (tüm modern Win10/11 CPU'ları), tek AEAD yeterli.
8. **Allocator stratejisi:** System default ile başla, kanıt bazlı jemalloc'a geçiş (gereksiz complexity introduction yok).
9. **CDN content-addressable (Haziran 2026 eki):** 24 saat signed URL limit'i cache ömrünü sınırlamaz. Background refresh worker ile rate-limit uyumlu. Cache key = stable identity (Discord snowflake) prensibi ile cache invalidation problemi sıfırlanır.
10. **Adaptive tier sizing (Haziran 2026 eki):** Faz 1.5 telemetry verisi ile tier boyutları tune edilir. Kullanıcı transparan opt-out. Hardcoded default + runtime auto-tune kombinasyonu.
11. **Moka v0.12 background-thread kaldırma:** Lock-free foreground maintenance → CPU idle düşer (Viscos <%1 idle hedefi için kritik).

**Olumsuz / Kabul edilen riskler:**
- **foyer 0.10 → 0.22 minor bump API breaking olabilir** (0.x → 0.x major parity, orta risk). İlk implementasyonu 0.22 ile yapılır, migration doc AI-yazar'a verilir.
- **foyer Windows NTFS overhead:** Linux io_uring engine'i WIP, Windows'ta sadece psync (blocking pread/pwrite + thread pool) → %30 latency overhead kabul edilmeli. NTFS file system journal overhead. Modern NVMe SSD'lerde throughput yeterli, sadece cold latency (ilk erişim) etkilenir.
- **Stretto PoC ertelendi:** Stretch goal olarak Faz 4 sonuna alındı (cachebench benchmark ile gerçek workload ölçümü). Şu an için erken değişiklik riskini almıyoruz.
- **chacha20poly1305 çıkarma sonucu:** ARM/Linux desteği olmayan hedefler için fallback yok. v1 Windows-only olduğu için OK; v3'te cross-platform gündeme gelirse yeniden değerlendirilir.
- **CDN refresh worker (Haziran 2026 eki):** Faz 4'te background task eklenir → complexity artışı. Worker Discord rate-limit (50 URL per call) + gateway state aware olmalı. AI-yazar riski orta → ilk implementasyon için ADR referansı + unit test coverage >%80 zorunlu.
- **Adaptive tier sizing (Haziran 2026 eki):** v1 default değerleri statik, adaptive v1.5'te. Eğer adaptive algorithm bug'lıysa cache thrashing olabilir → opt-out flag default true (sadece telemetry'i dinler, tier'ı değiştirmez).

**Alternatifler neden seçilmedi:**
- **Stretto (in-memory alt.):** OLTP trace'lerde %33–47 pp hit ratio kazancı var, ama Viscos mesaj erişimi OLTP + scan karışık pattern; scan-heavy'de %57 pp kayıp riski. İki cache engine maintain etme yükü. Faz 4 sonu stretch PoC olarak backlog'ta.
- **mini-moka:** Async yok, Viscos tokyo mimarisiyle uyumsuz.
- **quick_cache:** En küçük footprint ama TTL/TTI yok (Viscos plan'daki 1h TTL/5dk TTI kendin implement etmek AI-yazar riski yaratır).
- **redb (DB alt.):** Pure Rust, MVCC yapısal, ama **SQL yok** → message pagination JOIN FTS imkansız. Viscos'un temel relational ihtiyacına uymuyor.
- **SQLx (DB alt.):** Async-first, compile-time query check. Ama Delta Chat production kanıtı: async %20–30 perf regression. C-FFI zaten var (rusqlite ile aynı binary etki).
- **Diesel / SeaORM (ORM):** DSL öğrenme eğrisi, AI-yazar riski yüksek. Viscos manual SQL yazacak kadar küçük (10–15 sorgu).
- **sled (DB alt.):** Alpha (son sürüm Eki 2024), üretim riski.
- **Limbo (Turso):** v0.0.22, çok erken (531 downloads).
- **CacheLib Rust binding:** C++ build gerektirir, 25 MB binary bütçesini zorlar, foyer daha optimize storage engine sunuyor.
- **BlobCache (disk cache alt.):** %20 daha hızlı (1.21 vs 1.0 GB/s) ama **Rust port'u yok** (Go native), Viscos'ta kullanılamaz.
- **Possum (disk cache alt.):** Multi-process concurrent access, hole-punching + sparse files. Viscos tek-process, gereksiz; Linux-first optimizasyon, Windows'ta zayıf.
- **DuckDB / Stoolap (DB alt.):** OLAP engine'ler, OLTP workload için yanlış tool.

**Gözden geçirme tetikleyicileri:**
- foyer 0.x → 1.0 major versiyon çıkarsa (API breaking olabilir)
- Discord message access pattern telemetry verisi moka'nın düşük hit ratio gösterdiğini ortaya koyarsa (Stretto PoC tetiklenir, %15+ pp hit ratio farkı varsa v2 backlog'a al)
- Binary bütçesi 25 MB aşılırsa (rusqlite → redb değerlendirmesi, SQL kaybı kabul edilebilirse)
- Cross-platform (Linux/macOS) hedef eklendiğinde (chacha20poly1305 geri gelebilir)
- moka 0.12'de güvenlik açığı veya bakım duraksaması olursa
- Discord CDN signed URL süresi değişirse (24h → farklı TTL, refresh worker tuning)
- Adaptive tier sizing telemetry verisi cache thrashing gösterirse (v1.5'te alarm)

---

## Revizyon Geçmişi (ADR-0010)

| Tarih | Revizyon | Gerekçe |
|---|---|---|
| 2026-06-18 | **İlk karar** | moka 0.12 + foyer 0.22 + rusqlite 0.38 + AES-GCM, chacha20poly1305 çıkarıldı |
| 2026-06-18 | **Haziran 2026 trade-off analizi** | (1) Discord CDN 24h signed URL policy keşfi → content-addressable cache key eklendi (`attachment_id` snowflake). (2) Faz 1.5 telemetry backend → adaptive tier sizing eklendi (v1 default statik, v1.5 telemetry-driven). (3) Foyer Windows NTFS overhead trade-off'u netleştirildi (psync engine, %30 cold latency overhead kabul). (4) Moka v0.12 background-thread kaldırma notu (CPU idle hedefi <%1). Detay: [`cache-stack-research.md`](../../.cursor/plans/cache-stack-research.md) |
| 2026-06-19 | **PR-3 ile implementasyon** | `viscos-cache` crate'i (SQLite + moka + foyer facade) PR-3'te birleşti. `Arc<Cache>` shared state, `upsert_message_sync` + `recent_messages` API, parent directory auto-creation regression test, content-addressable CDN key stratejisi PR-3 ile kod tabanına girdi. Repository pattern (`viscos-cache::Cache` facade) twilight-cache-inmemory yerine tercih edildi (ADR-0008 zaten bu yöndeydi). |

---

## ADR-0011: Auth Stack — `keyring-core` + `secrecy` + Varyant A Encryption (Haziran 2026)

- **Tarih:** 2026-06-18
- **Durum:** 🟡 Proposed (insan onayı bekliyor)
- **Faz:** 2.0
- **Önceki plan:** [`phase-2.0-discord-api.md`](../../.cursor/plans/phase-2.0-discord-api.md) §2 Cargo.toml, §4.1 AuthStorage, §4.3 MFA, §8 Karar Noktası
- **Araştırma dokümanı:** [`viscos_auth_research.md`](../../.cursor/plans/viscos_auth_research.md)

### Context

Faz 2 (Discord API + Auth) planı `keyring = "2.3"` + `secrecy` (risk tablosunda) + TOTP-tek-MFA ile yazılmıştı. 2026 Haziran itibarıyla:

1. **`keyring 2.3` stale**: 4.0 (Nisan 2026) ile API `keyring-core` + ayrı store crate'lerine mimari olarak ayrıldı. Maintainer'lar (Walther Chen + Dan Brotsky, `open-source-cooperative`) yeni projelere **resmen `keyring` değil `keyring-core` + bir store crate'i** kullanmalarını öneriyor. `keyring 2.3` üzerinde pin'li kalmak 4 yıllık projede API drift + güvenlik patch yükü demek.
2. **Bellek hijyeni dependency olarak yok**: `secrecy` ve `zeroize` planda risk tablosunda geçiyor ama `Cargo.toml`'da yok. Token path'lerinde `Secret<String>` + `ZeroizeOnDrop` zorunlu değil → memory dump baseline savunması zayıf.
3. **Encryption anahtarı Bölüm 8'de açık**: DPAPI / passphrase / hybrid kararı verilmemiş. Viscos v1 için bu karar netleşmeli.
4. **MFA backup codes eksik**: Discord 2024'ten beri SMS'i kaldırdı, MFA yalnızca TOTP + backup codes. Planda backup codes storage'ı yok.
5. **Captcha handling yok**: Discord 2024'ten sonra `/auth/login`'de Cloudflare Turnstile / hCaptcha agresifleşti. Planda bu akış için karar noktası yok.
6. **Multi-account v2'ye atılmış**: Keyring entry'leri şu an `user = "user_token"` (sabit) → v2'de multi-account gelince refactor. v1'den itibaren `user = user_id` (Discord snowflake) key'leyerek refactor maliyeti sıfır.
7. **X-Super-Properties detaysız**: "viscos-auth üretir" yazıyor; WebGL/Canvas hash kaynağı, `build_number` senkronizasyon stratejisi belirsiz.
8. **Self-bot ToS disclaimer tutarsız**: README + modal + ADR arasında tek metin yok.

### Decision

**`keyring-core 0.7` + `windows-native-keyring-store 1.1` + `totp-rs 5.7` + `secrecy 0.10` + `zeroize 1`** — encryption Varyant A (DPAPI/Keyring) default, Varyant B (Argon2id passphrase) **v2.0'da opt-in**.

```toml
# [workspace.dependencies]
# Token storage — keyring-core 4.0 mimarisi
keyring-core = { version = "0.7", default-features = false }                 # search kapalı → regex yok
windows-native-keyring-store = { version = "1.1", default-features = false } # search kapalı
# apple-native-keyring-store (v2 macOS), dbus-secret-service-keyring-store (v2 Linux) ileride eklenir

# MFA
totp-rs = { version = "5.7", default-features = false, features = ["zeroize"] }

# Bellek hijyeni
secrecy = { version = "0.10", features = ["serde"] }
zeroize = { version = "1", features = ["derive"] }

# QR login (UI render)
qrcode = "0.14"

# v2.0 opt-in passphrase wrapper (Faz 5+)
# argon2 = { version = "0.5", features = ["std"] }
# aes-gcm = "0.10"
# age = "0.11"  # backup/export envelope
```

**Varyant A (default, v1):** Token → `keyring-core` → `windows-native-keyring-store` (DPAPI arkası, OS-bound). `Secret<String>` ile sarmalanmış in-memory, `ZeroizeOnDrop` ile düşürüldüğünde bellek temizlenir.

**Varyant B (opt-in, v2.0):** Kullanıcı bilinçli isterse ek passphrase katmanı: Argon2id (m=19 MiB, t=2, p=1) → 256-bit KEK → AES-GCM wrapper. **v1'de yok** (UX öldürür, threat model gerekli kılmıyor).

**Çoklu hesap altyapısı (v1'den itibaren):**
- `service = "Viscos"`, `user = user_id` (Discord snowflake)
- Her account ayrı `keyring` entry
- v1 UI single-account; v2.0'da `keyring-core`'un `search` feature'ı açılarak list UI gelir (0 refactor)

**MFA backup codes (v1'den itibaren):**
- Discord 8 karakterli alphanumeric backup code veriyor
- `SerializedAccount.mfa_backup_hashes: Vec<String>` (Argon2 PHC) — keyring entry'sinde
- Kullanım: `verify_backup_code(code: &str) -> bool` (Argon2 verify)
- 10 koddan az kaldığında UI'da uyarı

**Captcha handling stratejisi (v1'den itibaren):**
- `/auth/login` `LoginResponse::CaptchaRequired { captcha_sitekey, captcha_rqtoken }` döndüğünde:
  - **Varyant önerilen:** "Tarayıcıya yönlendir" UI akışı — `discord.com/login` adresini sistem tarayıcısında aç, kullanıcı orada giriş yapsın, token'ı Discord DevTools'tan alıp Viscos'a yapıştırsın (zaten `login_token()` fonksiyonu var).
  - **Varyant reddedilen:** Headless browser (Playwright/headless_chrome) → AI-yazılım projesi için yüksek risk, +30+ MB binary, +3-4 gün ekstra geliştirme.
- `LoginResult::CaptchaRequired { url }` döndürülür, shell "Tarayıcıda giriş yap, token'ı buraya yapıştır" UI'ı açar.

**X-Super-Properties detaylandırması (`phase-2.0` §3.3'e yedirilir):**
- `build_number` senkronizasyonu: Haftalık GitHub Action (cron) → `https://discord.com/app` JS bundle'ından `release_channel` ve `build_number` parse → PR otomatik açar, insan review.
- WebGL/Canvas hash kaynağı: Win11 default CEF renderer'ından + Win10 default WebView2 renderer'ından (Faz 1.6 kararıyla uyumlu). `viscos-webview` crate'i WebGL context'i oluşturup hash'i okur, `viscos-auth`'a iletir.
- Diğer fingerprint alanları (`navigator.userAgent`, `screen`, `locale`, `timezone_offset`) hardcoded — `crates/viscos-auth/src/super_properties.rs` static JSON.

**Self-bot ToS disclaimer (canonical metin):**
- `docs/DECISIONS.md` ADR-0011 Consequences bölümünde
- `README.md` → Disclaimer
- İlk açılış modal
- `Settings → About`
- Tek metin: "Viscos, Discord'un **resmi olmayan** bir istemcisidir. Kullanıcı kendi hesabıyla giriş yapar; ToS ihlali (otomasyon, scraping, mass DM) **bu istemcinin tasarım amacı değildir** ve tüm sorumluluk kullanıcıya aittir. Discord multi-layered detection (fingerprint + behavioral heuristics) ile self-bot tespit edip banlayabilir."

### Consequences

**Olumlu:**
1. **Keyring 4.0 ekosisteminde kalma**: Stale `keyring 2.3` üzerinde pin'li kalmaktan kaçınılır. Aktif bakım + güvenlik patch + 4 yıllık proje ömrü güvenliği.
2. **Binary bütçesi korunur**: `default-features = false` × 2 (keyring-core + windows-native-keyring-store) `regex` dependency'sini (~1+ MB) alınmasını engeller. 25 MB bütçeye sığar.
3. **Bellek hijyeni baseline**: `secrecy::Secret<String>` + `ZeroizeOnDrop` her token path'inde zorunlu → memory dump'a karşı defense-in-depth. Code review'da `expose_secret()` call site'ları grep'lenebilir.
4. **Multi-account v2 için 0 refactor**: `user = user_id` key'leme v1'den itibaren. v2.0'da yalnızca `keyring-core`'un `search` feature'ı açılır + UI eklenir.
5. **MFA kurtarma akışı net**: Backup codes keyring'de Argon2 PHC, 10'dan az kaldığında UI uyarısı. Hesap kurtarma yolculuğu documented.
6. **Captcha stratejisi kararı verildi**: Headless browser'a gerek yok, tarayıcı redirect yeterli. ~3-4 gün geliştirme tasarrufu, +30+ MB binary tasarrufu.
7. **Fingerprint senkronizasyonu deterministic**: Haftalık GitHub Action + PR → Discord build_number'ı her zaman güncel. Ban riski azalır.
8. **ToS disclaimer net ve tutarlı**: 4 yerde (ADR + README + modal + Settings) tek metin → kullanıcı net anlar, proje hukuki olarak net.
9. **MSRV uyumu**: `keyring-core 1.88` + `windows-native-keyring-store 1.88` → ADR-0006 (Rust 1.89) ile uyumlu.

**Olumsuz / Kabul edilen riskler:**
- **`keyring-core 0.7` 1.0 değil**: 1.0 çıkınca API drift olabilir. ADR-0008 twilight yaklaşımıyla aynı strateji: `cargo update` haftalık + AI PR review; major bump → 1 hafta geçiş.
- **MSRV yükselmesi**: `keyring-core 1.88` + `windows-native-keyring-store 1.88` → ADR-0006 zaten 1.89. OK.
- **Backup codes UX**: Kullanıcıya 10 kod vermek + saklamak diskte, kullanıcı kaybederse hesap kurtarma yok. Mitigation: kodlar **keyring'de şifreli** (DPAPI), UI'da "göster" + "yenile" + "indir (.txt)" aksiyonları.
- **Captcha redirect UX friction**: Kullanıcı bazen "neden tarayıcıya atıyor" diye şaşırır. Mitigation: modal'da net "Discord captcha istiyor, tarayıcıda giriş yapıp token'ı buraya yapıştır" metni + GIF ile visual aid.
- **Fingerprint WebGL hash'i CEF/WebView2'ye bağımlı**: Faz 1.6 kararı (Win11 CEF default, Win10 WebView2 default) ile birlikte çalışır. Backend değişirse fingerprint üretimi değişmeli.

**Alternatifler neden seçilmedi:**

- **`keyring 2.3` (mevcut plan)**: Stale. Walther Chen dep'sini `open-source-cooperative/keyring-rs`'ye taşıdı. Güvenlik patch almayı bırakabilir. 4 yıllık proje için riskli.
- **Doğrudan `windows-dpapi` crate**: Keyring zaten DPAPI üzerine kurulu. Doğrudan kullanmak → anahtar yönetimi sana düşer (chicken-and-egg), Credential Manager UI şeffaflığı kaybolur (kullanıcı "Viscos" entry'sini Denetim Masası'nda göremez), multi-process lock yok. **Kullanma.**
- **`age` crate passphrase default**: Her açılışta passphrase UX'i öldürür. v1'de yok, v2.0'da **backup envelope** olarak eklenir (export/import).
- **TPM / Windows Hello (`NCrypt` / Passport)**: En güçlü savunma (process bile çalışsa decrypt edilemez) ama biometric + headless / RDP sorunları + Rust binding'i yok (elle `windows-rs` ile yazılır, 3+ hafta). v1 için over-engineering. v3 / Linux-port'ta değerlendirilir.
- **`secrecy`'siz `String` saklama**: Memory dump baseline savunması yok. Risk: 5 dakikalık string in-memory kalsa bile swap'e düşebilir.
- **Headless browser (Playwright/headless_chrome) captcha için**: +30+ MB binary (headless_chrome Chromium payload'ı), +3-4 gün geliştirme, AI-yazılım projesi için yüksek maintenance. Discord captcha'yı agresifleştirdi ama **redirect + token paste** yeterli.
- **`fastwebsockets` + custom gateway** (auth için değil ama benzer trade-off): AI-yazım riski + Discord protocol drift. twilight zaten kullanılıyor (ADR-0008); auth için sıfırdan yazmak anlamsız.

**Gözden geçirme tetikleyiciler:**
- `keyring-core 1.0` major versiyon çıkarsa (API breaking olabilir)
- `keyring-core` bakım duraksaması (son commit >6 ay)
- Discord `/auth/login` rate-limit politikası değişirse (örn. aggressive IP ban)
- Discord captcha zorunluluğu kaldırırsa (headless browser tartışması yeniden açılır)
- Discord MFA mekanizması değişirse (passkey / WebAuthn eklenirse)
- Kullanıcı geri bildirimi: backup codes UX friction (kod sayısı veya indirme akışı)

---

## Revizyon Geçmişi (ADR-0011)

| Tarih | Revizyon | Gerekçe |
|---|---|---|
| 2026-06-18 | **İlk öneri** | `keyring 2.3` → `keyring-core 0.7` + `windows-native-keyring-store 1.1`; `secrecy 0.10` + `zeroize 1` eklendi; Varyant A default (DPAPI) / Varyant B v2.0 opt-in (Argon2id passphrase); multi-account v1 altyapısı (`user = user_id`); MFA backup codes; captcha stratejisi (redirect); X-Super-Properties detaylandırma; ToS disclaimer canonical metin |
| 2026-06-19 | **PR-4 cross-ref** | `viscos-auth` crate'i (keyring-core 0.7 + windows-native-keyring-store 1.1 + DPAPI token storage + secrecy::Secret wrapper + zeroize derive) main branch'te zaten mevcuttu; PR-4 (shell-hotkey-audio-scaffold) ile shell → auth entegrasyonu tamamlandı. `WebGL fingerprint` (MVP-1B, `compute_for_cef` + `compute_for_webview2`) PR-2 ile implemente edildi; ADR-0012 §3 anti-bot heuristic parite için zemin hazır. |

---

## ADR-0012: Frontend Mimari — Hibrit (WebView + Native Shell) (Haziran 2026 Trade-off Revizyonu)

- **Tarih:** 2026-06-18
- **Durum:** 🟡 Proposed (insan onayı bekliyor)
- **Faz:** 1.0 / 1.6 / 8.5 (cross-cutting)
- **Önceki plan:** [`viscos_index.md`](../../.cursor/plans/viscos_index.md) Bölüm 1 + 6 (hibrit mimari niyeti, somut trade-off analizi yok)
- **Araştırma dokümanı:** [`viscos_index.md` Bölüm 6 + bu ADR'nin Context bölümü](../../.cursor/plans/viscos_index.md)

### Context

Viscos'un frontend mimari kararı ("Discord'un web versiyonunu WebView2/CEF içinde çalıştır") master index ve Faz 1.0'da niyet olarak yazılmış ama **somut trade-off analizi yapılmamıştı**. Haziran 2026 itibarıyla ekosistem durumu:

1. **WebView + native shell hibrit modeli kanıtlanmış**: Dorion (Tauri+WebView), Leto (tao+wry), Ventauri (Tauri+Vencord), Vesktop (Electron+Vencord) — hepsi production'da. Binlerce kullanıcı, stabil.
2. **Tam native UI (kind, Acheron, Dissent) var ama yıllar sürüyor**: kind tek geliştiriciyle 1+ yıldır çalışıyor, hâlâ sticker/forum/voice eksik. Acheron yeni (Ocak 2026), Dissent sadece Linux.
3. **Discord Mart 2025'te büyük UI revamp yaptı** (Onyx theme + resizeable channel list + game overlay widgets). Kullanıcıların önemli kısmı memnun değildi, Vencord plugin'leri eski UI'a döndürmek zorunda kaldı. **Web client yaklaşımı kullanıcıya her zaman override imkânı verir (CSS); native yaklaşım Discord'un kendi UX'ine mahkûm eder.**
4. **DAVE E2EE (Eylül 2024'ten itibaren zorunlu) WebRTC encoded transform API kullanıyor** — bu sadece browser tabanlı context'te çalışıyor. Native voice için MLS (`davey` crate, MIT, Rust) + Opus + custom RTP routing — ay cinsinden iş.
5. **Discord'un kendi web client'ı tüm modern medya formatlarını zaten render ediyor**: animated WebP, AVIF, Lottie sticker (Discord'un kendi blog'una göre Lilliput pipeline), animated emoji. Bunları native'te sıfırdan yazmak (libwebp + animation decoder + frame timing) **nispeten erişilebilir** ama **context dışı**: Discord'un kendi client'ı zaten yapıyor.
6. **Discord sık sık DOM/class değiştiriyor**: webpack-generated hashed class names (`message__5126c`, `username_c19a55`). Vencord + browser-cli Discord skill + BetterDiscord hepsi `[aria-label]` / `[role]` selector'larına geçti. **Bridge.ts kırılganlık riski var.**
7. **Microsoft WebView2 GDI leak (#5536, STATE: OPEN) yapısal çözümsüz** → Win11 default CEF kararı (Faz 1.6). Bu zaten planda var; ADR-0012 onu da kapsar.
8. **Discord ToS grey-zone**: Discord ToS "modify the Discord client for any reason" diyor ama pratikte Vencord/Vesktop/Dorion kullanıcıları banlanmadı (Discord mühendislerinin kendi ifadesiyle: "we are never trying to ban third party clients"). Hibrit yaklaşım **düşük ToS riski** taşıyan tarafta.
9. **Servo web engine Haziran 2026'da Discord için hazır değil**: sadece login + mesaj okuma çalışıyor, mesaj yazamıyor, WebRTC yok. v3 backlog'unda zaten var.

**Mevcut planın eksikliği:** Doğru karar (hibrit) verilmiş ama gerekçe kanıta dayalı değil. Gelecek katkıcılar "neden native değil?" sorusunu soracak — bu ADR cevabı somutlaştırır.

### Decision

**Viscos frontend mimarisi hibrit (native Rust shell + WebView2/CEF + Discord web client) olarak KORUNUR**, üç iyileştirme ekiyle:

#### 1. Mimari Karar (değişmiyor)

```text
┌──────────────────────────────────────────────────┐
│ viscos-shell (tao + iced 0.14, native)          │
│  ├─ Side panel, tray, hotkey, autocomplete      │
│  └─ IPC bridge (pull-based, ADR-0012 §2)        │
└────────────────────┬─────────────────────────────┘
                     │ IPC
┌────────────────────▼─────────────────────────────┐
│ viscos-webview (WebViewBackend trait)           │
│  ├─ WebView2 (default Win10)                    │
│  └─ CEF (default Win11, Faz 1.6)                │
│     → discord.com/app SPA, Vencord/Equicord    │
└──────────────────────────────────────────────────┘
```

**Kanıta dayalı gerekçe:**

| Kriter | Hibrit | Tam Native (kind/Acheron) | Web-only (tarayıcı) |
|--------|--------|---------------------------|---------------------|
| Geliştirme süresi (v1) | ✅ 4-5.5 ay | ❌ 2-4 yıl tek kişi | ⚠ 2-3 ay ama sadece RAM optimizasyonu, özellik kazancı yok |
| Performance (RAM/Cold start) | ✅ 150-300 MB / 1-3s | ✅ 50-150 MB / 0.5-1.5s | ❌ 500-1500 MB / 3-6s |
| Discord özellik parity | ✅ Hepsi (Discord yapıyor) | ❌ Yıllar | ✅ Hepsi |
| DAVE E2EE | ✅ WebRTC tarayıcıda | ❌ Native reverse eng. | ✅ Hepsi |
| Vencord/Equicord uyumu | ✅ Preload + bridge | ❌ Yok | ✅ Hepsi |
| GDI leak (Win11) | ⚠ WV2 → CEF default | ✅ Yok | ✅ Yok |
| Discord ToS riski | ⚠ Grey (kanıtlanmış) | ❌ Reverse eng. zorunlu | ✅ OK |
| Binary boyutu | ✅ 18 MB (WV2) / 240 MB (CEF) | ✅ ~30-60 MB | N/A |
| Single-maintainer risk | ✅ Twilight + Tauri paylaşımı | ❌ kind 1 kişi | ✅ Hepsi |
| AI-agent uyumluluğu | ✅ Mevcut plan kanıtlanmış | ❌ Çok büyük yüzey | ✅ N/A |
| Mevcut production kanıt | ✅ Vesktop/Dorion/Leto | ⚠ kind alpha | ✅ Discord.com |
| Multi-platform (v2/v3) | ⚠ Win-only v1 | ✅ Qt ile kolay | ✅ Cross-browser |

**Skor:** Hibrit her satırda ya kazanıyor ya kabul edilebilir risk taşıyor. Tam native UI 5+ kritik eksik taşıyor.

#### 2. Bridge.ts Kırılganlık Azaltma (YENİ EK)

Discord sık sık DOM/class değiştiriyor (`message__5126c` gibi hash'li class name'ler). `frontend/src/bridge.ts` için **zorunlu best-practices** (Faz 1.0'da uygulanır):

```typescript
// frontend/src/bridge.ts — Selector Resilience Rules

// ✅ DOĞRU: aria-label ve role attribute selector'ları
const messageItem = document.querySelector('[id^="message-content-"]');
const channelName = document.querySelector('[aria-label*="channel"]');

// ❌ YANLIŞ: Hashed class name'ler (Discord her deploy'da değiştirir)
const messageItem = document.querySelector('.message__5126c');

// ✅ DOĞRU: Webpack module discovery (Vencord pattern'i)
// Discord'un kendi internal store'una read-only hook
const userStore = viscos.webpack.findByProps('getCurrentUser');

// ❌ YANLIŞ: querySelector ile Discord'un kendi state'ini okumaya çalışmak
// (Discord kendi internal data store'unu React prop'larına yazmıyor)
const userName = document.querySelector('.username_c19a55')?.textContent;

// ✅ DOĞRU: Bridge state pull (ADR-0012 §3)
// Discord DOM churn'inden bağımsız yaşamak için native taraftan veri çek
const unreadCount = await viscos.invoke({
  type: 'GetUnreadCount',
  data: { channel_id: channelId }
});

// ❌ YANLIŞ: DOM observer + heuristic ile unread count tahmin etmek
const observer = new MutationObserver(/* ... */);
```

**Bu kurallar `crates/viscos-webview/BRIDGE-RESILIENCE.md` dokümanında yayınlanır** (Faz 1.0 deliverable).

**Referans kanıt:** browser-cli Discord skill, BetterDiscord styling guide, Vencord webpack integration — hepsi aynı sonuca varıyor: `[aria-label]` > `[role]` > `[class*="prefix"]` > exact class. Vencord'un kendi `findByProps` / `findByCode` API'si Discord'un internal webpack instance'ına proxy koyar (function.m setter interception).

#### 3. Anti-Bot Heuristic Parite (YENİ EK — Faz 2.0 + Faz 1.5)

Discord mühendisleri "third party clients get caught up in heuristics" diyor (HN, 2022-2024 çeşitli beyanlar). Viscos için **iki ek savunma katmanı**:

**A) Discord client fingerprint paritesi (Faz 2.0, `crates/viscos-auth/src/super_properties.rs`):**
- X-Super-Properties `client_build_number` haftalık GitHub Action sync (plan'da var, korunuyor).
- **YENİ:** WebGL hash kaynağı deterministic olmalı — Viscos hangi WebView backend kullanıyorsa (Faz 1.6: Win11 CEF, Win10 WebView2), o backend'in Chromium build'i ile **Discord'un aynı tarihli Discord stable client'ının Chromium build'i aynı fingerprint yayınlamalı**. Edge stable ile Discord stable genelde senkron, ama 1-2 hafta lag olabiliyor — `client_build_number` senkronizasyonu bu yüzden haftalık.
- **YENİ:** Fingerprint parite check (her ayda 1 GitHub Action): kendi Viscos instance'ından alınan X-Super-Properties ile aynı tarihli resmi Discord stable client'ınkinden alınan karşılaştırılır. Sapma >%5 → uyarı PR'ı.

**B) İlk 24 saat "shadow mode" (Faz 1.5 telemetry backend):**
- Yeni login olduğunda ilk 24 saat **sadece REST** kullan (gateway push subscription deferred). Bu süre zarfında:
  - Sadece okuma REST çağrıları (mesaj geçmişi, kanal listesi).
  - `user-agent` ve `X-Super-Properties` Discord'un heuristic tarafından "ısıtılıyor".
  - Yazma (mesaj gönderme) 24 saat sonra aktif olur.
- Kullanıcıya modal: "Hesabınız yeni, ilk 24 saat Discord'un davranış analizi için bekleme süresi. Mesaj gönderebilirsiniz ama bazı özellikler ısınma sonrası açılır." Veya agresif kullanıcılar için opt-out (Ayarlar → Gelişmiş → Shadow mode atla).
- **Gerekçe:** kind'ın geliştiricisi benzer bir "warmup period" uyguluyor (HN beyanı, 2025).

#### 4. DAVE için Erken API Surface Sabitleme (YENİ EK — Faz 2.0)

Faz 7'de DAVE E2EE "v1'de atlanır" diyor ama `davey` (Rust MLS implementasyonu, MIT, son commit Mart 2026, `crates.io/crates/davey`) dependency olarak Faz 2.0'a eklenir:

```toml
# crates/viscos-auth/Cargo.toml (Faz 2.0, ADR-0012 eki)
[features]
default = []
dave = ["dep:davey"]

[dependencies]
davey = { version = "0.1", optional = true }
```

**Gerekçe:** Kullanmıyoruz ama compile-time API surface'i sabitlemiş oluruz. Faz 7'de gerçek native voice implementasyonuna geçildiğinde major version bump riski azalır. `davey` MIT lisanslı (GPL-3.0 uyumlu). `crates.io/crates/davey` 21 release, aktif.

#### 5. iced 0.14 + WebView Overlay Spike (YENİ EK — Faz 1.0)

`iced 0.14` son deneysel sürüm (1.0 freeze öncesi), wgpu renderer ile native. **Henüz production deployment kanıtı az** (Halloy, Sniffnet, Neothesia production ama koskoca bir Discord client + WebView overlay senaryosu yok).

**Faz 1.0'a 1 haftalık spike todo eklenir:**
- Native side panel + WebView Discord UI aynı pencere içinde.
- IPC + frame timing ölçümü (native frame drop <%1 mi?).
- Resize davranışı (COSMIC resize lag `pop-os/libcosmic#753` çözülmüş diye not düşülmüş ama iced 0.14'te production kanıtı az).
- **Spike sonucu olumsuzsa:** iced 0.14 → 0.13 downgrade veya `egui` değerlendirmesi (immediate-mode, native shell için farklı trade-off).

### Consequences

**Olumlu:**
1. **Mimari karar kanıta dayalı belgelendi.** AI-agent'lar + gelecek katkıcılar "neden native değil?" sorusunu cevaplayabilir. Yeni trade-off senaryolarında (örn. cross-platform eklenirse) gözden geçirme tetikleyicileri net.
2. **Bridge.ts kırılganlığı yapısal olarak azaltıldı.** Selector resilience rules + webpack module discovery → Discord DOM churn'inden bağımsız side panel. Mart 2025 UI revamp'ında bridge kırılırsa, native taraf çalışmaya devam eder.
3. **Anti-bot heuristic riski azaltıldı.** X-Super-Properties parite check + 24 saat shadow mode → Discord'un heuristic tarafından "yeni client" olarak işaretlenme riski azalır.
4. **DAVE için erken entegrasyon.** `davey` optional dependency ile compile-time API surface sabitlendi. Faz 7'de native voice eklenirken major version bump'tan kaçınılır.
5. **iced 0.14 riski erken spike ile test edildi.** Faz 1.0 sonuna kadar native shell + WebView overlay senaryosunda production-grade olduğu kanıtlanır ya da alternatif değerlendirilir.
6. **Mevcut ADR-0011 (auth) + ADR-0010 (cache) + ADR-0008 (twilight) + ADR-0006 (MSRV 1.89) ile uyumlu.** Hiçbir dependency çakışması yok.

**Olumsuz / Kabul edilen riskler:**
- **Selector resilience kuralları AI-PR review yükü ekler.** Her bridge PR'ında "aria-label > class" kontrolü yapılmalı → code review checklist'e eklenir.
- **24 saat shadow mode UX friction.** Yeni kullanıcı ilk gün "neden mesaj gönderemiyorum" diyebilir → modal net metin + opt-out.
- **`davey` 0.1.x sürümü, major API drift olabilir.** Optional dependency, hiçbir runtime path'i yok → risk sıfır. Sadece `cargo update` haftalık.
- **iced 0.14 spike Faz 1.0'a 1 hafta ekler.** Plan'da "2-3 hafta" Faz 1.0 süresi var, 1 hafta ekleme **kritik değil** ama schedule risk'i var. Mitigation: spike Faz 1.0 ilk haftasında, sonuç olumsuzsa downgrade Faz 1.0 ortasında yapılabilir.
- **Fingerprint parite check GitHub Action her ayda 1 kez çalışır** (5 dakika, 1 Windows runner dakikası). ADR-0004 CI bütçesine sığar.

**Alternatifler neden seçilmedi:**

- **Tam native UI (kind, Acheron modeli, C++/Qt):** Performans kazancı yıllara değmez. AI-agent workflow için riskli (Discord protocol drift + tersine mühendislik yükü + tek-maintainer bus factor). kind'ın kendisi bile voice/screen share/sticker'da eksik, Discord Şubat 2026'da permission split'leri yaptı, Mart 2026'dan itibaren DAVE zorunlu — her değişiklik native client'ı kırar.
- **Saf Electron (Vesktop modeli):** Viscos zaten reddetmiş (RAM 500-1500 MB, binary 150+ MB, cold start 3-6s). Hibrit planın temel motivasyonu.
- **Tauri (Tauri 2):** `tao + wry + iced` zaten Tauri'siz çalışıyor (Leto kanıtı). Tauri eklemek = +compile time, +feature baggage, +upstream bug riski (`tauri#13133`, `tauri#13758`). Mobile hedef v1'de yok.
- **Servo (Rust web engine, Linux Foundation Europe):** Haziran 2026'da Discord için hazır değil (sadece login + mesaj okuma, mesaj yazamıyor, WebRTC yok). v3 backlog'unda zaten.
- **Sıfırdan native UI ama Rust + Qt binding:** `cxx-qt` Rust binding alpha. Hibrit zaten daha iyi trade-off.
- **WebView'i tamamen atla, native-only custom HTTP + custom markdown renderer:** DAVE E2EE + animated WebP + Lottie sticker + Discord'un 200+ message component type'ı + permission system + presence engine — bunların hepsini sıfırdan yazmak **5+ yıl tek kişi** ve Discord breaking change'lerinde her şey stale olur.

**Gözden geçirme tetikleyiciler:**
- Discord UI yeni büyük revamp (örn. yılda 1+ defa Mart 2025 gibi). Bridge.ts resilience rules'un yeterli olup olmadığı kontrol edilir.
- Discord yeni bir medya formatı ekler (WebP → AV2, vb.). Hybrid otomatik kaplar, native client kırar.
- Vencord/Equicord plugin ekosistemi Discord'un yeni versiyonu için hazır olmazsa (örn. Discord OAuth flow değişirse) — Viscos pre-built plugin setine bağımlı olur.
- Discord DAVE'yi "sadece browser" olmaktan çıkarıp "native MLS" gerektirirse — `davey` aktif dependency olur.
- Dissent/kind/Acheron production-grade olursa (stickler + voice + animated emoji + permission engine) — native UI trade-off'u tekrar değerlendirilir.
- Microsoft WebView2 upstream #5536 çözülürse → WebView2 default'a geri dönüş değerlendirilir (ama CEF çıkışı kalmaz, opt-in sunulur).
- CEF self-update sıkıntısı büyürse ve Edge WebView2 güvenlik yaması yeterli olursa → Win11 default'u WebView2'ye geri çekilebilir (Faz 8.5 wizard).
- Servo 1.0 + Discord mesaj yazma + WebRTC encoded transform → v3 plan güncellenir.

---

## Revizyon Geçmişi (ADR-0012)

| Tarih | Revizyon | Gerekçe |
|---|---|---|
| 2026-06-18 | **İlk öneri** | WebView + native shell hibrit kararı somut trade-off matrisi ile belgelendi. 5 ek: (1) bridge.ts selector resilience rules, (2) anti-bot heuristic parite + shadow mode, (3) davey optional dependency, (4) iced 0.14 spike, (5) `docs/CEF-VS-WEBVIEW2.md` referansı. |
| 2026-06-19 | **PR-2 / PR-4 / PR-5 / PR-6 implementasyon** | (1) **PR-2 (feat/webview-webview2-runtime):** `WebView2Backend` real `wry::WebView` runtime + `cef-backend` feature-gated stub + CLI `--backend=` override + `select_default_backend()` orchestration + RDP/Win11 detection (ADR-0012 §6 uyumlu). (2) **PR-4 (feat/shell-hotkey-audio-scaffold):** `viscos-shell` tao + iced 0.14 native panel + tray + hotkey scaffold. (3) **PR-5 (feat/api-gateway-cache-bridge):** `GatewayCacheBridge` event routing twilight-gateway → `viscos-cache` + IPC push (ADR-0010 repository pattern + ADR-0012 §3 pull-based IPC uyumlu). (4) **PR-6 (chore/meta-docs-ci-installer-audit):** release.yml + size-gate.yml CI workflows + WiX fixture BACKEND preprocessor + WinGet manifest template + comprehensive audit stage. Faz 1.6 release engineering (gerçek `cef::BrowserHost::CreateBrowser`, V8 bridge, crashpad, 24h soak) **insan PR'larına bırakıldı** — `phase-1.6-cef-default-rollout-dalga-1b.md` playbook'u tamamlandı. |

---

## Global Revizyon Geçmişi (PR-1..PR-6 Merge — MVP-1B Infrastructure Complete)

| Tarih | Revizyon | Gerekçe |
|---|---|---|
| 2026-06-19 | **PR-1..PR-6 merge: MVP-1B infrastructure complete** | 6-PR refactor serisi ana branch'e merge edildi. **PR-1** (`feat/telemetry-store-mvp3`) — telemetry store MVP-3. **PR-2** (`feat/webview-webview2-runtime`) — WebView2 real runtime + CEF feature-gated stub + backend detection (Faz 1.6 Dalga 1a/1b/1c). **PR-3** (`feat/cache-facade-repository`) — viscos-cache SQLite+moka+foyer facade. **PR-4** (`feat/shell-hotkey-audio-scaffold`) — viscos-shell tao+iced+hotkey+audio scaffold. **PR-5** (`feat/api-gateway-cache-bridge`) — viscos-api gateway+REST+gateway_cache_bridge + IPC types reorganize (MVP-2). **PR-6** (`chore/meta-docs-ci-installer-audit`) — release.yml + size-gate.yml + WiX+WinGet + comprehensive audit stage. Toplam: 11 crate, 405+ tests pass, release binary ~1.56 MB. Faz 2.0 (Auth, PR-7+) + Faz 8.0 (Release Engineering, human PRs) follow-up. Detay: [`COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md`](../COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md) ve [`phase-1.6-cef-default-rollout-dalga-1b.md`](../../.cursor/plans/phase-1.6-cef-default-rollout-dalga-1b.md). |

---

```markdown
## ADR-NNNN: <Başlık>

- **Tarih:** YYYY-MM-DD
- **Durum:** 🟡 Proposed | ✅ Accepted | ❌ Superseded (ADR-XXXX)
- **Faz:** X.Y
- **Önceki plan:** <referans> (varsa)

### Context

<Problem, kuvvetler, kısıtlar. Neden şimdi karar veriliyor?>

### Decision

<Alınan karar, somut ve net.>

### Consequences

**Olumlu:**
- ...

**Olumsuz / Kabul edilen riskler:**
- ...

**Alternatifler neden seçilmedi:**
- ...

**Gözden geçirme tetikleyicileri:**
- <Bu kararı ne zaman tekrar değerlendirmek gerekir?>
```
