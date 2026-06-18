---
name: Phase 4.0 — Cache + Medya
overview: SQLite WAL mode (guilds, channels, messages, members, attachments), moka in-memory cache, foyer hybrid disk cache, viewport-prioritized image download, encryption at rest (AES-GCM), jemalloc benchmark ve geçiş kararı.
isProject: false
todos:
  - id: cache-crate
    content: viscos-cache crate oluştur
    status: pending
  - id: media-crate
    content: viscos-media crate (resim, video, ses cache)
    status: pending
  - id: sqlite-wal
    content: SQLite WAL mode kurulumu (rusqlite 0.38, ADR-0010)
    status: pending
  - id: schema
    content: Schema: guilds, channels, messages, members, attachments
    status: pending
  - id: moka-ram
    content: moka 0.12 in-memory cache (mesaj ID → Message)
    status: pending
  - id: foyer-disk
    content: foyer 0.22 hybrid cache (medya, blurhash) — minor bump 0.10→0.22 (ADR-0010)
    status: pending
  - id: viewport-priority
    content: Viewport-prioritized image download
    status: pending
  - id: encryption-at-rest
    content: Encryption at rest (AES-GCM, anahtar keyring'de; chacha20poly1305 çıkarıldı ADR-0010)
    status: pending
  - id: jemalloc-bench
    content: jemalloc allocator benchmark (heap fragmentation)
    status: pending
  - id: jemalloc-decision
    content: Jemalloc geçiş kararı (≥%15 azalma ise adopt)
    status: pending
  - id: sharedbuffer-impl
    content: WebViewBackend::post_shared_buffer implementasyonu (WebView2 SharedBuffer)
    status: pending
  - id: sharedbuffer-frontend
    content: frontend/bridge.ts getBinary<T>() API + releaseBuffer() cleanup pattern
    status: pending
  - id: sharedbuffer-bench
    content: JSON vs SharedBuffer benchmark (100 ardışık 80KB avatar fetch, >5× latency hedefi)
    status: pending
  - id: stretto-poc
    content: (STRETCH) cachebench benchmark: moka vs stretto, Discord OLTP trace, >15 pp hit ratio farkı varsa v2 backlog'a al (ADR-0010)
    status: pending
  - id: cdn-content-addressable
    content: Content-addressable attachment cache (attachment_id snowflake → foyer KV, signed URL moka metadata) (ADR-0010 Haziran 2026 eki)
    status: pending
  - id: cdn-refresh-worker
    content: CDN refresh worker: 23h'de batch POST /attachments/refresh-urls (rate-limit 50 per call)
    status: pending
  - id: adaptive-tier-sizing
    content: Adaptive tier sizing v1 (statik defaults + Faz 1.5 telemetry aggregate → tier tune)
    status: pending
---

# Phase 4.0 — Cache + Medya

> **Süre:** 2 hafta
> **Hedef:** Mesaj geçmişi, medya dosyaları, sunucu/kanal bilgisi cache'leniyor. Offline okuma. Viewport-prioritized medya.
> **Önceki faz:** [`phase-3.0-gateway.md`](./phase-3.0-gateway.md)
> **Sonraki faz:** [`phase-5.0-native-ui.md`](./phase-5.0-native-ui.md)

---

## 1. Mimari

```
┌──────────────────────────────────────────────────────────┐
│ viscos-core (types, events)                              │
└────────┬─────────────────────────────────────────────────┘
         │
   ┌─────┴──────┐
   ▼            ▼
┌──────┐  ┌────────┐
│ cache│  │ media  │
│      │  │        │
│ SQLite│  │ RAM+disk│
│ moka  │  │ foyer  │
└──────┘  └────────┘
```

**Per-account cache:** `%APPDATA%/Viscos/cache/{account_id}/`

---

## 2. Workspace Dependencies

> **ADR-0010** (Haziran 2026) ile güncellendi: `rusqlite 0.32 → 0.38` (Aralık 2025 patch birikimi), `foyer 0.10 → 0.22` (Ocak 2026 olgunlaşma), `chacha20poly1305` çıkarıldı (AES-NI olan Win10/11'de gereksiz). **Haziran 2026 trade-off eki:** Content-Addressable cache key (`attachment_id` snowflake, 24h signed URL limit'ten etkilenmez), CDN refresh worker (background batch 23-24h, 50 per call + rate-limit), adaptive tier sizing (Faz 1.5 telemetry-driven). Araştırma: [`cache-stack-research.md`](./cache-stack-research.md).

```toml
[workspace.dependencies]
# DB (minor bump: 0.32 → 0.38)
rusqlite = { version = "0.38", features = ["bundled", "blob"] }
r2d2 = "0.8"
r2d2_sqlite = "0.24"

# Migrations
refinery = { version = "0.8", features = ["rusqlite"] }

# Cache (moka korunur, foyer 0.10 → 0.22 minor bump)
moka = { version = "0.12", features = ["future"] }
foyer = "0.22"

# Encryption (chacha20poly1305 çıkarıldı, sadece AES-GCM)
aes-gcm = "0.10"
argon2 = "0.5"

# Allocator (benchmark için)
tikv-jemallocator = "0.5"
```

---

## 3. `viscos-cache` (SQLite + moka)

### 3.1 Schema (refinery migration)

```sql
-- crates/viscos-cache/migrations/V001__initial.sql

CREATE TABLE guilds (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT,
    owner_id TEXT,
    member_count INTEGER,
    features TEXT,  -- JSON array
    unavailable INTEGER DEFAULT 0,
    updated_at INTEGER NOT NULL  -- unix timestamp
);

CREATE TABLE channels (
    id TEXT PRIMARY KEY,
    guild_id TEXT,
    type INTEGER NOT NULL,  -- 0=text, 2=voice, 4=category, vb.
    name TEXT,
    topic TEXT,
    position INTEGER,
    parent_id TEXT,
    last_message_id TEXT,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_channels_guild ON channels(guild_id);
CREATE INDEX idx_channels_parent ON channels(parent_id);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    author_id TEXT NOT NULL,
    content TEXT,
    timestamp TEXT NOT NULL,
    edited_timestamp TEXT,
    pinned INTEGER DEFAULT 0,
    flags INTEGER DEFAULT 0,
    mentions TEXT,  -- JSON
    attachments TEXT,  -- JSON
    embeds TEXT,  -- JSON
    raw_json TEXT NOT NULL,  -- full payload
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_messages_channel_time ON messages(channel_id, timestamp DESC);
CREATE INDEX idx_messages_author ON messages(author_id);

CREATE TABLE members (
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    nickname TEXT,
    roles TEXT,  -- JSON array
    joined_at TEXT,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, guild_id)
);

CREATE TABLE attachments (
    -- id = Discord snowflake (stringified u64). STABLE IDENTITY — cache key
    -- Strateji: URL 24 saat signed, expire olur. id asla değişmez → cache
    -- key = attachment_id (content-addressable). URL sadece fetch anında alınır.
    id TEXT PRIMARY KEY,
    message_id TEXT,
    url TEXT NOT NULL,           -- şu an geçerli signed URL (refresh worker tarafından update)
    url_expires_at INTEGER,      -- unix timestamp, refresh worker burayı kontrol eder
    filename TEXT,
    content_type TEXT,
    size INTEGER,
    width INTEGER,
    height INTEGER,
    -- foyer cache key = u64 snowflake (encode edilmiş attachment.id)
    -- local_path sadece debug için, foyer kendi path'ini yönetir
    cached_at INTEGER,           -- ilk download zamanı
    last_accessed_at INTEGER,    -- LRU tracking (adaptive tier sizing için)
    encrypted INTEGER DEFAULT 1  -- AES-GCM zorunlu
);

CREATE INDEX idx_attachments_message ON attachments(message_id);
CREATE INDEX idx_attachments_expiry ON attachments(url_expires_at)
    WHERE url_expires_at IS NOT NULL;  -- refresh worker 23h'den eskiyenleri bulur

CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT,
    discriminator TEXT,
    avatar TEXT,
    bot INTEGER DEFAULT 0,
    updated_at INTEGER NOT NULL
);
```

### 3.2 `crates/viscos-cache/src/db.rs`

```rust
use rusqlite::{Connection, OpenFlags};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use refinery::embed_migrations;
use std::path::Path;

embed_migrations!("migrations");

pub struct Db {
    pool: Pool<SqliteConnectionManager>,
}

impl Db {
    pub fn open<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let manager = SqliteConnectionManager::file(path)
            .with_flags(OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)
            .with_init(|c| {
                c.execute_batch("
                    PRAGMA journal_mode = WAL;
                    PRAGMA synchronous = NORMAL;
                    PRAGMA temp_store = MEMORY;
                    PRAGMA mmap_size = 268435456;  -- 256MB
                    PRAGMA foreign_keys = ON;
                ")
            });
        
        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(2))
            .build(manager)?;
        
        let mut conn = pool.get()?;
        embedded_migrations::migrations_runner().run_async(&mut futures_util::future::Either::Left(async {
            Ok::<_, refinery::Error>(())
        })).await.map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;
        
        Ok(Self { pool })
    }
    
    pub fn pool(&self) -> &Pool<SqliteConnectionManager> { &self.pool }
}
```

### 3.3 moka In-Memory Cache

```rust
// crates/viscos-cache/src/memory.rs
use moka::future::Cache;
use viscos_core::types::Message;
use std::time::Duration;

pub struct MessageCache {
    cache: Cache<String, Message>,  // message_id → Message
}

impl MessageCache {
    pub fn new(capacity: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(Duration::from_secs(3600))      // 1 saat
            .time_to_idle(Duration::from_secs(300))       // 5 dk idle
            .build();
        Self { cache }
    }
    
    pub async fn get(&self, id: &str) -> Option<Message> {
        self.cache.get(id).await
    }
    
    pub async fn insert(&self, id: String, msg: Message) {
        self.cache.insert(id, msg).await;
    }
}
```

### 3.4 Message Repository

```rust
// crates/viscos-cache/src/messages.rs
use rusqlite::{params, OptionalExtension};
use serde_json;
use viscos_core::types::Message;

pub struct MessageRepo<'a> {
    conn: &'a rusqlite::Connection,
}

impl<'a> MessageRepo<'a> {
    pub fn upsert(&self, msg: &Message) -> rusqlite::Result<()> {
        let raw = serde_json::to_string(msg).unwrap();
        self.conn.execute(
            "INSERT OR REPLACE INTO messages 
                (id, channel_id, author_id, content, timestamp, edited_timestamp, raw_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%s', 'now'))",
            params![
                msg.id, msg.channel_id, msg.author.id, msg.content,
                msg.timestamp, msg.edited_timestamp, raw,
            ],
        )?;
        Ok(())
    }
    
    pub fn get(&self, id: &str) -> rusqlite::Result<Option<Message>> {
        let raw: Option<String> = self.conn
            .query_row("SELECT raw_json FROM messages WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?;
        Ok(raw.and_then(|r| serde_json::from_str(&r).ok()))
    }
    
    pub fn list_by_channel(&self, channel_id: &str, limit: u32, before: Option<&str>) -> rusqlite::Result<Vec<Message>> {
        let mut sql = String::from(
            "SELECT raw_json FROM messages WHERE channel_id = ?1"
        );
        if before.is_some() {
            sql.push_str(" AND timestamp < ?2");
        }
        sql.push_str(" ORDER BY timestamp DESC LIMIT ?3");
        
        let mut stmt = self.conn.prepare(&sql)?;
        let rows: Result<Vec<String>, _> = if let Some(b) = before {
            stmt.query_map(params![channel_id, b, limit], |r| r.get(0))?.collect()
        } else {
            stmt.query_map(params![channel_id, limit], |r| r.get(0))?.collect()
        };
        
        Ok(rows?.into_iter().filter_map(|r| serde_json::from_str(&r).ok()).collect())
    }
}
```

---

## 4. `viscos-media` (foyer + Encryption)

### 4.1 Hybrid Cache (Content-Addressable)

**Discord CDN URL politikası:** User-uploaded attachment URL'leri **24 saat signed** (`ex` + `is` + `hm` HMAC query params). Discord bunları otomatik yenilese de (gateway + REST response), 24 saatten eski mesajlarda signed URL expire olur. **Avatar, banner, guild icon, emoji, role icon** ise **signed değil, expire olmaz**.

**Cache key stratejisi (ADR-0010 Haziran 2026 eki):** Disk cache key = **Discord `attachment_id` snowflake (u64)**. URL asla key olmaz → 24 saatte cache invalid olmaz. Signed URL sadece fetch anında alınır (moka metadata cache + arka plan refresh worker).

```rust
// crates/viscos-media/src/cache.rs
use foyer::{HybridCache, HybridCacheBuilder, HybridCachePolicy};
use std::path::Path;
use bytes::Bytes;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct MediaCache {
    /// Disk-first hybrid cache. Key = Discord attachment snowflake (u64).
    /// 24h signed URL limit'ine takılmaz — key stable identity.
    blobs: HybridCache<u64, EncryptedMediaBlob>,

    /// RAM-only signed URL cache. 1 saat TTL.
    /// Cache miss → ya disk'ten blob oku (signed URL lazım değil),
    /// ya da yeni signed URL al (background refresh veya lazy).
    url_meta: moka::future::Cache<u64, CdnUrlMeta>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct EncryptedMediaBlob {
    /// nonce (12 byte) || AES-GCM(plaintext, key) + 16 byte tag
    pub ciphertext: Vec<u8>,
    pub mime: String,
    pub size_plain: u32,
}

#[derive(Clone)]
pub struct CdnUrlMeta {
    pub signed_url: String,
    pub expires_at: SystemTime,
    pub mime: String,
    pub size: u32,
}

impl MediaCache {
    pub async fn open<P: AsRef<Path>>(dir: P, capacity: usize) -> anyhow::Result<Self> {
        let blobs: HybridCache<u64, EncryptedMediaBlob> = HybridCacheBuilder::new()
            .with_name("viscos-media")
            .with_storage(foyer::Storage::Filesystem(dir.as_ref().to_path_buf()))
            .with_policy(HybridCachePolicy::WTinyLFU)  // moka'dan daha iyi (Discord OLTP pattern)
            .with_memory_capacity(32 * 1024 * 1024)  // 32 MB RAM tier (adaptive Faz 1.5)
            .build()
            .await?;

        let url_meta = moka::future::Cache::builder()
            .max_capacity(50_000)  // ~50K metadata entry
            .time_to_live(Duration::from_secs(3600))  // 1 saat — signed URL 24h ama biz erken yenileriz
            .time_to_idle(Duration::from_secs(300))   // 5 dk idle → eviction
            .build();

        Ok(Self { blobs, url_meta })
    }

    /// Fetch anında çağrılır. Disk'te varsa URL al (refresh gerekebilir),
    /// yoksa network'ten çekip diske yaz.
    pub async fn get_or_fetch<F, Fut>(
        &self,
        attachment_id: u64,
        fetcher: F,
    ) -> anyhow::Result<Bytes>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<(Bytes, String)>>, // (blob, signed_url)
    {
        // 1. URL metadata cache kontrol
        if let Some(meta) = self.url_meta.get(&attachment_id).await {
            if meta.expires_at > SystemTime::now() + Duration::from_secs(3600) {
                // URL en az 1 saat geçerli, kullan
            } else {
                // Yakında expire olacak → lazy refresh tetikle
                self.refresh_url_async(attachment_id).await;
            }
        } else {
            self.refresh_url_async(attachment_id).await;
        }

        // 2. Disk'te var mı kontrol (content-addressable)
        if let Some(blob) = self.blobs.get(&attachment_id).await {
            return Ok(decrypt_blob(&blob)?);
        }

        // 3. Yoksa fetch et (moka'dan signed URL al, network'ten indir)
        let url = self.url_meta.get(&attachment_id).await
            .ok_or_else(|| anyhow::anyhow!("URL metadata missing after refresh"))?
            .signed_url;

        let (data, new_url) = fetcher(url).await?;
        let encrypted = encrypt_blob(&data)?;
        self.blobs.insert(attachment_id, encrypted).await;
        self.url_meta.insert(attachment_id, parse_url_meta(&new_url)?).await;
        Ok(data)
    }
}
```

**Signed URL olmayan kaynaklar (avatar, banner, icon, emoji):**
- Bunlar expire olmaz → cache key basit: SHA-256(url) → Bytes (RAM only).
- moka `Cache<String, Bytes>` yeterli, foyer'a koymaya gerek yok (küçük + sıcak).
- Bkz. `crates/viscos-media/src/static_assets.rs` (Faz 4'te eklenecek).

### 4.2 Viewport-Prioritized Download (Content-Addressable uyumlu)

```rust
// crates/viscos-media/src/downloader.rs
use viscos_ipc::IpcBridge;
use std::collections::HashSet;
use tokio::sync::mpsc;

pub struct ViewportDownloader {
    visible: HashSet<u64>,    // attachment_id (snowflake), URL değil
    pending: HashSet<u64>,
    tx: mpsc::UnboundedSender<DownloadRequest>,
}

pub struct DownloadRequest {
    pub attachment_id: u64,
    pub priority: Priority,
}

pub enum Priority {
    High,    // viewport'ta
    Medium,  // yakın (scroll out 1 viewport)
    Low,     // diğer
}

impl ViewportDownloader {
    pub fn new() -> Self { /* ... */ }

    pub fn update_viewport(&mut self, visible: Vec<u64>) {
        self.visible = visible.into_iter().collect();
    }

    pub async fn enqueue(&self, attachment_id: u64) {
        let priority = if self.visible.contains(&attachment_id) {
            Priority::High
        } else {
            Priority::Low
        };
        self.tx.send(DownloadRequest { attachment_id, priority }).unwrap();
    }
}
```

### 4.3 CDN Refresh Worker (24h Signed URL Yenileme)

**Neden gerekli:** Discord user-uploaded attachment URL'leri 24 saat signed. Content-addressable cache key stratejisi (4.1) sayesinde **disk'teki blob ölümsüz**, ama fetch anında signed URL lazım. Refresh worker, expire olmadan URL'leri batch'leyerek yeniler.

**Discord rate-limit:** `POST /channels/{channel_id}/attachments/refresh-urls` → max **50 URL per call**.

```rust
// crates/viscos-media/src/refresh.rs
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};
use tokio::time::{interval, Interval};
use viscos_api::DiscordClient;
use crate::cache::{MediaCache, CdnUrlMeta};

/// Background task: her 1 saatte tick atar, 23-24h arası URL'leri batch'ler.
pub struct CdnRefreshWorker {
    /// channel_id → [(attachment_id, expires_at)]
    pending: HashMap<u64, Vec<(u64, SystemTime)>>,
    last_tick: Instant,
    refresh_window: Duration,         // default 1 saat (23-24h aralığını tara)
    api_client: DiscordClient,
}

impl CdnRefreshWorker {
    pub fn new(api_client: DiscordClient) -> Self {
        Self {
            pending: HashMap::new(),
            last_tick: Instant::now(),
            refresh_window: Duration::from_secs(3600),
            api_client,
        }
    }

    /// moka eviction listener'dan çağrılır: yeni metadata eklenince pending'e ekle.
    pub fn on_url_cached(&mut self, channel_id: u64, attachment_id: u64, expires_at: SystemTime) {
        self.pending
            .entry(channel_id)
            .or_default()
            .push((attachment_id, expires_at));
    }

    /// Her saat başı çalışır. 23-24h aralığındaki URL'leri refresh eder.
    pub async fn tick(&mut self, media: &MediaCache) -> anyhow::Result<()> {
        if self.last_tick.elapsed() < self.refresh_window {
            return Ok(());
        }
        self.last_tick = Instant::now();

        let now = SystemTime::now();
        let cutoff = now + Duration::from_secs(23 * 3600);  // 23h sonra

        // 1. Pending listesinden 23h < expires_at < 24h olanları seç
        let to_refresh: HashMap<u64, Vec<u64>> = self.pending
            .iter()
            .filter_map(|(channel_id, urls)| {
                let filtered: Vec<u64> = urls
                    .iter()
                    .filter(|(_, exp)| *exp > now && *exp <= cutoff)
                    .map(|(id, _)| *id)
                    .collect();
                if filtered.is_empty() { None } else { Some((*channel_id, filtered)) }
            })
            .collect();

        // 2. Her channel için batch refresh (50 per call)
        for (channel_id, attachment_ids) in to_refresh {
            for chunk in attachment_ids.chunks(50) {
                let refreshed = self.api_client
                    .refresh_attachment_urls(channel_id, chunk)
                    .await?;

                // 3. moka metadata'yı güncelle (blob cache'i değişmez)
                for (id, url) in refreshed {
                    let meta = parse_url_meta(&url)?;
                    media.url_meta.insert(id, meta).await;
                }

                // Rate-limit koruması: 100ms bekleme
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // 4. Pending listesini temizle (zaten refresh edilenler çıkar)
        self.pending.retain(|_, urls| {
            urls.retain(|(_, exp)| *exp > cutoff);
            !urls.is_empty()
        });

        Ok(())
    }

    /// Spawn: main loop, her refresh_window'da tick çağırır.
    pub async fn run(mut self, mut media: MediaCache) {
        let mut ticker = interval(self.refresh_window);
        loop {
            ticker.tick().await;
            if let Err(e) = self.tick(&media).await {
                tracing::warn!("CDN refresh worker tick failed: {e}");
            }
        }
    }
}
```

**Test stratejisi (CDN refresh):**
- Unit: 23-24h aralığı filtresi doğru (now mockable)
- Unit: 50'lik chunk'lar doğru (49 → 1 call, 51 → 2 call)
- Integration: rate-limit koruması (Discord fake API ile)
- Integration: refresh sonrası moka metadata güncelleniyor

### 4.4 Encryption at Rest (AES-GCM)

```rust
// crates/viscos-media/src/encrypt.rs
use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit, Aead};
use argon2::{Argon2, PasswordHasher, SaltString};
use viscos_auth::AuthStorage;
use rand::RngCore;

pub struct MediaCrypto {
    cipher: Aes256Gcm,
}

impl MediaCrypto {
    pub fn new_from_keyring() -> anyhow::Result<Self> {
        let auth = AuthStorage::new()?;
        // Anahtar keyring'de saklanır (Faz 2'de)
        // Veya kullanıcı parolasından türetilir (PBKDF2/Argon2)
        let key_bytes = auth.load_or_create_media_key()?;
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        Ok(Self { cipher })
    }
    
    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self.cipher.encrypt(nonce, plaintext)?;
        // nonce || ciphertext
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }
    
    pub fn decrypt(&self, blob: &[u8]) -> anyhow::Result<Vec<u8>> {
        if blob.len() < 12 { anyhow::bail!("Blob too short"); }
        let (nonce_bytes, ciphertext) = blob.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        Ok(self.cipher.decrypt(nonce, ciphertext)?)
    }
}
```

---

## 4.4 Büyük Blob Transfer (WebView2 SharedBuffer)

**Neden bu fazda?** Faz 2-3'teki Gateway entegrasyonu gerçek avatar/sticker/emoji trafiğini başlatacak. Faz 4'te cache katmanı eklendiğinde, büyük binary response'lar (80KB-2MB) pull-based JSON+base64 round-trip ile yavaş. WebView2 SharedBuffer ile zero-copy transfer, `eval_script` upstream bug'ından (tauri#13758) yapısal kaçış + ~9× latency düşüşü sağlar.

**Trade-off özeti:**
- **Artı:** Zero-copy, ~9× hız, ~40× transient memory azalma, tauri#13758 tetiklenmez
- **Eksi:** `webview2-com` direct dependency (+1-2MB binary, COM marshalling), Edge ≥ 114 pin gerekir (WebView2Feedback#3360 crash fix), JS'te `releaseBuffer()` cleanup discipline, CEF backend'inde ayrı implementasyon (Faz 8.5)

### 4.4.1 Workspace Dependency

```toml
[workspace.dependencies]
# Mevcut wry yanına raw WebView2 COM bindings (SharedBuffer için)
# Alternatif: wry upstream PR takip edilir (wry#767), kabul edilirse doğrudan wry üzerinden
webview2-com = "0.36"
```

> **Not:** Eğer [`tauri-apps/wry`](https://github.com/tauri-apps/wry/issues/767) 2026 ortasına kadar SharedBuffer'ı birinci sınıf desteklerse, `webview2-com` dependency kaldırılır ve `wry` API'si üzerinden gidilir. Bu durumda Faz 1'deki trait stub'ı zaten yeri hazır.

### 4.4.2 `WryWebView2Backend::post_shared_buffer` Implementasyonu

```rust
// crates/viscos-webview/src/webview2.rs (Faz 4'te eklenecek)
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Environment, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
};

impl WebView2Backend {
    /// Faz 4'te implement edilecek.
    /// Akış:
    ///   1. CoreWebView2Environment::CreateSharedBuffer(size) → SharedBuffer
    ///   2. SharedBuffer::OpenStream() ile native taraftan bytes yaz
    ///   3. CoreWebView2::PostSharedBufferToScript(buffer, ReadOnly, metadata_json)
    ///   4. JS tarafı 'message' event'inde ArrayBuffer olarak alır
    ///   5. JS işi bitince buffer.releaseBuffer() çağırır (zorunlu, yoksa leak)
    pub fn post_shared_buffer(
        env: &ICoreWebView2Environment,
        webview: &ICoreWebView2,
        bytes: &[u8],
        metadata: &str,  // JSON string, max ~2KB (kind, id, hash, vb.)
    ) -> anyhow::Result<()> {
        // 1. SharedBuffer oluştur (size bytes)
        let shared_buffer = unsafe { env.CreateSharedBuffer(bytes.len() as u64)? };

        // 2. Native stream üzerinden yaz
        unsafe {
            let stream = shared_buffer.OpenStream()?;
            stream.Write(bytes)?;
            // stream drop → Close otomatik
        }

        // 3. WebView'e postla (sahiplik transferi, zero-copy)
        unsafe {
            webview.PostSharedBufferToScript(
                &shared_buffer,
                COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
                metadata,
            )?;
        }

        // 4. Rust tarafı buffer'ı düşürür (JS artık sahip)
        drop(shared_buffer);

        Ok(())
    }
}
```

### 4.4.3 IpcCommand Genişletmesi (Faz 4'te)

```rust
// crates/viscos-ipc/src/lib.rs (Faz 4'te eklenecek variant)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IpcCommand {
    // ... mevcut variant'lar ...

    /// Binary response SharedBuffer ile döner. >10KB payload'lar için default path.
    GetAvatar { user_id: String, hash: String },
    GetSticker { sticker_id: String },
    GetEmoji { emoji_id: String, guild_id: Option<String> },
    GetAttachment { attachment_id: String },

    /// JSON path (mevcut davranış, küçük response'lar için)
    GetGuildIcon { guild_id: String, format: ImageFormat },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
}
```

### 4.4.4 Frontend `getBinary<T>()` API

```typescript
// frontend/src/bridge.ts (Faz 4'te eklenecek)

declare global {
  interface Window {
    viscos: {
      invoke: <T = any>(cmd: IpcCommand) => Promise<T>;
      onEvent: (handler: (event: any) => void) => () => void;
      // YENİ: büyük binary response'lar için SharedBuffer wrapper
      getBinary: <T extends ArrayBuffer = ArrayBuffer>(
        cmd: IpcCommand,
        kind: string,
        timeoutMs?: number
      ) => Promise<T>;
    };
    chrome: { webview: { addEventListener: (...) => void; removeEventListener: (...) => void } };
  }
}

const pendingBinary = new Map<string, {
  resolve: (buf: ArrayBuffer) => void;
  reject: (e: Error) => void;
  timeout: number;
}>();

window.viscos.getBinary = <T extends ArrayBuffer = ArrayBuffer>(
  cmd: IpcCommand,
  kind: string,
  timeoutMs = 5000
): Promise<T> => {
  return new Promise<T>((resolve, reject) => {
    const token = crypto.randomUUID();
    const timeout = window.setTimeout(() => {
      pendingBinary.delete(token);
      reject(new Error(`getBinary timeout (${timeoutMs}ms): ${kind}`));
    }, timeoutMs);

    pendingBinary.set(token, {
      resolve: (buf) => resolve(buf as T),
      reject,
      timeout,
    });

    // Tek seferlik message listener (window.viscos üzerinden değil, chrome.webview)
    const handler = (event: MessageEvent) => {
      const data = event.data;
      if (data?.kind === kind && data?.token === token) {
        window.chrome.webview.removeEventListener('message', handler);
        const p = pendingBinary.get(token);
        if (p) {
          pendingBinary.delete(token);
          window.clearTimeout(p.timeout);
          // ZORUNLU: SharedBuffer'ı serbest bırak
          // (data.buffer bir SharedBuffer-backed ArrayBuffer, releaseBuffer ile)
          try {
            (data.buffer as any).releaseBuffer?.();
          } catch (e) {
            console.warn('releaseBuffer başarısız', e);
          }
          p.resolve(data.buffer);
        }
      }
    };
    window.chrome.webview.addEventListener('message', handler);

    // Rust tarafına isteği gönder (token + cmd metadata)
    window.ipc.postMessage(JSON.stringify({ token, cmd }));
  });
};
```

**Kullanım örneği (avatar):**

```typescript
// Avatar'ı SharedBuffer olarak al
const buf = await window.viscos.getBinary(
  { type: 'GetAvatar', data: { userId: '123', hash: 'abc' } },
  'avatar'
);
const blob = new Blob([buf], { type: 'image/png' });
const url = URL.createObjectURL(blob);
// <img src={url} /> — releaseBuffer wrapper içinde zaten çağrıldı
```

### 4.4.5 Benchmark Hedefi

**Setup:** 100 ardışık 80KB PNG avatar fetch (Discord varsayılan avatar boyutu), 16 worker paralel.

| Metrik | JSON + base64 (mevcut) | SharedBuffer (hedef) | Kabul |
|--------|------------------------|----------------------|-------|
| Toplam süre | ~850ms | **< 170ms** (>5× düşüş) | ✅ |
| Memory peak (transient) | ~3.2MB | **< 100KB** (>30× düşüş) | ✅ |
| `eval_script` call sayısı | 100 | 0 | ✅ |
| IPC buffer şişmesi | +10MB | +0 | ✅ (50MB watchdog threshold tetiklenmemeli) |

Benchmark kodu: `crates/viscos-bench/benches/shared_buffer.rs` (criterion ile, lokal çalıştırılır, CI'da değil — gerçek WebView2 instance gerekir).

### 4.4.6 Test Stratejisi

| Test | Tip | Kabul |
|------|-----|-------|
| SharedBuffer roundtrip | Integration | Rust yazar → JS okur, byte-perfect |
| `releaseBuffer` çağrılmazsa | Integration | 1000 ardışık fetch sonrası renderer process memory < baseline + 50MB |
| Metadata > 2KB | Unit | Hata döner, IPC fail değil (graceful) |
| Concurrent fetch (16 paralel) | Integration | 16 SharedBuffer eş zamanlı, sıralama karışmaz |
| 32.000 SharedBuffer sonrası | Edge ≥ 114'te crash yok | Pin check: `CoreWebView2Environment.CompareBrowserVersions` Faz 1'de |

### 4.4.7 Edge Versiyon Pin

```rust
// crates/viscos-webview/src/webview2.rs (Faz 1'de eklenecek kontrol)
fn check_edge_version(env: &ICoreWebView2Environment) -> anyhow::Result<()> {
    let min_version = "114.0.1802.0";
    // CompareBrowserVersions return değeri: -1 (eski), 0 (eşit), 1 (yeni)
    let result = unsafe { env.CompareBrowserVersions(min_version, "EDGE_BROWSER_VERSION")? };
    if result < 0 {
        anyhow::bail!(
            "WebView2 runtime >= {} gerekli (mevcut: eski). \
             Lütfen Edge'i güncelleyin. \
             WebView2Feedback#3360 SharedBuffer crash fix bu sürümde.",
            min_version
        );
    }
    Ok(())
}
```

**Failure mode:** Eski Edge'de SharedBuffer çağrısı yapılmaz, otomatik olarak JSON fallback path'ine düşer (`post_shared_buffer` yerine normal `eval_script` ile base64 döner). **Crash değil, sadece performans düşüşü.**

### 4.4.8 CEF Backend Karşılığı (Faz 8.5 Backlog)

Faz 8.5'te CEF opt-in geldiğinde, aynı `WebViewBackend::post_shared_buffer` trait method'u CEF tarafında farklı implement edilecek:

```rust
// crates/viscos-webview/src/cef.rs (Faz 8.5 backlog)
impl CefBackend {
    pub fn post_shared_buffer(
        &self,
        bytes: &[u8],
        metadata: &str,
    ) -> anyhow::Result<()> {
        // CEF'in SharedMemoryRegion + message_router threshold-based davranışı
        // CefMessageRouter otomatik threshold (default 1MB) üstünde shared memory kullanır
        // Biz sadece bytes + metadata'yı router'a veriyoruz
        self.message_router.send_query(bytes, metadata)
    }
}
```

**Bu sayede `IpcBridge` interface'i değişmez, sadece transport değişir.** Vencord/Equicord plugin contract'ı korunur.

### 4.5 Adaptive Tier Sizing (Faz 1.5 Telemetry-Driven)

**Neden gerekli:** Cache tier boyutları kullanıcı davranışına göre dramatik değişir. Bir kullanıcının 5 kanalı, 50 sunucusu, 2K attachment'ı varken, diğerinin 200 kanalı, 100 sunucusu, 50K attachment'ı var. **Statik default'lar herkese uymuyor.**

**v1 stratejisi:** Statik default'lar korunur (RAM = 64 MB, foyer memory = 32 MB, foyer disk = 10 GB). v1.5'te adaptive devreye girer.

**v1.5 stratejisi:** Faz 1.5 telemetry backend'i (SQLite, opt-out) tier hit ratio'larını toplar. **Her saat aggregate** → adaptive algorithm tier boyutlarını tune eder.

```rust
// crates/viscos-cache/src/adaptive.rs (Faz 4'te stub, Faz 1.5'te aktif)
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierStats {
    pub window_start: u64,        // unix timestamp
    pub window_secs: u32,         // 3600 (1 saat)

    // moka (sıcak metadata) — RAM tier
    pub moka_capacity_bytes: u64,
    pub moka_hits: u64,
    pub moka_misses: u64,
    pub moka_evictions: u64,

    // foyer memory tier — sıcak blob parçaları
    pub foyer_memory_capacity: u64,
    pub foyer_memory_hits: u64,
    pub foyer_memory_misses: u64,

    // foyer disk tier — blob disk cache
    pub foyer_disk_capacity: u64,
    pub foyer_disk_hits: u64,
    pub foyer_disk_misses: u64,
}

impl TierStats {
    pub fn moka_hit_ratio(&self) -> f64 {
        self.moka_hits as f64 / (self.moka_hits + self.moka_misses).max(1) as f64
    }

    pub fn foyer_disk_hit_ratio(&self) -> f64 {
        self.foyer_disk_hits as f64 / (self.foyer_disk_hits + self.foyer_disk_misses).max(1) as f64
    }
}

/// Adaptive tuning algorithm. v1'de no-op (returns default).
/// v1.5'te hit ratio thresholds'a göre tier boyutlarını ayarlar.
pub struct TierTuner {
    config: AdaptiveConfig,
    current: TierSizes,
}

#[derive(Debug, Clone)]
pub struct TierSizes {
    pub moka_bytes: u64,
    pub foyer_memory_bytes: u64,
    pub foyer_disk_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    pub enabled: bool,                          // opt-out flag, default false (v1.5)
    pub moka_hit_ratio_low: f64,                // default 0.70 → büyüt
    pub moka_hit_ratio_high: f64,               // default 0.95 → küçült (RAM geri kazan)
    pub foyer_disk_hit_ratio_low: f64,          // default 0.40 → disk büyüt (kullanıcı onayı)
    pub user_disk_cap_bytes: u64,               // default 25 GB (10 GB default + adaptive)
}

impl TierTuner {
    pub fn tune(&mut self, stats: &TierStats) -> Option<TierSizes> {
        if !self.config.enabled { return None; }

        let mut new_sizes = self.current.clone();
        let mut changed = false;

        // moka tuning
        if stats.moka_hit_ratio() < self.config.moka_hit_ratio_low {
            new_sizes.moka_bytes = (new_sizes.moka_bytes * 2).min(256 * 1024 * 1024); // max 256 MB
            changed = true;
            tracing::info!("Adaptive: moka cache doubled to {} MB (hit ratio {:.2})",
                new_sizes.moka_bytes / 1024 / 1024, stats.moka_hit_ratio());
        } else if stats.moka_hit_ratio() > self.config.moka_hit_ratio_high
            && self.current.moka_bytes > 32 * 1024 * 1024
        {
            new_sizes.moka_bytes = (new_sizes.moka_bytes / 2).max(32 * 1024 * 1024); // min 32 MB
            changed = true;
            tracing::info!("Adaptive: moka cache halved to {} MB (hit ratio {:.2})",
                new_sizes.moka_bytes / 1024 / 1024, stats.moka_hit_ratio());
        }

        // foyer disk tuning (sadece yukarı, kullanıcı onayı gerekli)
        if stats.foyer_disk_hit_ratio() < self.config.foyer_disk_hit_ratio_low
            && new_sizes.foyer_disk_bytes < self.config.user_disk_cap_bytes
        {
            // ConfigurableGrowth: sadece cap içindeyse büyüt, cap dışı kullanıcı onayı iste
            new_sizes.foyer_disk_bytes = (new_sizes.foyer_disk_bytes * 2)
                .min(self.config.user_disk_cap_bytes);
            changed = true;
        }

        if changed {
            self.current = new_sizes.clone();
            Some(new_sizes)
        } else {
            None
        }
    }
}
```

**Tier tuning policy tablosu:**

| Tier | v1 Default | v1.5 Trigger | Yeni Değer | Cap | Onay Gerekir mi? |
|---|---|---|---|---|---|
| moka (RAM metadata) | 64 MB | Hit ratio <%70 | 2× (128 MB) | 256 MB | Hayır (otomatik) |
| moka (RAM metadata) | 64 MB | Hit ratio >%95 (overprovision) | ÷2 (32 MB) | 32 MB min | Hayır |
| foyer memory tier | 32 MB | Disk hit ratio >%60 | 2× (64 MB) | 128 MB | Hayır |
| foyer disk tier | 10 GB | Disk hit ratio <%40 | 2× (20 GB) | 25 GB (config) | **Evet** (cap dışı büyüme için tray notification) |
| foyer disk tier | 10 GB | User "Storage low" feedback | 5 GB (küçült, kullanıcı isteği) | — | **Evet** (UI) |

**Opt-out:** `config.cache.tier.auto_tune = true` (default v1.5'te true, v1'de false). Telemetry opt-out'tan bağımsız — adaptive sadece tier size değiştirmez, telemetry'i dinler.

**Restart'a gerek yok:** moka `Cache::builder().max_capacity(...)` builder'da set edilir → runtime'da capacity değiştirmek için rebuild gerekir. v1'de **tuning sadece restart sonrası uygulanır** (config'e yaz, sonraki açılışta oku). v1.5 ilerleyen iterasyonda runtime reconfigure.

**Test stratejisi:**
- Unit: hit ratio thresholds doğru tetikleniyor (mock stats ile)
- Unit: cap'e ulaşınca büyüme duruyor
- Unit: opt-out flag ile tuner no-op
- Integration: telemetry → tuner → config dosyası → sonraki restart'ta uygulanma (24 saat soak)

### 4.6 Phase 4 → Faz 1.5 Telemetry Entegrasyonu

Faz 4'te `TierStats` toplama infrastructure kurulur (SQLite tablo + hourly aggregation job). **Adaptive algorithm Faz 1.5'te aktif olur.** Faz 4'te stub olarak kalır (no-op).

```rust
// crates/viscos-cache/src/telemetry.rs (Faz 4 stub, Faz 1.5 aktif)
use rusqlite::Connection;
use crate::adaptive::TierStats;

pub struct CacheTelemetry {
    db: Connection,
}

impl CacheTelemetry {
    /// Faz 4'te stub — sadece veri topla, kullanma.
    /// Faz 1.5'te TierTuner.tune() çağırır.
    pub async fn on_window_close(&self, stats: TierStats) -> anyhow::Result<()> {
        // 1. SQLite'a yaz (Faz 1.5 telemetry backend schema'sına uygun)
        self.db.execute(
            "INSERT INTO cache_tier_stats (window_start, moka_hits, moka_misses, ...)
             VALUES (?1, ?2, ?3, ...)",
            rusqlite::params![
                stats.window_start, stats.moka_hits, stats.moka_misses,
            ],
        )?;

        // 2. 30 gün rolling retention (Faz 1.5)
        // 3. 100MB cap (Faz 1.5)

        Ok(())
    }
}
```

---

## 5. Jemalloc Benchmark

### 5.1 `crates/viscos-bench/benches/heap.rs`

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn bench_heap_under_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap");
    
    // Sistem allocator
    group.bench_function("system", |b| {
        b.iter(|| {
            // 1000 mesaj oluştur, 5 saniye tut, sonra sil
            let mut v: Vec<Box<[u8; 1024]>> = Vec::new();
            for _ in 0..1000 {
                v.push(Box::new([0u8; 1024]));
            }
            std::thread::sleep(Duration::from_millis(100));
            drop(v);
        });
    });
    
    // jemalloc
    #[cfg(feature = "jemalloc")]
    group.bench_function("jemalloc", |b| {
        b.iter(|| {
            let mut v: Vec<Box<[u8; 1024]>> = Vec::new();
            for _ in 0..1000 {
                v.push(Box::new([0u8; 1024]));
            }
            std::thread::sleep(Duration::from_millis(100));
            drop(v);
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_heap_under_load);
criterion_main!(benches);
```

### 5.2 Uzun Süreli Soak Test (Allocator Karşılaştırma)

```rust
// tools/heap-soak/src/main.rs
// 24 saatlik heap fragmentation karşılaştırması
// 1 dk'da bir RSS logla, sonra min/max/mean hesapla

use std::time::Duration;
use tokio::time::interval;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut interval = interval(Duration::from_secs(60));
    let start = std::time::Instant::now();
    
    // Background: sürekli mesaj oluştur/sil (yük simülasyonu)
    tokio::spawn(async {
        loop {
            simulate_load().await;
        }
    });
    
    for i in 0..(24 * 60) {
        interval.tick().await;
        let rss = get_rss();
        println!("t={}min rss={}MB", i, rss / 1024 / 1024);
    }
    
    Ok(())
}
```

**Karar kriteri:** jemalloc ile RSS artışı ≥%15 az → adopt. Aksi → default allocator.

---

## 6. Test Stratejisi (Faz 4.0)

| Test | Tip | Kabul |
|------|-----|-------|
| SQLite migration | Integration | Schema doğru, version tablosu |
| moka TTL | Integration | 1 saat sonra expire |
| moka idle | Integration | 5 dk idle sonra expire |
| Message insert/get | Integration | Roundtrip |
| Channel list query | Integration | Index kullanılıyor (EXPLAIN QUERY PLAN) |
| foyer disk persist | Integration | Process restart'ta cache korunuyor |
| Encryption roundtrip | Unit | Decrypt(Encrypt(x)) == x |
| Jemalloc vs system | Benchmark (lokal) | 24 saat soak, %15 fark |
| **SharedBuffer roundtrip** | Integration | Rust yazar → JS okur, byte-perfect |
| **SharedBuffer leak kontrolü** | Integration | 1000 fetch sonrası renderer memory < baseline + 50MB |
| **JSON vs SharedBuffer benchmark** | Benchmark (lokal) | 100 ardışık 80KB fetch, >5× latency düşüşü |
| **Edge ≥ 114 pin** | Unit | Eski Edge'de graceful JSON fallback |
| **Content-addressable cache key (Haziran 2026 eki)** | Integration | attachment_id ile insert/get, URL değişse bile cache korunuyor |
| **CDN refresh worker 23h filtresi** | Unit | Mock now() ile 23h < exp < 24h aralığı doğru |
| **CDN refresh 50-per-call batching** | Unit | 49 → 1 call, 51 → 2 call (rate-limit) |
| **CDN refresh rate-limit koruması** | Integration | Sleep 100ms per batch (Discord fake API ile) |
| **Signed URL olmayan kaynaklar (avatar/icon)** | Integration | URL key = SHA-256, expire olmuyor, sadece RAM |
| **Adaptive tier stub telemetry write** | Integration | Faz 4'te no-op, Faz 1.5 için schema doğru |

---

## 7. Kabul Kriterleri (Definition of Done)

- [ ] SQLite WAL mode aktif, schema migration çalışıyor
- [ ] moka cache 10K mesaj tutabiliyor, 1 saat TTL
- [ ] foyer disk cache restart'ta korunuyor
- [ ] Message insert/get/list çalışıyor
- [ ] Attachment download viewport-priority çalışıyor
- [ ] Encryption at rest: dosya diskte şifreli
- [ ] Jemalloc benchmark sonucu kaydedildi
- [ ] **Jemalloc karar:** Adopt / Erteleme / Default
- [ ] **`WryWebView2Backend::post_shared_buffer` implemente edildi** (webview2-com dependency)
- [ ] **`frontend/bridge.ts` `getBinary<T>()` API eklendi**, `try/finally releaseBuffer()` pattern
- [ ] **JSON vs SharedBuffer benchmark:** 100 ardışık 80KB avatar, **>5× latency düşüşü kanıtlandı**
- [ ] **1000 SharedBuffer fetch sonrası renderer memory < baseline + 50MB** (leak kontrol)
- [ ] **Edge ≥ 114 versiyon pin check** Faz 1'den, eski Edge'de JSON fallback çalışıyor
- [ ] `cargo clippy -- -D warnings` temiz
- [ ] `cargo test` tüm geçer
- [ ] 1 saatlik lokal soak: memory growth < %10
- [ ] **(ADR-0010)** `rusqlite 0.32 → 0.38` minor bump migrate edildi, integration testler yeşil
- [ ] **(ADR-0010)** `foyer 0.10 → 0.22` minor bump migrate edildi, `Code` trait encryption wrapper ile entegre
- [ ] **(ADR-0010)** `chacha20poly1305` dependency kaldırıldı, `cargo tree` temiz, `cargo deny check licenses` yeşil
- [ ] **(ADR-0010 Stretch)** cachebench benchmark: moka vs stretto Discord-trace workload'unda **>15 pp hit ratio farkı** stretto lehine ise v2 backlog'a al (yoksa karar: moka korunur, stretto reddedilir)
- [ ] **(ADR-0010)** Soak test: SQLite WAL + moka + foyer birlikte 1 saatlik lokal soak memory growth <%10
- [ ] **(ADR-0010 Haziran 2026 eki — Content-Addressable)** `attachment_id` (u64 snowflake) foyer cache key, URL asla key değil. 24h signed URL limit'i cache'i etkilemiyor.
- [ ] **(ADR-0010 Haziran 2026 eki — CDN Refresh Worker)** `CdnRefreshWorker` background task her saat 23h < expires_at < 24h aralığındaki URL'leri `POST /channels/{id}/attachments/refresh-urls` ile batch'ler (50 per call), rate-limit korumalı (100ms sleep).
- [ ] **(ADR-0010 Haziran 2026 eki — Signed URL Olmayan Kaynaklar)** Avatar, banner, guild icon, emoji, role icon → SHA-256(url) → moka RAM cache, foyer'a gerek yok.
- [ ] **(ADR-0010 Haziran 2026 eki — Adaptive Tier Stub)** `TierStats` toplama infrastructure kuruldu (SQLite), `TierTuner.tune()` Faz 4'te no-op, Faz 1.5'te aktif.
- [ ] **(ADR-0010 Haziran 2026 eki)** `attachments` schema `url_expires_at` + `idx_attachments_expiry` (refresh worker 23h filtresi için) entegre.
- [ ] **(ADR-0010 Haziran 2026 eki)** viewport-prioritized download `attachment_id` (u64) kullanıyor, URL değil.

---

## 8. Karar Noktası (Faz 4.0 Sonu)

> 🔵 **İNSAN:** Jemalloc geçiş kararı (benchmark sonucuna göre):
> - ≥%15 azalma → jemalloc'a geç (`tikv-jemallocator` adopt)
> - <%15 → system allocator kalır, v2'de tekrar dene
> - Trade-off: build complexity (jemalloc Windows MSVC sorunları) vs perf

> 🔵 **İNSAN:** Encryption at rest anahtarı:
> - Seçenek A: Keyring'de (DPAPI, kullanıcı oturumuyla)
> - Seçenek B: Kullanıcı parolasından PBKDF2/Argon2 türetme
> - Seçenek C: Hybrid (A default, B opsiyonel)
> - Trade-off: UX (parola her açılışta mı) vs güvenlik

> 🔵 **İNSAN:** Cache capacity ne olsun?
> - moka RAM: 50K entry (~100MB)
> - foyer disk: 5GB
> - Trade-off: RAM tasarrufu vs cache hit rate

> 🔵 **İNSAN:** Content-Addressable cache key stratejisi (Haziran 2026 eki):
> - Seçenek A: `attachment_id` (snowflake u64) — stable identity, 24h signed URL limit'ten etkilenmez (önerilen, mevcut karar)
> - Seçenek B: Signed URL itself — basit ama 24 saatte cache invalid olur
> - Seçenek C: Content hash (SHA-256 of bytes) — duplicate-friendly ama edit history kaybolur
> - Trade-off: complexity (refresh worker + iki katmanlı cache) vs cache ömrü sınırsız

> 🔵 **İNSAN:** Adaptive tier sizing opt-in mi olsun?
> - Seçenek A: Default opt-out (kullanıcı tray notification ile onaylar, recommended)
> - Seçenek B: Default opt-in (transparan, telemetry ile)
> - Seçenek C: Sadece disk için opt-in (en güvenli)
> - Trade-off: UX (transparan auto-tune) vs kullanıcı kontrolü

> 🔵 **İNSAN:** CDN refresh worker batch size:
> - Discord limit: 50 URL per call
> - Trade-off: büyük batch → az API call ama uzun transaction / küçük batch → çok call, rate limit risk

---

## 9. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| SQLite corruption | Veri kaybı | WAL mode + regular `PRAGMA integrity_check` |
| Foyer disk full | Yeni medya indirilemez | LRU eviction, user notification |
| Encryption key kayıp | Cache okunamaz | Recovery flow: re-login ile yeni key |
| Jemalloc Windows MSVC link | Build fail | `target-cpu=x86_64-v3` + manual config |
| Cache growth sınırsız | Disk dolma | Capacity cap + cleanup task |
| **CDN refresh worker rate-limit (Haziran 2026 eki)** | Discord API ban | 50 URL/call + 100ms sleep per batch + exponential backoff on 429 |
| **CDN refresh worker gateway disconnect** | Refresh durur, 24h sonra cache invalid | Worker state persist (SQLite), next start'ta devam eder |
| **Content-addressable cache invalidation (Haziran 2026 eki)** | Kullanıcı aynı attachment'ı 2 farklı mesajda paylaşırsa duplicate cache | Content hash (SHA-256) tabanlı dedup Faz 5 sonu stretch goal |
| **Adaptive tier thrashing (Haziran 2026 eki)** | Cache boyutu her saat değişir, restart spam | 24h'de en fazla 1 tier değişikliği (hysteresis) |
| **Foyer Windows NTFS overhead** | Disk cache cold latency yüksek | Psync + thread pool, adaptive memory tier büyütme (v1.5) |
| **Signed URL olmayan kaynaklar stale (URL değişirse)** | Avatar/icon 404 | moka TTL 1 saat + LRU eviction; Discord avatar hash değişirse yeni fetch |

---

## 10. Çıkış → Faz 5.0

Bu faz tamamlandığında:
- Mesaj geçmişi persist
- Medya cache'leniyor
- Encryption aktif
- Jemalloc kararı verilmiş

Faz 5.0 → Native UI (iced side panel) + Vencord POC. Cache'lenen veriyi iced widget'larında göster.
