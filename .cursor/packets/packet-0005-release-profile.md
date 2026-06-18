# Implementation Packet — ADR-0005: LTO `fat` + `panic = "abort"` (Release Profile)

## Header

- **ADR:** ADR-0005
- **Başlık:** LTO `fat` + `panic = "abort"` (Release Profile)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0005](../../docs/DECISIONS.md#adr-0005-lto-fat--panic-abort-release-profile)
- **Önceki plan:** `phase-0.0-foundation.md` § 1.1 (release profile)

## Hedef faz worker

**Foundation worker, Faz 0.0, Dalga 1 (kök `Cargo.toml` ile birlikte).** Bu packet ADR-0001'in ayrılmaz parçasıdır; release profile satırları kök `Cargo.toml`'a ADR-0001 packet'inde eklenir. Burada sadece doğrulama + test stratejisi detaylandırılır.

## Uygulama adımları

1. **Kök `Cargo.toml`'a `[profile.release]` ekle** (ADR-0001 packet'iyle birlikte):
   ```toml
   [profile.release]
   opt-level = 3
   lto = "fat"
   codegen-units = 1
   strip = true
   panic = "abort"
   ```

2. **Doğrulama (binary bütçesi)**:
   - `cargo build --release` → `target/release/viscos.exe` boyutu 15-25 MB aralığında.
   - `cargo bloat --release -p viscos --crates -n 20` → dead code eliminasyonu etkili.
   - `ls -lh target/release/viscos.exe` (PowerShell: `Get-Item target/release/viscos.exe | Select-Object Length`).

3. **Test davranışı** (release'te abort, test'te unwind):
   - `cargo test` (default profile) → `assert!` panic'leri unwind ile yakalanır, test geçer.
   - `cargo run --release` + panic tetikleme → process abort, exit code ≠ 0.
   - ADR-0005 Consequences: watchdog Faz 1'de bu abort'u yakalayıp restart edecek.

4. **CI entegrasyonu (ADR-0004 ile)**:
   - `build` job'unda 25 MB size gate var (zaten kurulu).
   - `--locked` flag ile `Cargo.lock` drift'i önlenir.

## Kabul kriterleri

- ✅ Kök `Cargo.toml`'da `[profile.release]` tablosu tam.
- ✅ `lto = "fat"` (string) — `lto = true` veya `lto = "thin"` DEĞİL.
- ✅ `panic = "abort"` (string).
- ✅ `codegen-units = 1` (tek codegen unit, daha iyi inlining).
- ✅ `strip = true` (sembol tablosu çıkarılmış).
- ✅ `cargo build --release` binary'si 15-25 MB aralığında (boş binary bile ~3-5 MB, Faz 1+'ta 18-25 MB).
- ✅ `cargo test` çalışıyor (default profile unwind korunmuş).

## Test stratejisi

- **Unit:** `tests/release_profile.rs` (opsiyonel) → binary dosya boyut kontrolü (üst sınır 25 MB).
- **Integration:** `cargo test` default profile'da unwind korunmuş → mevcut test'ler etkilenmez.
- **Manuel:**
  - `cargo build --release && (Get-Item target/release/viscos.exe).Length / 1MB` → 25 MB altında.
  - `cargo bloat --release -p viscos --crates -n 10` → top crate'ler beklenen (iced/wry/tao ileride).
  - `cargo run --release` + programatik panic tetikleme → abort, exit code 134 veya benzeri.

## Sınır durumları ve riskler

- **Build süresi:** `lto = "fat"` cold build %20-30 artırır. Mitigation: sccache (ADR-0004) ve Faz 1+'ta `Swatinem/rust-cache` warm.
- **Stack trace kalitesi:** Unwind yok, release binary'de backtrace sınırlı. Mitigation: Debug build'de `panic = "unwind"` (default), release'te bu trade-off kabul edildi.
- **Test etkileşimi:** Default profile unwind, release abort → ikili davranış farkı. Test'ler default profile'da yazıldığından OK.
- **Binary 25 MB aşımı:** Twilight veya moka gibi büyük dependency eklendiğinde (Faz 2+). Mitigation: Size gate CI fail → human review tetikler; ADR-0004 "Gelecek" bölümünde binary bütçesi koruma kararı.
- **`codegen-units = 1` + `lto = "fat"` = incremental build yavaşlaması:** Her değişiklik full re-link tetikler. Mitigation: Development sırasında `--profile dev` (default) kullan, release sadece release/PR'da.

## Review trigger'ları

- Binary 25 MB aşılırsa.
- Build süresi 10+ dakikaya çıkarsa (incremental dev yavaşlaması).
- `unsafe extern` veya C-FFI'dan dolayı panic mesajı debug ihtiyacı doğarsa.
- Cross-platform (Linux/macOS) hedef eklendiğinde — `lto = "fat"` cross-compile'da link sorunları çıkarabilir.

## Cross-references

- **ADR:** ADR-0001 (workspace), ADR-0004 (CI size gate).
- **Plan:** [`phase-0.0-foundation.md` § 1.1](../../.cursor/plans/phase-0.0-foundation.md).
- **Alternatif:** `lto = "thin"` (daha hızlı build, %10-15 daha büyük binary) — elendi.
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Hayır.** Kök `Cargo.toml`'a 5 satır eklemekten ibaret; CI size gate doğrulaması otomatik. Binary bütçesi aşılırsa insan review tetiklenir (zaten mimari karar gerektirir).
