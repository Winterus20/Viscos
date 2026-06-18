# Implementation Packet — ADR-0003: Config Library — `config-rs`

## Header

- **ADR:** ADR-0003
- **Başlık:** Config Kütüphanesi — `config-rs` (`figment` değil)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0003](../../docs/DECISIONS.md#adr-0003-config-kütüphanesi--config-rs-figment-değil)
- **Önceki plan:** `phase-0.0-foundation.md` § 3 (config-system todo)

## Hedef faz worker

**Foundation worker, Faz 0.0, Dalga 2.** `viscos-config` crate'i bu packet ile oluşturulur; diğer Foundation packet'lerinden (logging, error) sonra uygulanır ki `ViscosError::Config` adaptörü hazır olsun.

## Uygulama adımları

1. **`crates/viscos-config/Cargo.toml`**:
   - `config = { version = "0.14", default-features = false, features = ["toml", "convert-case"] }` (workspace'ten inherit).
   - `serde`, `viscos-error` bağımlılıkları.

2. **`crates/viscos-config/src/lib.rs`** — public API:
   ```rust
   pub struct AppConfig {
       pub logging: LoggingConfig,
       pub cache: CacheConfig,
       pub webview: WebViewConfig,
       pub auth: AuthConfig,
       pub telemetry: TelemetryConfig,
   }

   pub fn load() -> Result<AppConfig, ViscosError> {
       ConfigRs::builder()
           .add_source(File::with_name("config/default").required(false))
           .add_source(File::with_name("config/local").required(false))
           .add_source(
               Environment::with_prefix("VISCOS")
                   .separator("__")
                   .try_parsing(true)
           )
           .build()?
           .try_deserialize()
           .map_err(ViscosError::from)
   }
   ```

3. **`config/default.toml`** (git'te, committed):
   - Her section için default değer. Yorum satırları ile ne işe yaradığı.

4. **`config/local.toml.example`** (git'te):
   - Tüm key'ler yorum satırı, kullanıcı `local.toml` kopyalayıp düzenler.

5. **`.gitignore`**: `config/local.toml` ignore edilir.

6. **Test**:
   - `tests/integration.rs`: env override (`VISCOS__LOGGING__LEVEL=debug`) doğru yükleniyor.
   - `tests/missing_file.rs`: `local.toml` yoksa hata vermeden default'a düşüyor.

7. **Doğrulama**:
   - `cargo test -p viscos-config` → 2 test geçiyor.
   - `cargo run -p viscos -- --print-config` (debug subcommand, Faz 1+'ta) config'i yazdırıyor.

## Kabul kriterleri

- ✅ `viscos-config` crate workspace member'ları arasında, `cargo tree -p viscos-config` çalışıyor.
- ✅ `config = { version = "0.14", default-features = false, features = ["toml", "convert-case"] }` declare edilmiş.
- ✅ TOML, env var (`VISCOS__SECTION__KEY`) ve opsiyonel local.toml katmanları birlikte yüklenebiliyor.
- ✅ Read-only davranış kanıtlandı (write-back API yok).
- ✅ Default config dosyası (`config/default.toml`) tüm section'ları içeriyor.
- ✅ `figment` veya `confy` dependency yok (cargo deny check licenses geçiyor).

## Test stratejisi

- **Unit:** Her config struct'ı için default değer testi.
- **Integration:**
  - Env override precedence (env > local > default).
  - Missing `local.toml` hata değil, default'a düşme.
  - Yanlış tip (örn. `cache.size_mb = "abc"`) decode hatası.
- **Manuel:**
  - `VISCOS__LOGGING__LEVEL=trace cargo run -p viscos` → trace logları görünüyor.
  - `cargo deny check licenses` → `config` MIT/Apache olarak geçiyor.

## Sınır durumları ve riskler

- **Yazma ihtiyacı:** Kullanıcı "Save settings" UI'ından config değiştirip kalıcı yapmak isterse → config-rs read-only. Mitigation: Kullanıcı ayarları SQLite'ta (Faz 4'te `viscos-cache`), app config salt okunur kalır. ADR-0003'te bu trade-off kabul edildi.
- **Env separator değişimi:** `__` Windows shell'lerde sorunlu olabilir. Mitigation: PowerShell'de `$env:VISCOS__LOGGING__LEVEL = "debug"` test edildi; OK.
- **Profil/feature karışıklığı:** `convert-case` feature'ı key normalizasyonu için gerekli. Devre dışı bırakılırsa `logging.level` ile `logging.Level` aynı olmaz.
- **Stale 0.x crate:** `config` 0.x serisi, major versiyon 1.0 yok. ADR-0003'te trade-off kabul edildi; piyasadaki standart. Major bump olursa migration packet gerekir.

## Review trigger'ları

- `config` 1.0 major çıkarsa (API breaking).
- Kullanıcı write-back talep ederse (v2'de değerlendirilir; `confy` veya `figment` geçişi).
- Lisans değişirse (GPL-3.0 uyumu).

## Cross-references

- **ADR:** ADR-0001 (workspace), ADR-0007 (error tipi `ViscosError::Config`).
- **Plan:** [`phase-0.0-foundation.md` § 3](../../.cursor/plans/phase-0.0-foundation.md).
- **Alternatifler:** `figment` (stale), `figment2` (topluluk fork), `confy` (GPL-3.0+ lisans kontaminasyonu), `serde_yaml` (manuel layer) — hepsi elendi.
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Hayır.** ADR-0003 net bir dependency seçimi; alternatifler elenmiş, lisans uyumu net. AI yazar, CI doğrular. Yeni config section eklenirse (örn. Faz 6'da hotkeys için) bilgilendirme yeterli.
