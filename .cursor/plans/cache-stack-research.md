---
name: Cache Stack — Detaylı Araştırma (Haziran 2026)
overview: ADR-0010 için derlenen araştırma kayıtları. RAM cache (moka, stretto, quick_cache), disk cache (foyer, CacheLib, BlobCache, Possum, redb), DB (rusqlite, SQLx, redb, sled), encryption trade-off'ları, Discord CDN URL politikası, iş yükü analizi, trade-off tabloları.
isProject: false
---

# Cache Stack — Detaylı Araştırma (Haziran 2026)

> **Bu dosya:** ADR-0010 ve `phase-4.0-cache-media.md` için derlenen araştırma kayıtları.
> **Karar:** [`docs/DECISIONS.md` ADR-0010](../../docs/DECISIONS.md)
> **İlgili plan:** [`phase-4.0-cache-media.md`](./phase-4.0-cache-media.md)

---

## 1. Viscos İş Yükü (Neyi Optimize Ediyoruz?)

Discord istemcisi olarak cache ihtiyacımız **3 farklı kategoriye** ayrılıyor. Her biri farklı trade-off gerektiriyor:

### 1.1 Mesaj Metadata (JSON, küçük)

| Özellik | Değer |
|---|---|
| Tipik boyut | ~150–600 byte/msg (Discord compressed payload ~166 byte, gzip ile ~270 byte) |
| Erişim paterni | Sıcak kanal scroll (aynı mesaja 100+ erişim); soğuk kanal (1–3 erişim sonra unutulur) |
| Kalıcılık | Yüksek (offline okuma, scroll history) |
| Schema | Relational (guild_id, channel_id, timestamp DESC, author_id, vb.) |

**Discord internal verisi:** "Average user is in ~10-15 servers; most active in 3-5". Yani kullanıcının %60-80 erişimi 3-5 kanalda. **OLTP-style hot set + frequency skew.**

### 1.2 Kullanıcı/Guild Metadata (imaj, küçük)

| Özellik | Değer |
|---|---|
| Tipik boyut | 80KB – 1MB (avatar), 256KB – 2MB (banner), emoji 64–256KB |
| Erişim paterni | Çok sıcak (her mesajda author avatar, her kanalda guild icon) |
| Kalıcılık | Orta (Discord CDN URL **imzasız**, expire olmaz) |
| Boyut trendi | WebP optimizasyonu ile ortalama 200KB compressed |

### 1.3 Attachment/Sticker/Video (büyük, signed)

| Özellik | Değer |
|---|---|
| Tipik boyut | 200KB – 25MB (Nitro 50MB / 500MB) |
| Erişim paterni | Soğuk–ılık (bir mesaja 1–5 erişim, sonra unutulur) |
| Kalıcılık | Yüksek (içeride Discord yeniliyor, dışarıda **24 saat signed URL expire**) |
| URL stratejisi | Content-addressable (snowflake) + background refresh |

**Karışık pattern özeti:** sıcak metadata + sıcak avatar/icon + soğuk attachment → **3-katmanlı cache mimarisi gerekli** (SQLite + RAM + disk).

---

## 2. RAM Cache Alternatifleri

### 2.1 moka 0.12.15 (Mart 2026) — Mevcut Seçim

**Library bilgisi:**
- 2474 GitHub star, 574 reverse dependency, **89M+ total downloads** (crates.io)
- Latest: v0.12.15 (2026-03-22), MSRV 1.71.1
- License: MIT OR Apache-2.0
- Maintenance: Aktif (40 contributor), son commit Mart 2026

**Teknik özellikler:**
- **TinyLFU admission + LRU eviction** (Caffeine-Java ekolü)
- v0.12.0 (Ekim 2025): **background thread kaldırıldı**, lock-free foreground maintenance
- API stabilization: `get_with`, `try_get_with`, `insert`, `eviction_listener`
- Per-entry variable expiration (hierarchical timer wheel)
- Hit ratio production kanıtı: **crates.io kendi API service'inde ~%85 hit rate** (download endpoint, Kasım 2021'den beri)

**Roadmap'te bekleyen:**
- Cache statistics (Hit rate, miss rate API)
- W-TinyLFU geçişi (PR'da hill climbing aktif, [#249](https://github.com/moka-rs/moka/issues/249))
- Restore cache from snapshot

**Viscos için uygunluk:** ✅ İyi — Discord scroll pattern'i TinyLFU ile uyumlu (frequency skew), background thread kaldırılması CPU idle hedefi (<%1) için pozitif.

### 2.2 stretto 0.9.0 — CacheBench'de Parlıyor

**Library bilgisi:**
- CacheLib/Ristretto'nun Rust portu
- 64-stripe insert buffer (16 concurrent client benchmark)
- License: Apache-2.0

**Cachebench OLTP trace sonuçları (Discord read-state pattern'i en yakın senaryo):**

| Capacity | QuickCache | Stretto | Moka Sync |
|---|---|---|---|
| 256 | 22.28% | **75.74%** | 26.40% |
| 512 | 28.79% | **76.02%** | 32.71% |
| 1,000 | 35.12% | **76.13%** | 38.02% |
| 2,000 | 41.68% | **76.39%** | 42.99% |

**Stretto OLTP'ta %33–47 pp önde** (cap=256: %75.7 vs %26.4; cap=2000: %76.4 vs %43.0).

**AMA scan-heavy trace'lerde:**

> "When either fails (DS1 at 4M+, S1/S2 at 800K, P14, ConCat, MergeP, MergeS, the 160K rows of P4/P6/P7/P11/P12), admission rejects items LRU/W-TinyLFU would have kept and Stretto trails by up to 57 points."

**Quick-cache'in itirazı ([issue #49](https://github.com/arthurprs/quick-cache/issues/49)):** Mokabench'te Stretto %12 hit ratio (cap=100K, 1 client) — Stretto'nun cachebench iddiaları sorgulanıyor. **İki benchmark farklı sonuçlar** → dikkatli değerlendirilmeli.

**Viscos için:**
- Avantaj: Sıcak kanal scroll'da %33+ pp hit ratio potansiyeli
- Risk: Soğuk kanal scroll'da %57 pp kayıp
- Discord iş yükü **karışık pattern** (3-5 sıcak kanal + birçok orta kanal)

**Karar:** moka v1'de korunur, **Stretto PoC stretch goal olarak Faz 4 sonuna** alındı (ADR-0010). Gerçek workload benchmark'ı (>15 pp hit ratio farkı) → v2 backlog'a al.

### 2.3 quick_cache 0.6 — En Küçük Footprint

- S3-FIFO policy, en düşük overhead
- **TTL/TTI yok** → Discord'un 1h TTL/5dk TTI planına uymuyor, kendin implement etmek AI-yazar riski
- Per-key expiration yok
- Cachebench'de moka ile yakın, Stretto'nun gerisinde

**Karar:** Elendi (TTL/TTI eksikliği).

### 2.4 mini-moka

- moka'nın single-thread versiyonu
- Async yok, **Viscos'un tokio mimarisiyle uyumsuz**

**Karar:** Elendi.

### 2.5 foyer pure-memory modu

- foyer docs'a göre in-memory modda **moka'dan hızlı**, quick-cache'den çok az yavaş
- Avantaj: eğer foyer disk katmanı olarak kullanılacaksa **unified API** (HybridCache)
- Dezavantaj: moka'nın eviction listener API'si daha zengin (Discord IPC invalidation için)

**Karar:** Değerlendirilebilir alternatif ama ayrı bir ADR gerektirir. moka + foyer iki-engine bakım yükü kabul edilebilir.

---

## 3. Disk Cache Alternatifleri

### 3.1 foyer 0.22.3 (Ocak 2026) — Mevcut Seçim

**Library bilgisi:**
- 1673 GitHub star, 30 contributor
- 382K downloads (memory), 379K (hybrid)
- Latest: v0.22.3 (2026-01-23), MSRV 1.85
- License: Apache-2.0
- Maintenance: Aktif (son push 2026-03-05, "still under heavy development")

**Production kanıtı:**
- **RisingWave** (streaming database, S3-backed state) — p99 latency 125ms → 15ms
- **Chroma** (LLM embedding database)
- **SlateDB** (cloud native embedded storage)
- **ZeroFS**, **Percas** (distributed persistent cache)

**Teknik özellikler:**
- **Hybrid cache**: memory + disk unified
- **Pluggable eviction algorithms**: w-TinyLFU, LRU, S3-FIFO
- **Pluggable disk engines**: Block (4KiB–1GiB), Set-Associated (WIP, ~4KiB), Object (WIP, ≥1MiB)
- **Pluggable IO engines**: Psync (default, blocking pread/pwrite + thread pool), Libaio (WIP), Uring (Linux)
- **Zero-copy in-memory abstraction** (intrusive collection)
- `get_or_fetch()` API: concurrent miss deduplication

**Windows kısıtları:**
- Linux io_uring engine'i Windows'ta yok (WIP)
- NTFS'te sadece psync mevcut
- **p50 latency'de %30+ overhead** (Linux io_uring kazancı Windows'ta elde edilemiyor)
- File system journal overhead (her yazımda fsync)

**Viscos için:**
- Avantaj: RisingWave gibi ciddi scale kanıtı, pluggable engine mimarisi Discord'un değişken iş yüküne uyumlu
- Risk: Windows NTFS overhead kabul edilmeli, API breaking (0.x) riski

**Karar:** Korunur.

### 3.2 CacheLib (Meta C++) + Rust binding

**Avantajları:**
- OSDI'20 paper, Meta production (TAO backend cache)
- En olgun hybrid cache implementasyonu

**Eksileri:**
- **C++ build dependency** → Viscos 25 MB binary bütçesini zorlar (+10–15 MB)
- foyer author'ın blog'unda açıkça: "foyer provides a better optimized storage engine implement over CacheLib"
- Rust binding **sınırlı interface** (logging, metrics, tracing için C++ patch gerekli)

**Karar:** Elendi (binary bütçesi + Rust-native foyer tercihi).

### 3.3 BlobCache (Go native)

- Disk-first FIFO, cachebench'te %20 daha hızlı (1.21 vs 1.0 GB/s)
- **Rust port'u yok** → kullanılamaz

**Karar:** Elendi.

### 3.4 Possum

- Multi-process concurrent access, hole-punching + sparse files
- **Tek-process, Windows-first** proje için overkill
- Linux-optimized

**Karar:** Elendi.

### 3.5 cacache (npm altyapısı)

- Content-addressable disk cache, yüksek performans
- npm-internal tasarım, **SHA-1 hash key** zorunluluğu
- Discord CDN URL'leri ile uyumsuz (key'i sen seçemezsin)

**Karar:** Elendi (Discord CDN ile uyumsuz key stratejisi).

### 3.6 redb + LRU memory tier

**Library bilgisi:**
- Pure-Rust B-tree (LMDB-inspired), MVCC
- **Tek dosya** (RocksDB gibi dizin değil)
- License: MIT/Apache-2.0

**Benchmark (lmsbench, Ryzen 9950X3D + Samsung 9100 PRO NVMe):**

| Workload | redb | lmdb | rocksdb | sled | sqlite |
|---|---|---|---|---|---|
| bulk load | 17063ms | **9232ms** | 13969ms | 24971ms | 15341ms |
| individual writes | **920ms** | 1598ms | 2432ms | 2701ms | 7040ms |
| random reads (1 thread) | 1138ms | **637ms** | 2911ms | 1601ms | 4283ms |
| random reads (32 thread) | 410ms | **125ms** | 1100ms | 444ms | 576ms |
| removals | 23297ms | 10435ms | **6900ms** | 11088ms | 10423ms |
| compacted size | 1.69 GB | 1.26 GB | **454 MB** | N/A | 556 MB |

**Avantajları:**
- redb individual writes'ta en iyi (920ms)
- Pure Rust, tek dosya, **build kolaylığı**
- Foyer'ın yaptığı işi (memory + disk hybrid) tek depolama ile yapabilir

**Eksileri:**
- **SQL yok** → message pagination, FTS, complex query için yetersiz
- Eviction policy (TinyLFU vs LRU vs W-TinyLFU) kendin yaz → **AI-yazar riski**
- Compression kendin implement et
- Eviction listener yok

**Viscos için değerlendirme:**
- Eğer sadece KV cache yapacaksan (Discord raw JSON message_id → message) redb mantıklı
- AMA Discord'un gelecekte FTS ihtiyacı (Faz 5+) için SQLite gerekli
- v1 için foyer + moka kanıtlanmış kombinasyon daha güvenli

**Karar:** v1'de elendi (SQLite gerekli, eviction listener Discord IPC invalidation için). Foyer Windows overhead'i ciddi sorun olursa **v2 fallback** olarak değerlendirilebilir.

### 3.7 Diğer (fjall, sled, lmdb-sys, hana)

| Kütüphane | Durum | Karar |
|---|---|---|
| **fjall** | LSM-based pure Rust, hızlı batch write ama daha az kanıt | Elendi |
| **sled** | "Alpha" warning, son commit Ekim 2024 → **stable değil** | Elendi |
| **lmdb-sys** | mmap tabanlı, **NTFS transaction modeli ile conflict riski**, bounded memory map zorunluluğu | Elendi |
| **hana** | Düşük seviye, B+tree custom, kullanım riski yüksek | Elendi |

---

## 4. Mesaj Geçmişi DB Alternatifleri

### 4.1 rusqlite 0.38 — Mevcut Seçim

**Library bilgisi:**
- SQLite için Rust binding (C-FFI)
- Production kanıtı: **Delta Chat** (async %20–30 perf regression ölçtü, rusqlite'a geri döndü), Tauri apps

**Avantajları:**
- Discord mesaj metadata için **relational ihtiyaç var** (channel_id + timestamp DESC, pagination)
- JOIN'ler (message ↔ author ↔ attachment ↔ reaction)
- **FTS5** (gelecekte mesaj araması)
- UPDATE/DELETE atomik
- WAL mode + refinery migration (Rust-friendly)

**Karar:** Korunur.

### 4.2 SQLx

- Async-first, compile-time query check
- Delta Chat **async %20–30 perf regression** ölçtü
- C-FFI rusqlite ile aynı binary overhead
- Compile time daha yavaş

**Karar:** Elendi.

### 4.3 redb

- SQL yok → message pagination + FTS imkansız
- Discord raw JSON message_id → message için mantıklı ama gelecek FTS ihtiyacı için yetersiz

**Karar:** Elendi.

### 4.4 Limbo (Turso)

- v0.0.22, çok erken (Şubat 2026)
- SQLite fork'u, async-first Rust
- Production-ready değil, çok az kullanıcı

**Karar:** Elendi.

### 4.5 DuckDB / Stoolap

- OLAP engine'ler, OLTP workload için yanlış tool
- Discord mesaj erişimi OLTP

**Karar:** Elendi.

---

## 5. Encryption (AES-GCM) Trade-off Analizi

### 5.1 Mevcut Karar: AES-GCM

| Karar | Gerekçe |
|---|---|
| `aes-gcm` 0.10 | NIST standart AEAD |
| `chacha20poly1305` çıkarıldı | +60 KB binary tasarruf, Win10/11 x86_64'te AES-NI yaygın |

**Win10/11 AES-NI coverage:**
- Intel: 2010'dan beri (Westmere) tüm CPU'larda
- AMD: 2011'den beri (Bulldozer) tüm CPU'larda
- Hedef kitle %99 AES-NI var

**Software fallback gereksiz → tek AEAD yeterli.**

### 5.2 Trade-off Tablosu

| Algoritma | Avantaj | Dezavantaj | Viscos uygunluğu |
|---|---|---|---|
| **AES-GCM** | Hardware accel (AES-NI), ~3-5 GB/s, NIST standart | Nonce reuse catastrophic | ✅ Mevcut |
| **AES-GCM-SIV** | Nonce-misuse resistant | %30 yavaş, gereksiz | ❌ Overkill |
| **chacha20poly1305** | ARM hızlı, nonce reuse resilient | Win x86'da gereksiz, +60 KB | ❌ Çıkarıldı |
| **XChaCha20-Poly1305** | 24-byte nonce, misuse resistant | Software-only | ❌ Gerek yok |

**v3'te cross-platform (Linux ARM) eklenirse chacha20poly1305 geri gelebilir** (ADR-0010 gözden geçirme tetikleyicisi).

---

## 6. Compression Analizi

### 6.1 Medya için Compression Anlamsız

| Format | Zaten compressed? | Re-compress? |
|---|---|---|
| PNG / JPEG / WebP | Evet (lossy/lossless) | Hayır |
| Video (H.264/H.265/AV1) | Evet (codec) | Hayır |
| Audio (Opus) | Evet | Hayır |

**CPU cost + %0-2 boyut azalması + complexity = negatif ROI.**

### 6.2 Mesaj JSON için Compression Tartışmalı

- 166 byte compressed payload (Discord zstd-stream) → biz raw JSON saklıyoruz (~300-500 byte)
- `zstd::encode_all(msg, level=3)` ile %60-70 küçülme
- AMA **search/FTS5** için compression index'i bozar
- Sadece attachment binary'lerde opsiyonel zstd level 1 (lightweight) → RAM + disk cache boyutu %40 düşer

**Öneri:** Mesaj JSON'unda compression YAPMA (search/FTS açık), attachment binary'lerde opsiyonel.

---

## 7. Discord CDN URL Politikası (KRİTİK)

### 7.1 Discord Davranışı (Ekim 2023 → 2026 aktif)

| Kaynak türü | Signed? | Expire? | Cache key stratejisi |
|---|---|---|---|
| **Avatar, banner, guild icon** | ❌ | ❌ Asla | SHA-256(url) → moka RAM |
| **Emoji, role icon** | ❌ | ❌ Asla | SHA-256(url) → moka RAM |
| **User-uploaded attachment** | ✅ (ex + is + hm) | ✅ 24 saat | **attachment_id (snowflake) → foyer KV** |
| **Sticker (Nitro)** | ❌ | ❌ Asla | SHA-256(url) → moka RAM |
| **Soundboard sound** | ❌ | ❌ Asla | SHA-256(url) → moka RAM |

**Signed URL params:**
- `ex`: hex timestamp (expire)
- `is`: hex timestamp (issued)
- `hm`: HMAC-SHA256 signature

### 7.2 Cache Key Stratejisi Trade-off

**Strateji A: URL-as-key**
- Avantaj: Basit, native HTTP cache semantiği
- Dezavantaj: **24 saatte tüm attachment cache invalid olur** → kullanıcı 1+ günlük eski mesajda attachment'a tıklarsa re-download
- Disk cache boyutu efektif olarak 24 saatlik window

**Strateji B: Content-Addressable (Discord `attachment_id` snowflake)** ✅ **ÖNERİLEN**
- Avantaj: 24 saat limit'i cache ömrünü sınırlamaz, **cache ölümsüz**, disk boyutu efektif tüm history
- Dezavantaj: 2 katmanlı cache (signed URL moka'da, blob foyer'da), refresh worker complexity
- Avant-garde cache invalidation yok — "stable identity" prensibi

### 7.3 Discord'un Kendi Yenileme Mekanizması

- **Gateway event'leri zaten refreshed URL gönderiyor** (REST response'ları da)
- Yani mesaj alındığında URL geçerli
- **1+ gün önceki mesajlar**: URL expire olmuş olabilir → `POST /channels/{id}/attachments/refresh-urls` (50 URL per call)

### 7.4 CDN Refresh Worker (Background)

```rust
// Her saat başı çalışır:
// 1. moka metadata'dan expires_at 23-24h aralığında olanları seç
// 2. channel_id'ye göre grupla
// 3. Her grup için POST /attachments/refresh-urls (50 per call)
// 4. Yeni URL'i moka'ya yaz (encrypted blob cache'inde değişiklik yok)
// 5. Rate-limit koruması: 100ms sleep per batch
```

**Avantajları:**
- Lazy fetch (sadece kullanıcı tıkladığında URL refresh) + background pre-warming
- 24 saat signed URL limit'i invisible to user
- Discord rate-limit uyumlu

---

## 8. Disk-First vs Memory-First Mimari (Discord-Spesifik)

### 8.1 Erişim Pattern Analizi

| Pattern | Yüzde | Cache stratejisi |
|---|---|---|
| **Sıcak scroll (aynı kanal)** | %60-80 | LFU/TinyLFU memory tier (moka) |
| **Avatar/icon (her mesajda)** | %15-20 | RAM cache (küçük, imzasız URL) |
| **Soğuk kanal scroll** | %5-10 | Disk cache (foyer) |
| **Attachment tıklama** | %2-5 | Disk cache + lazy URL refresh |

### 8.2 Tier Boyutları (v1 Default)

| Tier | Boyut | İçerik |
|---|---|---|
| moka (RAM metadata) | 64 MB | Sıcak mesaj + üye lookup |
| moka (RAM URL meta) | ~5 MB | Signed URL + expiry (~50K entry) |
| foyer memory tier | 32 MB | Sıcak blob chunk |
| foyer disk tier | 10 GB | Tüm attachment blob (encrypted) |
| SQLite | Sınırsız (~disk) | Mesaj + metadata + telemetry |

**300 MB RAM hedefi (Resmi Discord 500-1500 MB):**
- moka 64 MB + URL meta 5 MB + foyer memory 32 MB = ~101 MB cache overhead
- Viscos shell + WebView + IPC = ~150 MB
- Toplam ~250 MB → 300 MB hedefe sığar

### 8.3 Adaptive Tier Sizing (Faz 1.5)

| Tier | v1 Default | Adaptive Trigger | Yeni Değer |
|---|---|---|---|
| moka (metadata) | 64 MB | Hit ratio <%70 | 2× (128 MB) max 256 MB |
| moka (metadata) | 64 MB | Hit ratio >%95 | ÷2 (32 MB) min |
| foyer memory | 32 MB | Disk hit ratio >%60 | 2× (64 MB) max 128 MB |
| foyer disk | 10 GB | Disk hit ratio <%40 | 2× (20 GB) max 25 GB (user cap) |

**Opt-out:** `config.cache.tier.auto_tune = true` (default v1.5'te true).

---

## 9. Foyer vs Redb (Fallback Senaryosu)

Eğer foyer'ın Windows NTFS overhead'i kabul edilemez olursa:

| | foyer 0.22 | redb + LRU memory |
|---|---|---|
| RAM tier | Built-in (w-TinyLFU) | Custom LRU (AI-yazar riski) |
| Disk tier | Hybrid, zero-copy | Tek dosya, MVCC |
| Eviction listener | Evet (Discord IPC invalidation) | Custom (AI-yazar riski) |
| Compression | Pluggable (Lz4 built-in) | Custom |
| Serialization | `Code` trait (serde) | Custom (bincode/rmp) |
| Production kanıtı | RisingWave + Chroma + SlateDB | redb tek başına, prod kanıtı sınırlı |
| Build complexity | Psync engine + thread pool | Minimal |

**Karar:** v1'de foyer korunur (production kanıtı). Redb fallback **v2 backlog**'ta değerlendirilir.

---

## 10. Rakiplerden Öğrenilenler (Cache Spesifik)

### 10.1 Discord Resmi (Electron)

- Memory leak nedeniyle **4 GB RAM'de auto-restart** (Aralık 2025)
- Image cache için `webFrame.clearCache()` (Electron API)
- %80+ RSS kullanımına kadar "free memory" yiyor (Electron açığı)

**Viscos dersi:** WebView2/CEF + native Rust shell sayesinde baseline memory 200-300 MB → 4 GB restart threshold'una asla ulaşılmaz. Viscos'un gerçek restart tetikleyicisi **GDI watchdog** (7000/9000), RAM değil.

### 10.2 Vesktop (Electron + Vencord)

- localStorage ile küçük ayarlar (`hideNag`, plugin config)
- IndexedDB opsiyonel ama **10 MB üzeri OOM riski** (BD compatibility layer)
- Discord'un kendi IndexedDB'sini kullanıyor (Viscos'un aksine ek cache layer yok)

**Viscos dersi:** Disk cache (foyer) ile RAM baskısı olmadan 50K+ attachment saklanabilir. IndexedDB OOM riski yok.

### 10.3 Element Desktop (Matrix)

- electron-store + IndexedDB (messages/events)
- Seshat event indexer (SQLite-based, native)
- matrix-rust-sdk'ya geçiş (encrypted storage)

**Viscos dersi:** matrix-rust-sdk unified storage yaklaşımı Viscos'un `viscos-cache` crate yapısına benzer (multi-tier unified API). Element WebView2/CEF native UI olmadığı için bizden farklı.

### 10.4 Tauri uygulamaları

- `tauri-plugin-redb-cache` (LRU + redb + zlib): 2-tier pattern, **bizim foyer yerine redb tercihi**
- HTTP Range request desteği (büyük medya streaming)

**Viscos dersi:** Tauri ecosystem redb pattern'i tercih ediyor ama Viscos'un 25 MB binary bütçesi + Rust-native kararı foyer'a yönlendiriyor.

---

## 11. Alınan Dersler ve Sonuç

### 11.1 Mevcut Karar Doğrulama

ADR-0010'un 3-katmanlı mimarisi (SQLite + moka + foyer) **doğru temel**. Alternatifler elendi:

| Kategori | Seçim | Neden |
|---|---|---|
| DB | rusqlite 0.38 | SQL ihtiyacı, Delta Chat kanıtı, AI-yazar riski en düşük |
| RAM cache | moka 0.12 | TinyLFU Discord OK, crates.io %85 hit ratio, W-TinyLFU yolda |
| Disk cache | foyer 0.22 | RisingWave/Chroma/SlateDB kanıtı, pluggable engine |
| Encryption | AES-GCM | AES-NI yaygın, +60 KB tasarruf |

### 11.2 Yeni Eklenen Kararlar (Haziran 2026)

1. **Content-Addressable Cache Key**: `attachment_id` snowflake → foyer KV, signed URL moka metadata'da
2. **CDN Refresh Worker**: Background task, 23-24h aralığında URL yenileme, 50 per call + rate-limit
3. **Adaptive Tier Sizing**: Faz 1.5 telemetry-driven, opt-out flag

### 11.3 Açık Sorular (v2 backlog)

- Stretto vs moka workload-specific benchmark (Faz 4 sonu stretch)
- W-TinyLFU moka'ya geldiğinde upgrade
- Redb+LruCache fallback (foyer Windows overhead'i ciddi sorun olursa)
- Adaptive tier sizing v2 (runtime reconfigure, hysteresis)

---

## 12. Referanslar

- [moka GitHub](https://github.com/moka-rs/moka)
- [foyer GitHub](https://github.com/foyer-rs/foyer)
- [stretto cachebench OLTP benchmark](https://docs.rs/crate/stretto/latest)
- [redb README + benchmark](https://github.com/cberner/redb/)
- [Discord CDN signed URL security](https://www.bitdefender.com/en-us/blog/hotforsecurity/discord-tightens-security-with-temporary-file-links)
- [Discord Chat Exporter CDN expiration issue](https://github.com/Tyrrrz/DiscordChatExporter/issues/1266)
- [Discord internal storage article](https://sujeet.pro/articles/discord-message-storage)
- [Discord WebSocket zstd optimization](https://discord.com/blog/how-discord-reduced-websocket-traffic-by-40-percent)
- [Foyer blog: Past, Present, and Future](https://blog.mrcroxx.com/posts/foyer-a-hybrid-cache-in-rust-past-present-and-future/)
- [Tauri cache plugin](https://github.com/bishen/tauri-plugin-redb-cache)
- [Discord auto-restart 4GB (Windows 11)](https://www.windowslatest.com/2025/12/06/discord-admits-its-windows-11-app-is-a-resource-hog-tests-auto-restart-when-ram-usage-exceeds-4gb/)
