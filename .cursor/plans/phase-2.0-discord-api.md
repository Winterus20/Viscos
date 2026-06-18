---
name: Phase 2.0 — Discord API + Auth
overview: Discord REST API client, rate limiting, X-Super-Properties header, token authentication (email/şifre, QR, token yapıştırma), `keyring-core` + `windows-native-keyring-store` (DPAPI) entegrasyonu, MFA (TOTP + backup codes) desteği, captcha redirect stratejisi, `secrecy` + `zeroize` bellek hijyeni. Detay: ADR-0011.
isProject: false
todos:
  - id: api-crate
    content: viscos-api crate oluştur
    status: pending
  - id: auth-crate
    content: viscos-auth crate (token, keyring-core, MFA TOTP+backup, secrecy, zeroize)
    status: pending
  - id: rest-client
    content: REST client (reqwest, connection pooling, brotli/gzip)
    status: pending
  - id: rate-limiter
    content: Per-route rate limit bucket, 429 Retry-After handling
    status: pending
  - id: super-properties
    content: X-Super-Properties header üretimi (web client fingerprint, build_number GitHub Action sync, WebGL hash CEF/WebView2'den)
    status: pending
  - id: keyring-integration
    content: keyring-core 0.7 + windows-native-keyring-store 1.1 (DPAPI), default-features=false (regex yok)
    status: pending
  - id: login-flow
    content: Login UI: email/şifre, QR kod, token yapıştırma, captcha redirect
    status: pending
  - id: mfa-support
    content: MFA (TOTP) + backup codes (Argon2 PHC, keyring entry'sinde)
    status: pending
  - id: mfa-backup-codes
    content: MFA backup codes storage + UI (göster/yenile/indir .txt, <10 kaldığında uyarı)
    status: pending
  - id: captcha-handling
    content: Captcha redirect akışı (Discord tarayıcıda giriş, token yapıştırma)
    status: pending
  - id: secrecy-zeroize
    content: Secret<String> + ZeroizeOnDrop tüm token path'lerinde
    status: pending
  - id: multi-account-foundation
    content: keyring user=user_id (Discord snowflake) key'leme, v2 için 0 refactor
    status: pending
  - id: token-validation
    content: Token validation (geçerli mi, süresi dolmuş mu, 401 → keyring delete + re-login UI)
    status: pending
  - id: davey-dep
    content: davey 0.1 optional dependency (DAVE E2EE compile-time API surface sabitleme) — ADR-0012 §4
    status: pending
  - id: shadow-mode
    content: İlk 24 saat shadow mode (sadece REST, yazma blocked, opt-out) — ADR-0012 §3.B
    status: pending
  - id: fingerprint-parity
    content: X-Super-Properties WebGL hash backend parity (CEF/WebView2 ile senkron) — ADR-0012 §3.A
    status: pending
---

# Phase 2.0 — Discord API + Auth

> **Süre:** 2–3 hafta
> **Hedef:** Discord API'sine bağlanma, kimlik doğrulama, rate limit'lere uyum.
> **Önceki faz:** [`phase-1.5-telemetry-and-restart-optimization.md`](./phase-1.5-telemetry-and-restart-optimization.md) (eski adı `phase-1.5-mouse-throttling.md`, Haziran 2026'da yeniden adlandırıldı)
> **Sonraki faz:** [`phase-3.0-gateway.md`](./phase-3.0-gateway.md)

---

## 1. Mimari Genel Bakış

```
┌──────────────────────────────────────────────────────────┐
│ viscos-shell (UI)                                        │
│   ├─ Login ekranı (iced, native)                        │
│   └─ Auth callback handler                              │
└────────────────────┬─────────────────────────────────────┘
                     │
       ┌─────────────┴─────────────┐
       ▼                           ▼
┌─────────────────┐       ┌─────────────────┐
│ viscos-auth     │       │ viscos-api      │
│ - Token storage │       │ - REST client   │
│ - Keyring       │◄──────┤ - Rate limit    │
│ - MFA           │       │ - Super-Props   │
└────────┬────────┘       └────────┬────────┘
         │                         │
         └────────────┬────────────┘
                      ▼
            ┌─────────────────┐
            │ Discord API     │
            │ (api.discord...)│
            └─────────────────┘
```

---

## 2. Workspace'e Crate Ekleme

> **Mimari karar (Haziran 2026):** Discord REST katmanı için sıfırdan `reqwest` yerine **`twilight-rs` sub-crate'leri** kullanılacak. Tam gerekçe: [`docs/DECISIONS.md` ADR-0008](../DECISIONS.md). Özet: AI-yazım riski, Discord protocol drift, modern transport (tokio-websockets), type-safe model katmanı, rate-limiting dahil.
>
> **Auth stack (Haziran 2026 — ADR-0011):** `keyring 2.3` stale olduğu için **4.0 mimarisine** geçildi: `keyring-core 0.7` + `windows-native-keyring-store 1.1` (her ikisi de `default-features = false` → `regex` dependency yok, ~1+ MB binary tasarrufu). Bellek hijyeni `secrecy 0.10` (serde feature) + `zeroize 1` (derive feature) zorunlu. Encryption Varyant A (DPAPI/Keyring) default, Varyant B (Argon2id passphrase) v2.0'da opt-in. Multi-account v1'den itibaren altyapı (`user = user_id`). MFA backup codes eklendi. Captcha için tarayıcı redirect stratejisi (headless browser yok). Tam gerekçe: [`viscos_auth_research.md`](./viscos_auth_research.md).

```toml
[workspace]
members = [
    "crates/viscos-core",
    "crates/viscos-config",
    "crates/viscos-error",
    "crates/viscos-log",
    "crates/viscos-api",       # YENİ
    "crates/viscos-auth",      # YENİ
    "crates/viscos-shell",
    "crates/viscos-webview",
    "crates/viscos-ipc",
    "crates/viscos-watchdog",
    "crates/viscos",
]

[workspace.dependencies]
# Discord API — twilight-rs (modüler, sadece REST + model)
twilight-model = { version = "0.17", default-features = false, features = ["tracing"] }
twilight-http = { version = "0.17", default-features = false, features = ["rustls-platform-verifier", "simd-json"] }

# Auth — keyring-core 4.0 mimarisi (ADR-0011)
keyring-core = { version = "0.7", default-features = false }                 # search feature kapalı → regex yok
windows-native-keyring-store = { version = "1.1", default-features = false } # search feature kapalı
# v2.0'da eklenecek (cross-platform):
# apple-native-keyring-store = { version = "1", default-features = false }
# dbus-secret-service-keyring-store = { version = "1", default-features = false }

# MFA
totp-rs = { version = "5.7", default-features = false, features = ["zeroize"] }
# (sadece verify; TOTP secret üretmiyoruz, kullanıcının authenticator'ı üretiyor)

# Bellek hijyeni — Secret<String> + ZeroizeOnDrop zorunlu
secrecy = { version = "0.10", features = ["serde"] }
zeroize = { version = "1", features = ["derive"] }

# DAVE E2EE — ADR-0012 §4 (compile-time API surface sabitleme, runtime'da kullanılmaz)
# Snazzah/davey, MIT, Rust MLS + OpenMLS implementasyonu
# Faz 7'de native voice eklenirse aktif feature olur
davey = { version = "0.1", optional = true }

# QR login (UI render)
qrcode = "0.14"

# v2.0 opt-in passphrase wrapper (Faz 5+ / ADR-0011 Varyant B):
# argon2 = { version = "0.5", features = ["std"] }
# aes-gcm = "0.10"
# age = "0.11"  # backup/export envelope
```

**Alınmayan twilight sub-crate'ler (bilinçli dışlama):**
- `twilight-cache-inmemory` ❌: Faz 4'te moka+SQLite+foyer planlanmış
- `twilight-gateway` ❌: Faz 3'te ayrı plan, kendi crate'inde
- `twilight-voice` ❌: Faz 7'de DAVE E2EE özel implementasyon
- `twilight-standby` / `twilight-mention` / `twilight-util` ❌: User client'ta gereksiz

**Alınmayan keyring sub-crate'ler:**
- `keyring 2.3` ❌: Stale (ADR-0011), 4.0 mimarisine geçildi
- `keyring-core`'un `search` feature'ı ❌: `regex` dependency'sini alır (~1+ MB), 25 MB bütçeyi zorlar
- `windows-native-keyring-store`'un `search` feature'ı ❌: aynı gerekçe

---

## 3. `viscos-api` (REST Client — twilight-http adaptörü)

> **Tasarım notu:** `twilight_http::Client` zaten production-tested, rate-limit ve X-Super-Properties dahil. Viscos'un yaptığı: (a) twilight'ın `Client` struct'ını kendi config + auth flow'umuza bağlamak, (b) twilight'ın `Error` tipini `viscos-error::ViscosError::Api`'ye map etmek, (c) twilight-model'in typed objelerini `viscos-core::events` API'sine çevirmek.

### 3.1 `crates/viscos-api/src/rest.rs`

```rust
use twilight_http::{
    Client as TwilightClient,
    request::Request,
    error::Error as TwilightError,
};
use twilight_model::{
    id::Id,
    user::User as TwilightUser,
    guild::Guild as TwilightGuild,
    channel::Channel as TwilightChannel,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, error};
use viscos_error::{Result, ViscosError};
use viscos_core::events::UserId;

/// Viscos REST adaptörü — twilight_http::Client'i sarmalayan ince katman.
pub struct RestClient {
    inner: Arc<TwilightClient>,
    token: Arc<tokio::sync::RwLock<Option<String>>>,
    // X-Super-Properties: Web client fingerprint (twilight bunu otomatik yapmaz,
    // Viscos browser-parity için kendi üretir)
    super_properties: String,
}

impl RestClient {
    pub fn new() -> Result<Self> {
        // twilight_http::Client::builder() sadece proxy ve timeout ayarları yapar.
        // Token, bearer header'ları request başına eklenir.
        let inner = TwilightClient::builder()
            .timeout(Duration::from_secs(30))
            .build();
        
        let inner = Arc::new(inner);
        let super_properties = generate_super_properties();
        
        Ok(Self {
            inner,
            token: Arc::new(tokio::sync::RwLock::new(None)),
            super_properties,
        })
    }
    
    pub fn set_token(&self, token: String) {
        // Twilight: token set edilmez, her request'te bearer_auth header'ı eklenir.
        // Viscos: token'ı saklarız, request helper'larımız header ekler.
        *self.token.blocking_write() = Some(token);
    }
    
    /// twilight_http::Error → ViscosError adaptörü.
    /// Rate limit durumunda Retry-After header'ı parse edilir ve iç state'e yansıtılır.
    async fn map_error(&self, route: &str, err: TwilightError) -> ViscosError {
        match err.kind() {
            twilight_http::error::ErrorType::Response { status, .. } if status.get() == 429 => {
                // twilight zaten 429'da global rate limit'i handle eder ve otomatik retry yapar.
                // Burada sadece loglarız.
                error!(route, "429 rate limit (twilight will retry internally)");
                ViscosError::Api(ApiError {
                    code: 429,
                    message: "rate limited".to_string(),
                    retry_after: Some(1.0),
                })
            }
            _ => ViscosError::Api(ApiError {
                code: 500,
                message: format!("twilight error: {:?}", err),
                retry_after: None,
            }),
        }
    }
    
    /// User bilgisini çek — token validation için kullanılır.
    pub async fn get_current_user(&self) -> Result<viscos_core::events::User> {
        let token = self.token.read().await.clone()
            .ok_or_else(|| ViscosError::Auth("token not set".into()))?;
        
        let req = self.inner
            .user(Id::new(1)) // placeholder; twilight self-current-user için ayrı route kullanır
            .build();
        // ... gerçek implementasyon: twilight_http::client::Client::user() ile değil,
        // twilight'ın kendi `/users/@me` route'unu çağırır (raw request).
        // Aşağıdaki Bölüm 3.2'de detay var.
        todo!()
    }
    
    /// Mesaj gönder — typed ChannelId + content, twilight otomatik JSON serialize eder.
    pub async fn create_message(
        &self,
        channel_id: Id<viscos_core::events::ChannelMarker>,
        content: &str,
    ) -> Result<viscos_core::events::Message> {
        let token = self.token.read().await.clone()
            .ok_or_else(|| ViscosError::Auth("token not set".into()))?;
        
        let twilight_msg = self.inner
            .create_message(channel_id)
            .content(content)
            .await
            .map_err(|e| self.map_error("create_message", e).await)?
            .model()
            .await
            .map_err(|e| ViscosError::Api(ApiError {
                code: 500,
                message: format!("decode error: {:?}", e),
                retry_after: None,
            }))?;
        
        Ok(viscos_core::events::Message::from(twilight_msg))
    }
    
    // get, post, put, delete: twilight_http::Client zaten fluent builder API'si veriyor.
    // Viscos sadece typed wrapper'larını yazar.
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("discord api error {code}: {message}")]
    Discord { code: i32, message: String, retry_after: Option<f64> },
    #[error("decode error: {0}")]
    Decode(String),
    #[error("twilight error: {0}")]
    Twilight(String),
}
```

> **Not:** `viscos_error::ViscosError::Api(ApiError)` artık `ApiError` struct'ını içerir (mevcut plandaki `ApiError` enum'u yerine). Twilight kendi hata tipini verir, biz typed adaptörümüze çeviririz.

### 3.2 twilight-http Kullanım Detayları

**Fluent builder pattern (twilight'ın doğal API'si):**
```rust
// Mevcut Discord mesajı güncelle
let msg = client.update_message(channel_id, message_id)
    .content(Some("yeni içerik"))
    .await?
    .model()
    .await?;

// Slash command interaction response gönder (Faz 7+ gerekli değil ama API'si hazır)
client.interaction_callback(interaction_id, interaction_token)
    .response(|r| r.content("pong"))
    .await?;

// Reaction ekle
client.create_reaction(channel_id, message_id, &emoji)
    .await?;
```

**Avantajlar vs custom reqwest:**
- ✅ Rate limit `X-RateLimit-*` header'ları otomatik parse + sıraya alma
- ✅ Global rate limit (`x-ratelimit-global: true`) otomatik handle
- ✅ Per-route + per-bucket bucket yönetimi twilight'ın iç state'inde
- ✅ HTTP 503/504 otomatik exponential backoff
- ✅ 429 response'larda `Retry-After` header'ı otomatik işlenir
- ✅ brotli/gzip decompression dahil
- ✅ SIMD-accelerated JSON parsing (`simd-json` feature, Faz 4 medya response'larında önemli)

### 3.3 twilight-model → viscos-core Adaptör Katmanı

`twilight-model` Discord'un tüm API objelerini typed struct'lar olarak verir. Viscos'un kendi `viscos-core` event'leri var (master index Bölüm 5 — `GatewayEvent::MessageCreate(Message)`, `User`, `Guild` vb.). Adaptör katmanı (`From` impl'leri):

```rust
// crates/viscos-api/src/convert.rs
use twilight_model as tw;
use viscos_core::events as vc;

impl From<tw::user::User> for vc::User {
    fn from(u: tw::user::User) -> Self {
        Self {
            id: vc::UserId::new(u.id.get()),
            username: u.name,
            discriminator: u.discriminator.unwrap_or_else(|| "0".into()),
            avatar: u.avatar.map(|h| h.to_string()),
            bot: u.bot.unwrap_or(false),
        }
    }
}

impl From<tw::guild::Guild> for vc::Guild { /* ... */ }
impl From<tw::channel::Channel> for vc::Channel { /* ... */ }
impl From<tw::channel::Message> for vc::Message { /* ... */ }
```

**Neden adaptör:**
1. `viscos-core` saf domain type — twilight'a bağımlılık yok (dependency graph temiz)
2. Twilight'ın `Option<HashMap<...>>` gibi karmaşık tipleri Viscos'un düz struct'larına çevrilir
3. Cache (Faz 4) twilight tipini değil Viscos tipini tutar
4. Frontend (WebView2) Viscos tipini alır, twilight detayı görmez

### 3.3.1 X-Super-Properties Detay (ADR-0011)

Discord Web client fingerprint'i `X-Super-Properties` header'ında (base64-encoded JSON) gönderilir. Viscos bu header'ı her REST + Gateway request'inde ekler.

**Statik alanlar** (`crates/viscos-auth/src/super_properties.rs`):

```rust
pub const SUPER_PROPERTIES_TEMPLATE: &str = r#"{
    "os": "Windows",
    "browser": "Viscos",
    "device": "",
    "system_locale": "en-US",
    "browser_user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "browser_version": "120.0.0.0",
    "os_version": "10",
    "referrer": "",
    "referring_domain": "",
    "referrer_current": "",
    "referring_domain_current": "",
    "release_channel": "stable",
    "client_build_number": 0,           // ← runtime'da güncellenir
    "client_event_source": null,
    "has_client_mods": false,
    "timezone_offset_minutes": -180
}"#;
```

**Dinamik alanlar:**

1. **`client_build_number`** senkronizasyonu — Haftalık GitHub Action (`.github/workflows/sync-build-number.yml`, `cron: 0 6 * * 1`):
   - `https://discord.com/app` JS bundle'ından `release_channel` ve `build_number` parse
   - PR otomatik açar (`super_properties.rs` patch)
   - İnsan review + merge → 1 hafta Discord drift toleransı
   - **Ban riski azaltma:** Build_number eskiyse Discord "client modified" heuristic'i tetikler

2. **WebGL/Canvas hash'i** — `crates/viscos-webview/src/fingerprint.rs`:
   ```rust
   pub fn capture_webgl_hash(renderer: &WebViewBackend) -> String {
       // Win11 default CEF: cef BrowserHost → WebGL context → UNMASKED_RENDERER_WEBGL
       // Win10 default WebView2: ICoreWebView2 → WebGL → aynı
       // SHA-256 hash, base64 encode
   }
   ```
   - CEF/WebView2 renderer'ı headless context oluşturur, WebGL extension'ları sorgular, hash alır
   - `viscos-auth::super_properties` runtime'da bu hash'i template'e inject eder
   - **Gerekçe:** Native UI wrapper olmamıza rağmen "Discord Web tarayıcıda çalışıyor" görüntüsü

3. **Diğer fingerprint alanları** (`screen`, `locale`, `timezone_offset`) hardcoded — birebir Discord Web'in değerleriyle aynı olmalı, değişirse suspicious.

**Doğrulama:** `crates/viscos-auth/tests/super_properties_format.rs`:
- Base64 decode → valid JSON parse
- `client_build_number >= DiscordWeb build_number` (haftalık senkronizasyon sonrası)
- `os = "Windows"`, `browser = "Viscos"` (Viscos'un kendi kimliği — Discord "browser" alanından client'ı tanır)

### 3.4 API Endpoint'leri (Faz 2'de)

twilight'ın fluent builder API'si ile:

| Endpoint | twilight çağrısı | Kullanım |
|----------|------------------|----------|
| `GET /users/@me` | `client.current_user()` (built-in) | Token validation |
| `POST /auth/login` | twilight yok (manuel) | Email/şifre login |
| `POST /auth/mfa/totp` | twilight yok (manuel) | MFA challenge |
| `GET /users/@me/guilds` | `client.guilds().await` (paginated) | Sunucu listesi |
| `GET /users/@me/channels` | `client.private_channels().await` (paginated) | DM listesi |
| `GET /channels/{id}/messages` | `client.messages(channel_id).await` | Mesaj geçmişi |
| `POST /channels/{id}/messages` | `client.create_message(channel_id).content(c).await` | Mesaj gönder |
| `GET /guilds/{id}/channels` | `client.guild_channels(guild_id).await` | Sunucu kanalları |
| `GET /guilds/{id}/members` | `client.guild_members(guild_id).limit(1000).await` | Üye listesi |

`/auth/login` ve `/auth/mfa/totp` Discord'un özel auth endpoint'leri — twilight bunları **sağlamaz** (bot token varsayımı). Bu iki endpoint Viscos tarafından manuel `reqwest` ile yazılır (Faz 2'de ~50 satır, sadece login akışı).

### 3.5 Test Stratejisi

| Test | Tip | Kabul |
|------|-----|-------|
| twilight client builder | Unit | Config doğru, timeout set |
| Token bearer auth | Unit | twilight request'ine header eklenir |
| 429 retry (twilight internal) | Integration (mock) | twilight otomatik retry yapar, sonuç success |
| X-Super-Properties | Unit | Base64 valid JSON, `client_build_number` template geçerli |
| X-Super-Properties build_number sync | Integration (mock) | GitHub Action parse + PR açma akışı dry-run |
| WebGL hash capture (CEF/WebView2) | Integration | Hash 64 hex char, deterministic aynı renderer'da |
| twilight-model → viscos-core `From` impl | Unit | Tüm alanlar doğru map |
| Auth endpoint mock (reqwest) | Integration | Login success + MFA + captcha |
| Captcha redirect akışı | Integration (mock) | `LoginResult::CaptchaRequired { url }` → shell UI render |

---

## 4. `viscos-auth` (Token, Keyring, MFA)

> **Mimari karar (Haziran 2026 — ADR-0011):** `keyring 2.3` → `keyring-core 0.7` + `windows-native-keyring-store 1.1` (4.0 mimarisi). `Secret<String>` + `ZeroizeOnDrop` tüm secret materyal için zorunlu. Multi-account v1'den itibaren altyapı (`user = user_id`). MFA backup codes ayrı keyring alanı. Captcha: tarayıcıya yönlendir, token yapıştır.
>
> **Frontend mimari bağlamı (Haziran 2026 — ADR-0012):** Viscos hibrit (WebView + native shell). DAVE E2EE WebRTC encoded transform API ile browser'da bedava çalışır. Faz 7'de native voice eklenirse `davey` (Rust MLS) optional dependency olarak zaten auth crate'inde — compile-time API surface sabitlenmiş olur.

### 4.0.5 DAVE E2EE Optional Dependency (ADR-0012 §4)

`crates/viscos-auth/Cargo.toml`:

```toml
[features]
default = []
dave = ["dep:davey"]

[dependencies]
davey = { version = "0.1", optional = true }
```

**Gerekçe:** `Snazzah/davey` (Rust MLS implementasyonu, MIT, aktif — son release `rs-0.1.3` Mart 2026) Discord'un `discord/dave-protocol` referansını OpenMLS üzerinden implemente ediyor. Faz 7'de native voice/video eklendiğinde major API drift riski sıfırlanır — `davey` v0.1.x derlenmiş ve import'ları doğrulanmış olur.

**Kullanım:** Faz 2.0'da hiçbir runtime path'i yok (sadece `cargo check`). Faz 7'de `--features dave` ile native voice gateway'sine bağlanır.

**Lisans uyumu:** `davey` MIT → GPL-3.0 uyumlu (cargo-deny check licenses yeşil).

### 4.1 `crates/viscos-auth/src/lib.rs`

```rust
use keyring_core::{Entry, set_default_store, unset_default_store, Error as KeyringError};
use secrecy::{Secret, ExposeSecret};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::ZeroizeOnDrop;
use std::sync::Arc;
use tracing::{info, warn};

const SERVICE_NAME: &str = "Viscos";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("keyring error: {0}")]
    Keyring(#[from] KeyringError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("no account found for user {0}")]
    AccountNotFound(String),
    #[error("token validation failed: {0}")]
    ValidationFailed(String),
    #[error("keyring store not initialized (call AuthStorage::install() at startup)")]
    StoreNotInitialized,
}

/// In-memory hesap state'i.
/// `Secret<String>` + `ZeroizeOnDrop` → memory dump baseline savunma.
#[derive(ZeroizeOnDrop)]
pub struct StoredAccount {
    pub user_id: String,            // Discord snowflake (u64 string)
    pub username: String,           // tag (username#discriminator veya yeni unique username)
    pub token: Secret<String>,      // DPAPI arkası keyring'de, in-memory Secret
    pub mfa_backup_hashes: Vec<Secret<String>>,  // Argon2 PHC strings (v1)
    pub created_at: i64,            // Unix timestamp
    pub last_validated_at: i64,     // 401 handling için
}

/// Disk formatı (keyring entry'sinde JSON).
#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
struct SerializedAccount {
    user_id: String,
    username: String,
    token: String,
    mfa_backup_hashes: Vec<String>,  // Argon2 PHC strings
    created_at: i64,
    last_validated_at: i64,
}

pub struct AuthStorage {
    _private: (),  // Store runtime'da global, struct sadece helper
}

impl AuthStorage {
    /// Platform-specific store'u kur. v1: Windows native (DPAPI arkası).
    /// v2+ Linux/macOS'ta Store trait'ini dispatch eder.
    pub fn install() -> Result<(), AuthError> {
        #[cfg(target_os = "windows")]
        {
            use windows_native_keyring_store::Store;
            set_default_store(Store::new()?).map_err(AuthError::Keyring)?;
            info!("keyring-core: windows-native (DPAPI) store initialized");
        }
        #[cfg(target_os = "macos")]
        {
            // v2.0: apple-native-keyring-store
            return Err(AuthError::ValidationFailed("macOS not yet supported in v1".into()));
        }
        #[cfg(target_os = "linux")]
        {
            // v2.0: dbus-secret-service-keyring-store
            return Err(AuthError::ValidationFailed("Linux not yet supported in v1".into()));
        }
        Ok(())
    }

    pub fn shutdown() {
        unset_default_store();
    }

    /// v1'de tek active account. user_id bazlı key'leme ile v2'de 0 refactor.
    pub fn store_account(&self, account: &StoredAccount) -> Result<(), AuthError> {
        let entry = Entry::new(SERVICE_NAME, &account.user_id)?;
        let ser = SerializedAccount {
            user_id: account.user_id.clone(),
            username: account.username.clone(),
            token: account.token.expose_secret().clone(),
            mfa_backup_hashes: account.mfa_backup_hashes.iter()
                .map(|s| s.expose_secret().clone())
                .collect(),
            created_at: account.created_at,
            last_validated_at: account.last_validated_at,
        };
        let json = serde_json::to_string(&ser)?;
        entry.set_password(&json)?;
        Ok(())
    }

    pub fn load_account(&self, user_id: &str) -> Result<Option<StoredAccount>, AuthError> {
        let entry = Entry::new(SERVICE_NAME, user_id)?;
        match entry.get_password() {
            Ok(json) => {
                let ser: SerializedAccount = serde_json::from_str(&json)?;
                Ok(Some(StoredAccount {
                    user_id: ser.user_id,
                    username: ser.username,
                    token: Secret::new(ser.token),
                    mfa_backup_hashes: ser.mfa_backup_hashes.into_iter()
                        .map(Secret::new)
                        .collect(),
                    created_at: ser.created_at,
                    last_validated_at: ser.last_validated_at,
                }))
            }
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_account(&self, user_id: &str) -> Result<(), AuthError> {
        let entry = Entry::new(SERVICE_NAME, user_id)?;
        match entry.delete_credential() {
            Ok(()) => {
                info!(user_id, "keyring account deleted");
                Ok(())
            }
            Err(KeyringError::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// 401 alındığında çağrılır. Account silinir, kullanıcıya re-login UI'ı gösterilir.
    pub fn handle_401(&self, user_id: &str) -> Result<(), AuthError> {
        warn!(user_id, "Discord 401 received — invalidating token");
        self.delete_account(user_id)
    }

    /// MFA backup code doğrula (Argon2 verify).
    pub fn verify_backup_code(&self, user_id: &str, code: &str) -> Result<bool, AuthError> {
        let account = self.load_account(user_id)?
            .ok_or_else(|| AuthError::AccountNotFound(user_id.into()))?;
        use argon2::{Argon2, PasswordHash, PasswordVerifier};
        for hash in &account.mfa_backup_hashes {
            let hash_str = hash.expose_secret();
            if let Ok(parsed) = PasswordHash::new(hash_str) {
                if Argon2::default().verify_password(code.as_bytes(), &parsed).is_ok() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
```

**`keyring-core` 4.0 API farkları (eski `keyring 2.3`'ten):**
- `Entry::new(service, user)` aynı → geriye dönük uyumlu
- `set_default_store(Store::new()?)` + `unset_default_store()` → **runtime'da bir kez kurulur**, test'te mock store takılabilir
- `Entry::set_password(&str)` / `get_password() -> Result<String>` → aynı, Result type'ı `keyring_core::Error`
- Multi-store desteği: `Entry::new_with_target` ile custom `target_name` (Viscos şu an gerekmiyor, v2'de düşünülür)

**Test/mock stratejisi:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use keyring_core::mock;

    fn setup_mock_store() {
        // Her test'te izole mock store (global state, dikkatli handle)
        let _ = keyring_core::set_default_store(mock::Store::new());
    }

    #[test]
    fn store_then_load_roundtrip() {
        setup_mock_store();
        let storage = AuthStorage { _private: () };
        let account = StoredAccount {
            user_id: "123456789012345678".into(),
            username: "testuser".into(),
            token: Secret::new("secret-token".into()),
            mfa_backup_hashes: vec![],
            created_at: 1718700000,
            last_validated_at: 1718700000,
        };
        storage.store_account(&account).unwrap();
        let loaded = storage.load_account(&account.user_id).unwrap().unwrap();
        assert_eq!(loaded.user_id, account.user_id);
        assert_eq!(loaded.token.expose_secret(), "secret-token");
    }
}
```

> **Önemli not:** `keyring-core` mock store'u her test'te izole olmaz (global state). CI'da `cargo nextest` test-parallelism'i kapatılabilir veya `set_default_store` mutex ile korunabilir (`parking_lot::Mutex` 1 KB). Detay `phase-0.5`'teki test convention'a bırakıldı.

### 4.2 Login Akışları

**Login result enum (captcha dahil):**

```rust
#[derive(Debug, Clone)]
pub enum LoginResult {
    /// Token alındı, account storage'a yazıldı
    Success(StoredAccount),
    /// MFA TOTP kodu gerekli (Discord 6 hane istiyor)
    MfaRequired { ticket: String, mfa_type: MfaType },
    /// MFA backup code gerekli (kullanıcı TOTP erişimi kaybetmiş)
    MfaBackupCodeRequired { ticket: String },
    /// Captcha gerekli (Cloudflare Turnstile / hCaptcha) → tarayıcıya yönlendir
    CaptchaRequired { url: String, sitekey: String, rqtoken: String },
    /// QR login session süresi doldu
    QrExpired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaType { Totp, Sms }  // Discord 2024'ten beri SMS kaldırıldı, sadece Totp aktif
```

**Email/Şifre (captcha handling ile):**

```rust
pub async fn login_email_password(
    api: &RestClient,
    storage: &AuthStorage,
    email: &str,
    password: &str,
) -> Result<LoginResult> {
    let resp: LoginResponse = api.post_json("/auth/login", &serde_json::json!({
        "login": email,
        "password": password,
    })).await?;
    
    match resp {
        LoginResponse::Success { token, user_id, username } => {
            let account = StoredAccount {
                user_id,
                username,
                token: Secret::new(token),
                mfa_backup_hashes: vec![],
                created_at: now_unix(),
                last_validated_at: now_unix(),
            };
            storage.store_account(&account)?;
            Ok(LoginResult::Success(account))
        }
        LoginResponse::MfaRequired { ticket, mfa_type } => {
            Ok(LoginResult::MfaRequired { ticket, mfa_type })
        }
        LoginResponse::CaptchaRequired { captcha_url, sitekey, rqtoken } => {
            // ADR-0011 stratejisi: tarayıcıya yönlendir, token yapıştır
            Ok(LoginResult::CaptchaRequired {
                url: captcha_url,
                sitekey,
                rqtoken,
            })
        }
    }
}
```

**QR Kod Login (Discord mobil app):**

```rust
pub async fn login_qr_start(api: &RestClient) -> Result<QrLoginSession> {
    let resp: QrLoginResponse = api.post_json("/auth/qr-login/start", &serde_json::json!({})).await?;
    let qr = QrCode::new(resp.url.as_bytes())?;
    let png = qr.render::<Luma<u8>>().build();
    Ok(QrLoginSession {
        session_id: resp.session_id,
        qr_png: png,
    })
}

pub async fn login_qr_poll(api: &RestClient, session_id: &str) -> Result<LoginResult> {
    loop {
        let resp: QrPollResponse = api.get(&format!("/auth/qr-login/{}", session_id)).await?;
        match resp {
            QrPollResponse::Pending => tokio::time::sleep(Duration::from_secs(2)).await,
            QrPollResponse::Success { token, user_id, username } => {
                let account = StoredAccount {
                    user_id,
                    username,
                    token: Secret::new(token),
                    mfa_backup_hashes: vec![],
                    created_at: now_unix(),
                    last_validated_at: now_unix(),
                };
                return Ok(LoginResult::Success(account));
            }
            QrPollResponse::Expired => return Ok(LoginResult::QrExpired),
            QrPollResponse::MfaRequired { ticket, mfa_type } => {
                // QR login sonrası MFA — hesap 2FA aktifse ikinci adım
                return Ok(LoginResult::MfaRequired { ticket, mfa_type });
            }
        }
    }
}
```

**Token Yapıştırma (captcha fallback dahil):**

```rust
pub async fn login_token(
    api: &RestClient,
    storage: &AuthStorage,
    token: &str,
) -> Result<StoredAccount> {
    api.set_token(token.to_string());
    let user: User = api.get("/users/@me").await?;  // 401 → validation failed
    let account = StoredAccount {
        user_id: user.id.to_string(),
        username: user.username,
        token: Secret::new(token.to_string()),
        mfa_backup_hashes: vec![],
        created_at: now_unix(),
        last_validated_at: now_unix(),
    };
    storage.store_account(&account)?;
    Ok(account)
}
```

> **Not:** `login_token` aynı zamanda **captcha redirect sonrası fallback**'i: `LoginResult::CaptchaRequired { url, .. }` → shell "tarayıcıda giriş yap" modal'ı açar, kullanıcı Discord DevTools'tan token'ı alır, `login_token(storage, token)` çağrılır.

### 4.3 MFA (TOTP + Backup Codes)

**TOTP verify (sadece ileri, üretmiyoruz — kullanıcının authenticator'ı üretir):**

```rust
use totp_rs::Algorithm;

pub async fn login_mfa(
    api: &RestClient,
    storage: &AuthStorage,
    ticket: &str,
    code: &str,
) -> Result<LoginResult> {
    // totp-rs verify YAPMIYOR — sadece Discord API'ına iletiyoruz
    // (Discord TOTP secret'ı kullanıcı tarafında, Viscos generate etmiyor)
    let resp: MfaResponse = api.post_json("/auth/mfa/totp", &serde_json::json!({
        "ticket": ticket,
        "code": code,
    })).await?;

    match resp {
        MfaResponse::Success { token, user_id, username } => {
            let backup_codes = extract_backup_codes(&resp);  // Discord response'undan al
            let mfa_backup_hashes = hash_backup_codes(&backup_codes);

            let account = StoredAccount {
                user_id,
                username,
                token: Secret::new(token),
                mfa_backup_hashes: mfa_backup_hashes.into_iter().map(Secret::new).collect(),
                created_at: now_unix(),
                last_validated_at: now_unix(),
            };
            storage.store_account(&account)?;
            Ok(LoginResult::Success(account))
        }
        MfaResponse::InvalidCode => Err(AuthError::ValidationFailed("invalid MFA code".into())),
    }
}
```

**Backup codes (Discord 8 karakterli alphanumeric, MFA kurtarma):**

```rust
use argon2::{Argon2, PasswordHasher, password_hash::{SaltString, rand_core::OsRng}};
use rand::Rng;

/// Discord 10 backup code üretir (resmi davranış: MFA kurulumunda kullanıcıya verir).
pub fn generate_backup_codes() -> Vec<String> {
    (0..10).map(|_| {
        let code: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(9)
            .map(char::from)
            .collect();
        format!("{}-{}", &code[..4], &code[4..])  // "abcd-12345" formatı
    }).collect()
}

/// Backup code'ları Argon2 PHC string olarak hash'le (keyring entry'sinde).
/// Plaintext asla disk'e yazılmaz.
pub fn hash_backup_codes(codes: &[String]) -> Vec<String> {
    codes.iter().map(|c| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(c.as_bytes(), &salt)
            .expect("Argon2 hash")
            .to_string()
    }).collect()
}

/// Backup code doğrula (TOTP erişimi kaybedildiğinde).
pub async fn login_mfa_backup(
    api: &RestClient,
    storage: &AuthStorage,
    user_id: &str,
    ticket: &str,
    code: &str,
) -> Result<LoginResult> {
    // 1. Keyring'deki hash'lerden biriyle eşleşiyor mu kontrol et
    if !storage.verify_backup_code(user_id, code)? {
        return Err(AuthError::ValidationFailed("invalid backup code".into()));
    }
    // 2. Discord'a gönder
    let resp: MfaResponse = api.post_json("/auth/mfa/totp", &serde_json::json!({
        "ticket": ticket,
        "code": code,
        "backup": true,
    })).await?;
    // 3. Success path yukarıdaki gibi
    // ...
}
```

**UI tarafı (Faz 5 polish, Faz 2'de fonksiyonel):**
- MFA setup ekranı: QR kod render + backup code listesi (göster / yenile / `.txt` indir)
- MFA verify ekranı: 6-hanelik TOTP input
- "TOTP cihazımı kaybettim" linki → backup code input
- Backup code < 10 kaldığında settings'te uyarı

> **Not:** SMS MFA Discord 2024'ten beri kaldırıldı, sadece TOTP + backup codes destekleniyor. `MfaType::Sms` enum'da kalsa da `MfaType::Totp` ile aynı path.

---

## 5. Login UI (iced)

**İlk basit versiyon (Faz 5 öncesi):** Modal pencerede iced form.

```rust
// crates/viscos-shell/src/login.rs
use iced::{Element, TextInput, Button, Column, Text, text_input, button};

pub struct LoginView {
    email: String,
    password: String,
    mfa_code: String,
    error: Option<String>,
    state: LoginState,
}

#[derive(Debug, Clone)]
pub enum LoginState {
    EmailPassword,
    /// TOTP 6-hane kod girişi
    MfaRequired { ticket: String, mfa_type: MfaType },
    /// Backup code girişi (TOTP cihazı kayıp senaryosu)
    MfaBackupCode { ticket: String },
    /// Captcha — kullanıcıyı tarayıcıya yönlendir, sonra token yapıştırsın
    CaptchaRequired { url: String, sitekey: String, rqtoken: String, manual_token: String },
    /// QR kod render + polling
    QrLogin { png: Vec<u8> },
    /// Direkt token yapıştırma
    Token { manual_token: String },
}

#[derive(Debug, Clone)]
pub enum LoginMessage {
    EmailChanged(String),
    PasswordChanged(String),
    MfaChanged(String),
    MfaBackupCodeChanged(String),
    CaptchaTokenPasted(String),
    Submit,
    QrStart,
    QrPoll,
    TokenSubmit,
    /// "Discord captcha istiyor" → sistem tarayıcısını aç
    CaptchaOpenBrowser { url: String },
}

impl LoginView {
    pub fn view(&self) -> Element<LoginMessage> {
        match &self.state {
            LoginState::EmailPassword => Column::new()
                .push(Text::new("Email"))
                .push(TextInput::new("email", &self.email, LoginMessage::EmailChanged))
                .push(Text::new("Password"))
                .push(TextInput::new("password", &self.password, LoginMessage::PasswordChanged).password())
                .push(Button::new(Text::new("Login")).on_press(LoginMessage::Submit))
                .into(),
            LoginState::CaptchaRequired { url, manual_token, .. } => Column::new()
                .push(Text::new("Discord captcha doğrulaması istiyor."))
                .push(Text::new("Tarayıcıda giriş yap → DevTools → Network → herhangi bir istek → Authorization header → token'ı kopyala."))
                .push(Button::new(Text::new("Tarayıcıyı Aç"))
                    .on_press(LoginMessage::CaptchaOpenBrowser { url: url.clone() }))
                .push(Text::new("Token:"))
                .push(TextInput::new("token", manual_token, LoginMessage::CaptchaTokenPasted))
                .push(Button::new(Text::new("Giriş Yap")).on_press(LoginMessage::TokenSubmit))
                .into(),
            // ...
        }
    }
}
```

**ToS Disclaimer (canonical metin, ADR-0011):**

İlk açılışta modal (Faz 1'den):

```rust
pub fn first_run_disclaimer() -> Element<DisclaimerMessage> {
    Column::new()
        .push(Text::new("Viscos — Üçüncü Parti Discord İstemcisi").size(20))
        .push(Text::new(
            "Viscos, Discord'un RESMİ OLMAYAN bir istemcisidir. \
             Kullanıcı kendi hesabıyla giriş yapar; \
             ToS ihlali (otomasyon, scraping, mass DM) bu istemcinin tasarım amacı değildir \
             ve tüm sorumluluk kullanıcıya aittir. \
             Discord multi-layered detection (fingerprint + behavioral heuristics) ile \
             self-bot tespit edip banlayabilir."
        ))
        .push(Text::new("Devam ederek bu koşulları kabul ediyorsunuz."))
        .push(Button::new(Text::new("Anladım, devam et")).on_press(DisclaimerMessage::Accept))
        .into()
}
```

Aynı metin:
- `docs/DECISIONS.md` ADR-0011 Consequences bölümünde
- `README.md` → Disclaimer
- `Settings → About` (kalıcı)
- `cargo deny check licenses` GPL-3.0 proje kapsamında

Faz 5'te daha güzel UI. Faz 2'de sadece fonksiyonel.

---

## 6. Test Stratejisi (Faz 2.0)

| Test | Tip | Kabul |
|------|-----|-------|
| keyring-core store/load/delete | Integration | DPAPI (Windows native store) çalışıyor, mock store test'lerde izole |
| Multi-account farklı user_id'ler | Integration | user_id="A" ve "B" ayrı entry, çakışma yok |
| Secret<String> zeroize | Unit | Drop sonrası bellek temiz (miri test) |
| Email/password login (mock) | Integration | Başarı + MFA + captcha path'leri |
| Captcha redirect (mock) | Integration | `LoginResult::CaptchaRequired { url, .. }` → shell UI render |
| Captcha fallback to token paste | Integration | redirect sonrası `login_token()` success |
| Token validation (mock) | Integration | Geçerli token → user info, geçersiz → hata |
| 401 handling | Integration | handle_401() → keyring delete + re-login UI |
| QR login flow (mock) | Integration | Start + poll + success + MFA-required (2FA hesap) |
| MFA TOTP (mock) | Integration | 6 hane code → success |
| MFA backup code verify (mock) | Integration | Backup code Argon2 PHC match → success |
| Backup code regenerate | Integration | 10 yen kod üretildi, eski hash'ler invalid |
| Rate limit 429 (mock) | Integration | Retry-After doğru sleep |
| Super-properties format | Unit | Base64 → valid JSON, `client_build_number` template geçerli |
| WebGL hash capture | Integration | CEF/WebView2 → 64 hex char SHA-256 |

**Mock server:** `wiremock` crate. CI'da tüm auth flow'ları offline test.

---

## 7. Kabul Kriterleri (Definition of Done)

- [ ] Email/şifre login çalışıyor (gerçek hesap, lokal)
- [ ] Token storage `keyring-core` + `windows-native-keyring-store`'da (DPAPI, restart'tan sonra otomatik login)
- [ ] `Secret<String>` + `ZeroizeOnDrop` tüm token path'lerinde (miri testi temiz)
- [ ] QR kod login çalışıyor (lokal, mobil Discord ile)
- [ ] QR login → MFA (2FA hesap) akışı çalışıyor
- [ ] MFA TOTP çalışıyor
- [ ] MFA backup codes üretildi + keyring'de Argon2 PHC + verify çalışıyor
- [ ] Captcha redirect akışı: shell modal → tarayıcı aç → token yapıştır → success
- [ ] 401 handling: token geçersiz → keyring delete + re-login UI
- [ ] Multi-account farklı user_id'lerle ayrı keyring entry'ler (v1 altyapı)
- [ ] ToS disclaimer modal ilk açılışta görünüyor + kabul zorunlu
- [ ] Rate limit 429 doğru handle ediliyor
- [ ] Super-properties header valid (build_number Discord Web ile senkron, WebGL hash CEF/WebView2'den)
- [ ] Token validation çalışıyor (geçersiz token hata verir)
- [ ] `cargo clippy -- -D warnings` temiz
- [ ] Tüm auth test'leri geçer (mock server + keyring mock)
- [ ] Gerçek hesapta end-to-end login (acceptance test, lokal)

---

## 8. Karar Noktaları (Faz 2.0 Sonu — ADR-0011 ile çoğu kapatıldı)

> ✅ **ÇÖZÜLDÜ (ADR-0011):** Token storage encryption anahtarı
> - **Varyant A (DPAPI/Keyring) default v1**, Varyant B (Argon2id passphrase) **v2.0'da opt-in**
> - Gerekçe: Threat model %95 yeterli, passphrase UX öldürür, AI-yazılım projesi için over-engineering

> ✅ **ÇÖZÜLDÜ (ADR-0011):** MFA backup codes storage
> - Argon2 PHC string olarak `keyring` entry'sinde
> - 10 koddan az kaldığında UI uyarı, göster / yenile / `.txt` indir

> ✅ **ÇÖZÜLDÜ (ADR-0011):** Self-bot ToS disclaimer
> - Canonical metin ADR-0011 Consequences + README + Modal + Settings → 4 yerde tutarlı

> ✅ **ÇÖZÜLDÜ (ADR-0011):** Captcha handling
> - **Varyant önerilen:** "Tarayıcıya yönlendir" UI akışı — `discord.com/login` sistem tarayıcısında açılır, kullanıcı orada giriş yapar, token'ı DevTools'tan alıp Viscos'a yapıştırır
> - **Varyant reddedilen:** Headless browser (Playwright/headless_chrome) — +30+ MB binary, +3-4 gün geliştirme, AI-yazım riski

> 🔵 **İNSAN (kalan):** Login UI Faz 2'de mi, Faz 5'te mi?
> - Faz 2: Basit modal, fonksiyonel (önerilen — MVP demo'su mümkün)
> - Faz 5: Tam temalı login ekranı, native
> - Trade-off: Faz 2 demo'su mümkün, Faz 5 polish

> 🔵 **İNSAN (kalan):** Captcha UX — GIF / video tutorial ekle?
> - **Varyant A (önerilen):** Modal'da 1-2 cümle metin + Discord DevTools screenshot'u
> - **Varyant B:** Kısa GIF / video embed (Discord embedding için external CDN gerekir → privacy)
> - Trade-off: A yeterli mi yoksa B friction azaltır mı?

---

## 9. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| Discord account ban | Hesap kaybı | Rate limit'lere sıkı uyum (twilight dahili), user-agent doğru, fingerprint temiz; X-Super-Properties haftalık GitHub Action senkronizasyonu (build_number eskiyse Discord "client modified" heuristic'i tetikler) |
| Token leak (memory dump) | Hesap çalınması | `Secret<String>` + `ZeroizeOnDrop` tüm token path'lerinde (miri test); `keyring-core` DPAPI (default-features=false, `regex` yok); kod review'da `expose_secret()` call site'ları audit'lenebilir |
| Keyring platform farkı | Linux/macOS bug | `keyring-core 0.7` modüler mimari; v1 Windows-only, v2'de `apple-native-keyring-store` + `dbus-secret-service-keyring-store` eklenecek (CI matrix genişler); `keyring-core::mock::Store` test'lerde izole |
| `keyring-core 0.7` 1.0 değil | API drift (4 yıllık proje) | `cargo update` haftalık + AI PR review; major bump → 1 hafta geçiş (aynı strateji ADR-0008 twilight) |
| Login flow değişir (Discord güncelleme) | Kırılma | `/auth/login` + `/auth/mfa/totp` zaten twilight dışı (manuel `reqwest`); E2E test haftalık CI; JSON schema validate, regex yok |
| Captcha agresifleşme | Login kırılması | ADR-0011: tarayıcı redirect + token paste fallback; headless browser YOK (binary + geliştirme maliyeti); modal'da net talimat + DevTools screenshot |
| MFA backup codes kayıp | Hesap kurtarılamaz | Kodlar keyring'de Argon2 PHC (plaintext asla disk'te); UI "göster / yenile / `.txt` indir"; <10 kaldığında uyarı |
| 401 invalidation | Kullanıcı kafası karışır | `handle_401()` → keyring delete + re-login UI otomatik; kullanıcı "Çıkış Yap" demeden de tetiklenir |
| X-Super-Properties fingerprint rotation | Discord "client modified" heuristic | `client_build_number` haftalık GitHub Action sync; WebGL hash CEF/WebView2 renderer'dan (Faz 1.6 backend kararıyla uyumlu) |
| ToS self-bot tespit | Hesap ban | Disclaimer modal + README + Settings 4 yerde tutarlı (ADR-0011 canonical metin); twilight user-token modunda (bot token bucket'ı değil); rate-limit disiplini |
| Rate limit yanlış | 429 cascade | `twilight_http::Client` otomatik handle (X-RateLimit-*, 429, global); Viscos sadece error tipine map eder |
| twilight-rs breaking change | API drift | Pin `0.17.x`; `cargo update` haftalık + AI PR review; major bump → 1 hafta geçiş |

---

## 10. Çıkış → Faz 3.0

Bu faz tamamlandığında:
- Discord hesabıyla login olunabiliyor
- Token güvenli saklanıyor
- Rate limit'lere uyuluyor
- API client temel endpoint'ler için hazır

Faz 3.0 → Gateway WebSocket (gerçek zamanlı mesaj/event'ler).
