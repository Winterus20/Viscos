# Implementation Packet — ADR-0010: Cache Stack — SQLite + moka + foyer (Varyant A)

## Header

- **ADR:** ADR-0010
- **Başlık:** Cache Stack — SQLite + moka + foyer (Varyant A, 2026 Q2)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18 (Haziran 2026 trade-off analizi revizyonu ile)
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0010](../../docs/DECISIONS.md#adr-0010-cache-stack--sqlite--moka--foyer-varyant-a-2026-q2)
- **Önceki plan:** [`phase-4.0-cache-media.md`](../../.cursor/plans/phase-4.0-cache-media.md), [`cache-stack-research.md`](../../.cursor/plans/cache-stack-research.md)

## Hedef faz worker

**Cache+Media worker, Faz 4.0, Dalga 1.** `viscos-cache` ve `viscos-media` crate'lerinin kurulumu. Bu packet, `phase-4.0-cache-media.md` § 1-2'yi uygular. Haziran 2026 trade-off revizyonu (CDN content-addressable + adaptive tier sizing) bu packet'in iki ayrı sub-section'ıdır (Dalga 2 ve Dalga 3).

## Uygulama adımları

### Dalga 1 — Temel stack (Faz 4.0 ilk 1 hafta)

1. **`Cargo.toml` `[workspace.dependencies]`** — cache dependency'leri:
   ```toml
   rusqlite = { version = "0.38", features = ["bundled", "blob"] }
   r2d2 = "0.8"
   r2d2_sqlite = "0.24"
   refinery = { version = "0.8", features = ["rusqlite"] }
   moka = { version = "0.12", features = ["future"] }
   foyer = "0.22"
   aes-gcm = "0.10"
   argon2 = "0.5"
   tikv-jemallocator = "0.5"  # koşullu benchmark
   ```
   - `chacha20poly1305` **YOK** (cleanup, ADR-0010 Decision).

2. **`crates/viscos-cache/`** workspace'e ekle:
   - `Cargo.toml`: yukarıdaki bağımlılıklar + `viscos-core`, `viscos-error`.
   - `src/db.rs`: rusqlite + r2d2 pool, WAL mode.
   - `src/migrations/`: refinery migration'ları (V001__initial.sql: messages, guilds, channels, members, read_state).
   - `src/cache.rs`: moka future cache wrapper.
   - `src/error.rs`: `CacheError` enum + `From` impl'leri.

3. **`crates/viscos-media/`** workspace'e ekle:
   - `Cargo.toml`: foyer, moka (url_meta için), aes-gcm, argon2, `viscos-cache`, `viscos-error`.
   - `src/cache.rs`: `MediaCache` struct (ADR-0010 Decision §A).
   - `src/encryption.rs`: AES-GCM encrypt/decrypt.

4. **Encryption anahtar yönetimi (Varyant A, default)**:
   - `keyring-core` + `windows-native-keyring-store` (ADR-0011'de eklenecek) → encryption key otomatik OS-bound (DPAPI).
   - `keyring::Entry::new("Viscos", "cache_encryption_key")` ile 256-bit key al.

5. **Test:**
   - `tests/cache_round_trip.rs`: 10K mesaj insert + query + delete.
   - `tests/moka_concurrent.rs`: 16 concurrent read/write hit ratio.
   - `tests/foyer_disk_eviction.rs`: 10 GB cache'e 15 GB yaz → eviction OK.

### Dalga 2 — CDN Content-Addressable (Haziran 2026 eki)

6. **`crates/viscos-media/src/cache.rs`** — `MediaCache` ADR-0010 Decision §A'ya göre:
   ```rust
   pub struct MediaCache {
       blobs: foyer::HybridCache<u64, EncryptedMediaBlob>,  // key = snowflake
       url_meta: moka::future::Cache<u64, CdnUrlMeta>,      // 1h TTL
   }
   ```

7. **`crates/viscos-media/src/refresh.rs`** — `CdnRefreshWorker` (23h < expires_at < 24h olanları batch'le, 50 per call).

8. **Test:**
   - `tests/cdn_content_addressable.rs`: aynı attachment_id 2 kez yazılırsa cache hit (URL değişse bile).
   - `tests/cdn_refresh_worker.rs`: 23h'de olan URL'ler refresh queue'ya girer.

### Dalga 3 — Adaptive Tier Sizing (Faz 1.5 telemetry entegrasyonu)

9. **`crates/viscos-cache/src/tier.rs`** — telemetry-driven sizing:
   - Default: moka 64 MB, foyer memory 32 MB, foyer disk 10 GB (ADR-0010 Decision §B tablosu).
   - v1'de statik; Faz 1.5'te telemetry backend hazır olunca adaptive.
   - Opt-out: `cache.tier.auto_tune = false`.

10. **Doğrulama**:
    - `cargo test -p viscos-cache` → 10+ integration test geçer.
    - `cargo test -p viscos-media` → CDN refresh test'leri geçer.
    - 24h soak: 1000+ mesaj, 50+ attachment → cache hit ratio >%70 (moka), >%40 (foyer disk).

## Kabul kriterleri

- ✅ `viscos-cache` + `viscos-media` crate'leri workspace member.
- ✅ `rusqlite 0.38`, `moka 0.12`, `foyer 0.22`, `aes-gcm 0.10` declare edilmiş.
- ✅ `chacha20poly1305` **YOK** (cargo deny ile doğrula).
- ✅ `lto = "fat"` sonrası binary 25 MB altında.
- ✅ WAL mode + connection pool (r2d2) çalışıyor.
- ✅ Refinery migration'ları çalışıyor (down + up).
- ✅ `MediaCache` content-addressable (key = snowflake, URL değil).
- ✅ `CdnRefreshWorker` rate-limit aware (50 URL per call).
- ✅ Adaptive tier sizing v1'de statik, v1.5'te aktif (opt-out destekli).

## Test stratejisi

- **Unit:**
  - `tests/db_pool.rs`: 16 concurrent connection acquire < 10ms.
  - `tests/moka_eviction.rs`: capacity aşımı → LRU eviction.
  - `tests/foyer_recovery.rs`: process restart sonrası cache intact.
- **Integration:**
  - 10K mesaj insert + 10K query (random access): < 100ms p99.
  - 1K attachment yaz + oku: encrypted, integrity OK.
  - Discord test hesabı ile: mesaj scroll, attachment cache hit.
- **Manuel (Faz 4.0 sonu):**
  - 24h soak: 1000+ mesaj + 50+ attachment → restart 0.
  - Hit ratio dashboard (SQLite query): moka >%70, foyer >%40.
  - Binary size kontrolü (`cargo bloat`): 25 MB altında.
  - `chacha20poly1305` cargo tree'de yok (doğrula).

## Sınır durumları ve riskler

- **foyer 0.10 → 0.22 minor bump API breaking:** Orta risk (0.x → 0.x). Mitigation: İlk implementasyon 0.22 ile, AI-yazar'a migration doc.
- **foyer Windows NTFS overhead:** Linux io_uring yok, sadece psync → %30 cold latency. Mitigation: Modern NVMe SSD'lerde throughput yeterli; cold latency kabul.
- **Stretto PoC ertelendi:** Stretch goal, Faz 4 sonu cachebench benchmark. Şu an erken değişiklik riski alınmıyor.
- **`chacha20poly1305` çıkarma:** ARM/Linux fallback yok. v1 Windows-only OK; v3'te cross-platform gündeme gelirse yeniden değerlendirilir.
- **CDN refresh worker complexity:** Background task, rate-limit aware, gateway state aware. AI-yazar riski orta. Mitigation: ADR referansı + unit test coverage >%80 zorunlu.
- **Adaptive tier sizing bug:** Cache thrashing riski. Mitigation: `auto_tune = false` opt-out default true (v1.5'te sadece telemetry dinler, tier değiştirmez).
- **AES-NI yok:** Eski CPU'larda (pre-Haswell 2013) AES-GCM yavaş. Mitigation: v1 modern Win10/11 hedef, sorun yok.
- **Migration rollback:** Refinery down migration yazmak zorunlu (schema değişikliğinde).

## Review trigger'ları

- foyer 1.0 major versiyon çıkarsa (API breaking).
- moka hit ratio telemetry verisi düşükse (Stretto PoC tetiklenir, %15+ pp fark varsa v2 backlog).
- Binary bütçesi 25 MB aşılırsa (rusqlite → redb değerlendirmesi, SQL kaybı kabul edilirse).
- Cross-platform hedef eklendiğinde (chacha20poly1305 geri gelebilir).
- moka 0.12'de güvenlik açığı / bakım duraksaması.
- Discord CDN signed URL TTL değişirse (24h → farklı).
- Adaptive tier sizing telemetry thrashing gösterirse (v1.5'te alarm).

## Cross-references

- **ADR:** ADR-0001 (workspace), ADR-0005 (binary bütçesi), ADR-0011 (encryption anahtarı, Varyant A keyring).
- **Plan:** [`phase-4.0-cache-media.md`](../../.cursor/plans/phase-4.0-cache-media.md), [`cache-stack-research.md`](../../.cursor/plans/cache-stack-research.md).
- **Alternatifler:** Stretto, mini-moka, quick_cache, redb, SQLx, Diesel/SeaORM, sled, Limbo, CacheLib Rust binding, BlobCache, Possum, DuckDB — hepsi elendi (ADR-0010 Consequences).
- **Telemetri:** Faz 1.5'te adaptive tier sizing için [`phase-1.5-telemetry-and-restart-optimization.md`](../../.cursor/plans/phase-1.5-telemetry-and-restart-optimization.md).
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Evet — Dalga 1 başlangıcında bir kez.** `viscos-cache` ve `viscos-media` crate'lerinin kurulumu, encryption anahtar yönetimi (keyring entegrasyonu), migration stratejisi — tüm bunlar mimari karar gerektirir. Dalga 2 (CDN content-addressable) ve Dalga 3 (adaptive tier sizing) PR review'unda yakalanabilir. **Stretto PoC tetiklendiğinde** ayrı insan onayı gerekir (algoritma değişimi).
