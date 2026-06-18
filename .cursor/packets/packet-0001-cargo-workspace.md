# Implementation Packet — ADR-0001: Cargo Workspace

## Header

- **ADR:** ADR-0001
- **Başlık:** Cargo Workspace (Bazel/Buck2 değil)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0001](../../docs/DECISIONS.md#adr-0001-cargo-workspace-bazelbuck2-değil)
- **Önceki plan:** `phase-0.0-foundation.md` § 1 (Cargo workspace kurulumu)

## Hedef faz worker

**Foundation worker, Faz 0.0, Dalga 1.** Bu packet, `phase-0.0-foundation.md` `workspace-init` todo'sunu uygular. ADR-0001'den ADR-0007'ye kadar tüm Foundation packet'leri aynı PR'ın altında sıralı commit'ler halinde uygulanır; sıralama bu packet'ten başlar (çünkü workspace olmadan diğer crate'ler yok).

## Uygulama adımları

1. **Kök `Cargo.toml` oluştur** (`/Cargo.toml`):
   - `[workspace]` tablosu, `resolver = "2"`.
   - `members` listesi: Faz 0.0'da oluşan 5 crate (`viscos-core`, `viscos-config`, `viscos-error`, `viscos-log`, `viscos`); geri kalan 6 crate (api, cache, media, shell, webview, watchdog, ipc, auth) yorum satırı olarak hazır.
   - `[workspace.package]`: `version = "0.1.0"`, `edition = "2024"`, `rust-version = "1.85"`, `license = "GPL-3.0"`, `authors`, `repository`.
   - `[workspace.dependencies]`: ADR-0002 ile koordineli olarak `tokio` (granular), `tracing`, `thiserror`, `anyhow`, `config`, `serde` declare edilir (version yalnız, crate'ler kendi `Cargo.toml`'larında yeniden declare eder).
   - `[profile.release]`: ADR-0005 (lto = fat, panic = abort).

2. **Dizin yapısını oluştur** (`crates/`, `config/`, `.github/workflows/`, `.cargo/`, `docs/`):
   - Boş crate'ler için `crates/<name>/src/lib.rs` (içi boş `// placeholder`) dosyaları oluşturulur.
   - `config/default.toml` ve `config/local.toml.example` dosyaları oluşturulur (Faz 0.0'ın sonraki packet'lerinde doldurulacak).

3. **`.gitignore` ve `LICENSE` ekle**:
   - `LICENSE` (GPL-3.0) standart metni.
   - `.gitignore`: `/target`, `Cargo.lock.bak`, IDE dosyaları, `config/local.toml`.

4. **Doğrulama**:
   - `cargo metadata --no-deps --format-version 1` çalışıyor, 5 member listeliyor.
   - `cargo build --workspace` başarılı (boş binary).
   - `cargo run -p viscos` "Hello, Viscos!" yazıp çıkıyor.

## Kabul kriterleri

- ✅ Kök `Cargo.toml`'da `[workspace]` tablosu + `resolver = "2"` var.
- ✅ 5 member crate `[workspace.package]`'tan ortak edition/rust-version/license alıyor.
- ✅ `cargo build --workspace` sıfır uyarı ile geçiyor.
- ✅ `cargo tree -p viscos` çıktısında `viscos-core`, `viscos-config`, `viscos-error`, `viscos-log` bağımlılık olarak görünüyor.
- ✅ Release binary'si `lto = "fat"` + `panic = "abort"` ile derlenmiş (`cargo build --release`).
- ✅ `Cargo.lock` workspace kökünde tek dosya.

## Test stratejisi

- **Unit:** Bu packet'te test yok (sadece iskelet).
- **Integration:** `cargo build --workspace` workspace üyesi sayısını doğrular.
- **Manuel:**
  - `cargo metadata | jq '.workspace_members | length'` → 5.
  - `cargo build --release` çıktısında `Finished release` + LTO mesajı.
  - `ls target/release/viscos.exe` boyutu 20-25 MB aralığında (ADR-0005 referans).

## Sınır durumları ve riskler

- **Workspace root'a yanlışlıkla ek crate:** Yorum satırı olarak listelenen Faz 1+ crate'leri açmadan derleme denerse `member not found` hatası. Mitigation: Faz 0.0'da yorum bırak, sıralı aç.
- **Edition drift:** Bir crate `[workspace.package]`'tan edition alırken yanlışlıkla kendi `Cargo.toml`'unda `edition = "2021"` declare ederse. CI'da `cargo build --workspace` yakalar, insan review yakalar.
- **Resolver 1 vs 2:** Resolver 1 (`resolver = "1"`) ile feature unification farklı davranır → MSRV-aware resolver için 2 zorunlu. ADR-0006 referans.

## Review trigger'ları

- Crate sayısı 30+ olursa (ADR-0001'deki kendi review trigger'ı).
- Full rebuild 15+ dakikayı kronik olarak aşarsa.
- Polyglot (TS/Python) monorepo ihtiyacı doğarsa (Bazel değerlendirmesi).

## Cross-references

- **ADR:** ADR-0002 (tokio granular), ADR-0005 (release profile), ADR-0006 (rust-version 1.85).
- **Plan:** [`phase-0.0-foundation.md` § 1](../../.cursor/plans/phase-0.0-foundation.md).
- **Paket:** Bu packet'ten sonra ADR-0002 → 0007 aynı Foundation worker tarafından uygulanır.
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md) (bu dispatcher'ın ürettiği index).

## İnsan onayı gerekli mi?

**Evet — bir kez.** Workspace iskeleti ilk oluşturulduğunda insan tarafından gözden geçirilmeli. Sonraki ADR-0002–0007 packet'leri bu yapı üstüne kurulduğundan, yanlış başlangıç tüm Foundation'ı etkiler. Kabul edildikten sonra diğer packet'ler için insan onayı yalnızca mimari tutarsızlık durumunda gerekir.
