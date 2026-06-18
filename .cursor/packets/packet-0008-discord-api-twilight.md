# Implementation Packet — ADR-0008: Discord REST + Gateway — twilight-rs (sub-crate seçici entegrasyon)

## Header

- **ADR:** ADR-0008
- **Başlık:** Discord REST + Gateway — `twilight-rs` (sub-crate seçici entegrasyon)
- **Durum:** ✅ Accepted
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0008](../../docs/DECISIONS.md#adr-0008-discord-rest--gateway--twilight-rs-sub-crate-seçici-entegrasyon)
- **Önceki plan:** [`phase-2.0-discord-api.md`](../../.cursor/plans/phase-2.0-discord-api.md), [`phase-3.0-gateway.md`](../../.cursor/plans/phase-3.0-gateway.md)

## Hedef faz worker

**Bu packet iki worker'a bölünür:**

- **Auth+API worker, Faz 2.0, Dalga 1:** REST katmanı (`twilight-http`) + model katmanı (`twilight-model`) — `viscos-api` crate'inin oluşturulması, REST client kurulumu, adaptör layer (`ViscosError::Api`).
- **Gateway worker, Faz 3.0, Dalga 1:** Gateway katmanı (`twilight-gateway`) — WebSocket, sharding, session resume, zstd-stream, reconnect backoff.

**Öncelik sırası:** Faz 2.0 (REST) → Faz 3.0 (Gateway). Gateway Faz 3.0 olmadan da REST çalışabilir, test edilebilir.

## Uygulama adımları

### Faz 2.0 Dalga 1 (Auth+API worker)

1. **`Cargo.toml` `[workspace.dependencies]`** — twilight sub-crate'leri ekle:
   ```toml
   twilight-model = { version = "0.17", default-features = false, features = ["tracing"] }
   twilight-http = { version = "0.17", default-features = false, features = [
       "rustls-platform-verifier", "simd-json"
   ] }
   ```
   - `twilight-cache-inmemory` ❌ (Faz 4'te kendi cache'imiz).
   - `twilight-util`, `twilight-standby`, `twilight-mention`, `twilight-voice` ❌ (alınmayacak, ADR-0008 Consequences).

2. **`crates/viscos-api/Cargo.toml`** — workspace member'larına ekle:
   - `twilight-model = { workspace = true }`
   - `twilight-http = { workspace = true }`
   - `viscos-core = { path = "../viscos-core" }`
   - `viscos-error = { path = "../viscos-error" }`
   - `tokio = { workspace = true }`

3. **`crates/viscos-api/src/lib.rs`** — public API:
   ```rust
   pub mod auth;
   pub mod rest;
   pub mod events;

   pub use auth::{AuthStorage, login_with_token};
   pub use rest::{ViscosHttp, HttpError};
   pub use events::GatewayEvent;
   ```

4. **`crates/viscos-api/src/rest.rs`** — twilight-http wrapper:
   ```rust
   use twilight_http::Client;
   use crate::error::ApiError;

   pub struct ViscosHttp {
       client: Client,
       token: secrecy::Secret<String>,
   }

   impl ViscosHttp {
       pub fn new(token: Secret<String>) -> Self { ... }
       pub async fn current_user(&self) -> Result<twilight_model::user::User, ApiError> {
           self.client.current_user().await.map_err(...)
       }
       // Diğer REST method'ları (Faz 2.0 devam eden packet'lerde)
   }
   ```

5. **`crates/viscos-error/src/lib.rs`** — yeni variant:
   ```rust
   #[error("discord api error: {0}")]
   Api(#[from] ApiError),
   ```
   + `ApiError` wrapper (twilight'ın kendi hata tiplerini ViscosError'a map eder).

6. **Test:**
   - `tests/rest_integration.rs` (mock server): login → `current_user` çağrısı.
   - 25+ REST method'u için happy path + error path.

### Faz 3.0 Dalga 1 (Gateway worker)

7. **`Cargo.toml` `[workspace.dependencies]`** — `twilight-gateway` ekle:
   ```toml
   twilight-gateway = { version = "0.17", default-features = false, features = [
       "twilight-http", "rustls-platform-verifier", "zstd", "simd-json"
   ] }
   ```

8. **`crates/viscos-api/src/gateway.rs`** — twilight-gateway wrapper:
   ```rust
   use twilight_gateway::{Cluster, Config, Event};

   pub struct ViscosGateway {
       cluster: Cluster,
   }

   impl ViscosGateway {
       pub async fn connect(token: &str, intents: Intents) -> Result<Self, ApiError> { ... }
       pub async fn next_event(&mut self) -> Option<GatewayEvent> {
           Some(self.cluster.next().await?.try_into()?)
       }
   }
   ```

9. **`crates/viscos-api/src/events.rs`** — twilight `Event` → `viscos_core::GatewayEvent` adaptörü:
   ```rust
   pub enum GatewayEvent {
       Ready,
       MessageCreate(Message),
       GuildCreate(Guild),
       // ...
   }

   impl TryFrom<twilight_gateway::Event> for GatewayEvent { ... }
   ```

10. **Doğrulama (her iki faz)**:
    - `cargo build --workspace` → 0 hata.
    - `cargo test -p viscos-api` → integration test'ler geçer.
    - `cargo bloat --release -p viscos` → twilight-model katkısı +1-2 MB.
    - Faz 3.0'da: 24h soak test (Discord hesabı ile) → reconnect, resume, zstd-stream OK.

## Kabul kriterleri

- ✅ `viscos-api` crate workspace member'ları arasında.
- ✅ `twilight-model`, `twilight-http`, `twilight-gateway` workspace dependency'lerinde.
- ✅ `twilight-cache-inmemory`, `twilight-voice`, `twilight-standby`, `twilight-mention`, `twilight-util` **YOK** (grep ile doğrula).
- ✅ `ViscosError::Api` variant + `From<twilight_http::Error>` impl mevcut.
- ✅ `ViscosHttp` + `ViscosGateway` wrapper struct'ları mevcut.
- ✅ `GatewayEvent` enum twilight `Event`'inden adapt ediyor (`TryFrom`).
- ✅ Binary bütçesi 25 MB altında (`cargo bloat` ile doğrula).
- ✅ MSRV 1.89 (ADR-0006) ile uyumlu (`twilight-rs 0.17` MSRV'si ile).
- ✅ Faz 3.0'da zstd-stream framing kendi kodumuzda YOK (twilight hallediyor).

## Test stratejisi

- **Unit:**
  - `tests/events_try_from.rs`: Her twilight event tipi için `TryFrom` testi.
  - `tests/rest_error_mapping.rs`: 401, 429, 5xx → `ViscosError::Api` doğru variant.
- **Integration (Faz 2.0):**
  - Mock HTTP server (mockito/wiremock) ile login flow.
  - 25+ REST endpoint happy + error path.
- **Integration (Faz 3.0):**
  - Discord test hesabı (kullanıcı sağlar) ile gerçek bağlantı.
  - Session resume testi: bağlantıyı kopar → 30s içinde resume.
  - Reconnect backoff: ağ kesintisi 5dk → exponential backoff ile yeniden bağlan.
- **Manuel (Faz 3.0):**
  - 24h soak: mesaj dinle, restart 0 olmalı.
  - zstd-stream testi: büyük guild (>1000 üye) → `GUILD_CREATE` event'i parse.
  - `lsof` / Process Hacker → tek socket connection (shard başına).

## Sınır durumları ve riskler

- **MSRV drift:** `twilight-rs` 0.18 çıkarsa (Şubat 2026'da Aralık 2025'ten beri 0.17.x'te — minor bump). Mitigation: `cargo update` haftalık + AI PR review.
- **Discord protocol major versiyon:** Discord yeni bir protocol major versiyonu çıkarırsa (örn. v11). Mitigation: Twilight ekibi 24 saat içinde cevap veriyor (ADR-0008 Olumlu #1).
- **Twilight `DiscordHttpError` tipi:** `viscos-error::ViscosError::Api(ApiError)` adaptör katmanı zorunlu. Adapter eksikse hata yayılımı kırılır.
- **Bot-merkezli tipler:** Twilight'ın `Interaction` event'leri user client'ta kullanılmıyor ama Discord'tan geliyor. Mitigation: discard etmek yerine `GatewayEvent::Interaction` olarak tut, ileride user settings sync'te kullan.
- **+1-2 MB binary:** twilight-model tüm Discord objelerini typed struct olarak tutuyor. Mitigation: `lto = "fat"` (ADR-0005) dead-code eliminasyonu sonrası net etki.
- **+30-60s cold compile time:** twilight-model derive'ları. Mitigation: sccache (ADR-0004).
- **AI-yazım riski:** Sıfırlanır (ADR-0008 Olumlu #1). Custom zstd-stream yazma riski kalmadı.

## Review trigger'ları

- Twilight 1.0 major versiyon çıkarsa (API breaking).
- Discord yeni protocol major versiyonu (v11+).
- `twilight-cache-inmemory` bizim cache planımızdan (Faz 4) üstün olursa (olası değil).
- Binary bütçesi 25 MB aşılırsa (twilight'i kaldırıp custom'e dönmek en son çare).
- Discord rate-limit politikası değişirse (örn. global rate limit).
- Discord session resume davranışı değişirse (resume window 5dk → farklı).

## Cross-references

- **ADR:** ADR-0006 (MSRV 1.89), ADR-0007 (`ViscosError::Api` variant), ADR-0011 (token storage), ADR-0010 (cache stack — twilight-cache-inmemory yerine).
- **Plan:** [`phase-2.0-discord-api.md`](../../.cursor/plans/phase-2.0-discord-api.md) (Faz 2.0), [`phase-3.0-gateway.md`](../../.cursor/plans/phase-3.0-gateway.md) (Faz 3.0).
- **Araştırma:** [`viscos_index.md` Bölüm 6](../../.cursor/plans/viscos_index.md) (twilight low-level niyeti).
- **Alternatifler:** serenity, fastwebsockets, discord-user-rs, reqwest+tokio-tungstenite — hepsi elendi (ADR-0008 Consequences).
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Evet — bir kez (Faz 2.0 Dalga 1).** `viscos-api` crate'inin kurulumu, twilight sub-crate seçimi, `ViscosError::Api` adaptörü — tüm bunlar mimari karar gerektiren API sınırları. Faz 2.0'da ilk commit onaylandıktan sonra REST method eklemeleri (Faz 2.0 devam eden packet'ler) PR review'unda yakalanır. Faz 3.0 Gateway kurulumu da ayrı insan onayı gerektirir (sharding stratejisi + reconnect backoff parametreleri).
