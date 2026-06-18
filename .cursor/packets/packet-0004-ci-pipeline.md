# Implementation Packet — ADR-0004: CI Pipeline (GitHub Actions + 7-Job Matrix)

## Header

- **ADR:** ADR-0004
- **Başlık:** GitHub Actions + 7-Job Matrix (Tek runner, 2 katmanlı cache)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0004](../../docs/DECISIONS.md#adr-0004-github-actions--7-job-matrix-tek-runner-2-katmanlı-cache)
- **Önceki plan:** `phase-0.0-foundation.md` § 8 (ci-workflow todo)

## Hedef faz worker

**Foundation worker, Faz 0.0, Dalga 3.** `workspace-init` ve `config-system` packet'lerinden sonra; workspace ve temel crate'ler buildable olduktan sonra CI kurulur ki ilk pipeline yeşil başlasın.

## Uygulama adımları

1. **`.github/workflows/ci.yml`** oluştur — 7 job, `windows-latest`, paralel + seri karma:

   ```yaml
   name: CI
   on:
     push:
       branches: [main]
     pull_request:
     schedule:
       - cron: '0 6 * * 1'   # Pazartesi 06:00 UTC — haftalık audit/deny tetikleyici

   jobs:
     fmt:
       runs-on: windows-latest
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
         - run: cargo fmt --all -- --check

     clippy:
       runs-on: windows-latest
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
         - run: |
           rustup component add clippy
           cargo clippy --workspace --all-targets -- -D warnings

     test:
       runs-on: windows-latest
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
         - run: |
           cargo install cargo-nextest --locked
           cargo nextest run --workspace --all-features --retries 2
           cargo test --workspace --doc

     build:
       runs-on: windows-latest
       env:
         SCCACHE_GHA_ENABLED: 'true'
         RUSTC_WRAPPER: 'sccache'
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
         - uses: mozilla-actions/sccache-action@v0.0.4
         - run: cargo build --workspace --release --locked
         - name: Size gate
           run: |
             $size = (Get-Item target/release/viscos.exe).Length / 1MB
             if ($size -gt 25) { throw "Binary 25 MB sınırını aştı: $size MB" }

     audit:
       runs-on: windows-latest
       steps:
         - uses: actions/checkout@v4
         - run: |
           cargo install cargo-audit --locked
           cargo audit

     deny:
       runs-on: windows-latest
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
         - run: |
           cargo install cargo-deny --locked
           cargo deny check --all-features

     geiger:
       runs-on: windows-latest
       continue-on-error: true
       steps:
         - uses: actions/checkout@v4
         - uses: Swatinem/rust-cache@v2
         - run: |
           cargo install cargo-geiger --locked
           cargo geiger --all-features
   ```

2. **`.cargo/deny.toml`** — lisans allowlist (MIT, Apache-2.0, BSD, GPL-3.0, GPL-3.0-only), source kısıtı (`crates.io` only), ban kuralları.

3. **`README.md` rozetleri** (opsiyonel): CI badge, `actions/checkout@v4` linki.

4. **Repository Settings**:
   - Branch protection: `main` korumalı, CI required.
   - Actions: `Read and write permissions`, `Allow GitHub Actions to create and approve pull requests` (gerekirse).

5. **Doğrulama**:
   - Boş PR aç → tüm job'lar yeşil (geiger hariç, continue-on-error).
   - PR'da yanlış lisans ekle → `deny` job fail.
   - PR'da `unsafe { }` ekle → `geiger` job (warn) PR yorumunda listeler.

## Kabul kriterleri

- ✅ `.github/workflows/ci.yml` 7 job içeriyor (fmt, clippy, test, build, audit, deny, geiger).
- ✅ Tüm job'lar `windows-latest` runner'da çalışıyor.
- ✅ `Swatinem/rust-cache@v2` + `mozilla-actions/sccache-action` iki katmanlı cache aktif.
- ✅ `build` job'unda 25 MB size gate var ve fail ediyor test'te (geçici büyük binary ile doğrula).
- ✅ `audit` + `deny` haftalık schedule ile çalışıyor.
- ✅ `geiger` `continue-on-error: true` ile bilgi amaçlı.
- ✅ İlk yeşil PR merge edildi.

## Test stratejisi

- **Smoke:** Boş "hello world" crate ile tüm job'lar yeşil.
- **Negative:**
  - `Cargo.toml`'a GPL-2.0 lisanslı bir crate ekle → `deny` fail.
  - 26 MB binary üreten dummy code → `build` size gate fail.
  - `cargo fmt` violation → `fmt` fail.
  - `cargo clippy` warning → `clippy` fail.
- **Schedule:** Haftalık cron Pazartesi 06:00'da `audit` + `deny` çalıştığını doğrula.
- **Manuel:** PR'da 7 job'ın hepsinin yan yana göründüğünü kontrol et.

## Sınır durumları ve riskler

- **Cache 10 GB limit:** GH cache 10 GB sınırı; 11 crate + native deps büyüyebilir. Mitigation: `Swatinem/rust-cache@v2` zaten LRU eviction yapıyor.
- **sccache cold start:** İlk build'de sccache miss → 5+ dakika. Mitigation: `Swatinem/rust-cache` warm iken 30 saniye.
- **geiger false-positive:** WinAPI yoğun projede `unsafe` çok. Mitigation: `continue-on-error: true` (zaten karar).
- **Self-hosted runner maliyet:** Mart 2026'dan beri $0.002/dakika fee. Mitigation: Aylık 3000+ dakika olursa self-hosted değerlendir (zaten ADR'de yazılı).
- **Parallelism limit:** GitHub Actions free tier 20 paralel job. 7 job OK; 20+ olursa sıraya girer.
- **Actions dakika:** 7 job × ortalama 3 dk = 21 dk/PR. Faz 4'te build 10+ dakika olursa ADR-0004 "Gelecek" bölümündeki Depot/WarpBuild geçişi tetiklenir.

## Review trigger'ları

- Build süreleri 10+ dakikaya çıkarsa (Depot/WarpBuild managed runner değerlendir).
- Self-hosted runner kapasitesi doğarsa (aylık 3000+ dakika).
- GH Actions fiyatlandırma değişirse.
- Security advisory: haftalık `audit` bir CVE yakalarsa.

## Cross-references

- **ADR:** ADR-0001 (workspace, derlenebilirlik varsayımı), ADR-0005 (size gate, lto).
- **Plan:** [`phase-0.0-foundation.md` § 8](../../.cursor/plans/phase-0.0-foundation.md).
- **AI Workflow:** [`phase-0.5-ai-workflow-setup.md`](../../.cursor/plans/phase-0.5-ai-workflow-setup.md) — AI-PR auto-flag'leri bu CI'a bağlanır.
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Evet — bir kez.** İlk CI konfigürasyonu ve branch protection kuralları repository admin tarafından gözden geçirilmeli. Sonraki workflow değişiklikleri (örn. yeni job) PR review'unda yakalanır; mimari karar gerektirmez.
