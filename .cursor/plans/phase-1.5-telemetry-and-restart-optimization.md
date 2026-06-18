---
name: Phase 1.5 — Telemetry & Restart Optimization
overview: GDI watchdog'un Faz 1'de topladığı veriye dayalı telemetry backend, restart sıklığını raporlama, pre-restart draft autosave hardening, IPC audit tool, channel callback cleanup. **Mouse hover throttling kaldırıldı** (Microsoft kanıtladı: WebView2 DirectComposition bypass ediyor, JS throttle teknik olarak imkânsız). Yeniden adlandırıldı: önceki ad "Mouse Hover Throttling + IPC Audit" idi.
isProject: false
todos:
  - id: telemetry-backend
    content: viscos-telemetry crate (GDI time-series, restart count, mouse event counter, webview2 version)
    status: pending
  - id: telemetry-storage
    content: SQLite telemetry storage (rolling 30 gün, max 100MB)
    status: pending
  - id: ipc-audit-tool
    content: viscos-ipc-audit CLI tool (eval_script payload > 10KB tespit)
    status: pending
  - id: callback-cleanup
    content: Frontend wrapper: delete onmessage pattern (Vue onUnmounted)
    status: pending
  - id: draft-autosave-hardening
    content: Draft autosave'in SQLite entegrasyonu (Faz 2 öncesi stub)
    status: pending
  - id: tray-restart-badge
    content: Tray icon badge: "X restart today" (şeffaflık)
    status: pending
  - id: cef-decision-gate
    content: Telemetry verisine göre Faz 1.6 (Win11 CEF default) tetikleme kriteri
    status: pending
  - id: docs-update
    content: webview2-hardening.md Katman 2 throttle bölümünü "etkisiz kanıtlandı" notuyla güncelle
    status: pending
  - id: shadow-mode
    content: Discord fingerprint parite check (her ay GitHub Action, X-Super-Properties drift uyarısı) + ilk 24 saat shadow mode (sadece REST) — ADR-0012 §3
    status: pending
  - id: cve-feed
    content: Chromium security advisory feed (haftalık scrape, kritik CVE → CEF acil update) — ADR-0012 §CefUpdate
    status: pending
---

# Phase 1.5 — Telemetry & Restart Optimization

> **Süre:** 1 hafta
> **Hedef:** Watchdog'un Faz 1'de topladığı GDI verisini yapısal hale getirmek → MVP'nin GDI leak kabul kriterini (24 saatte <5 restart) ölçmek → gerekiyorsa Faz 1.6'da CEF'e geçişi tetiklemek.
> **Kritik referans:** [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md) ve [`webview2-hardening.md`](./webview2-hardening.md)
> **Önceki faz:** [`phase-1.0-window-webview.md`](./phase-1.0-window-webview.md)
> **Sonraki faz:** [`phase-1.6-cef-default-rollout.md`](./phase-1.6-cef-default-rollout.md) (koşullu, telemetry verisine göre)

---

## 1. Neden Mouse Throttle Kaldırıldı?

**Eski plan (v1):** JS seviyesinde `pointermove` rAF throttle → GDI leak'i azalt.

**Haziran 2026 bulgusu:** Microsoft Edge WebView2 feedback [#5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536) ve `tauri-apps/wry#1691` testleri kanıtladı:

| Workaround | Sonuç |
|-----------|-------|
| `SetIsVisible(false/true)` toggle | **Daha kötü** (6000 GDI/30s) |
| WRY parent subclass (`NotifyParentWindowPositionChanged` deferred) | Etkisiz |
| `WH_MOUSE` low-level hook | Etkisiz |
| Subclass WebView2 child windows (`WM_MOUSEMOVE` throttle) | Etkisiz |
| `--disable-gpu-compositing` browser arg | Etkisiz |
| `--disable-features=RemoveRedirectionBitmap` | Etkisiz |
| CSS `:hover` kaldır | Etkisiz |
| CSS `pointer-events: none` | Etkisiz |
| JS `requestAnimationFrame` throttle | **Etkisiz** |
| 0 JS plain `<p>` HTML | **Hâlâ leak ediyor** |

**Neden?** WebView2 mouse input'u **DirectComposition üzerinden** alıyor, Win32 message queue'yu bypass ediyor. JS katmanında throttle etmek, browser process'e ulaşan mouse event'lerini durdurmuyor.

**Bu yüzden Faz 1.5 yeniden tanımlandı:**
- ❌ Mouse throttle → **kaldırıldı**
- ✅ Telemetry → restart sayısını ölç
- ✅ Restart gap minimizasyonu → pre-restart draft autosave
- ✅ IPC audit + channel cleanup → diğer upstream bug'ları önle

**Viscos hedefi:** Microsoft bug'ını **çözmek değil**, **görünür kılmak ve gerektiğinde CEF'e kaçmak**.

---

## 2. Telemetry Backend (`viscos-telemetry`)

### 2.1 Amaç

MVP kabul kriteri: 24 saatlik soak test'te **< 5 restart** + **gap < 2s** + **draft kaybı 0**. Telemetry bunu ölçer ve Faz 1.6 tetikleme kararını veriye dayandırır.

### 2.2 Workspace

`Cargo.toml`'a ekle:

```toml
[workspace]
members = [
    # ... mevcut
    "crates/viscos-telemetry",  # YENİ
]

[workspace.dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }  # SQLite telemetry storage
```

### 2.3 Veri Modeli

```sql
-- crates/viscos-telemetry/migrations/001_init.sql

CREATE TABLE gdi_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp_ms INTEGER NOT NULL,        -- Unix epoch ms
    gdi_count INTEGER NOT NULL,
    delta_per_sec REAL NOT NULL,           -- gdi artış hızı (saniye başına)
    threshold_warning INTEGER NOT NULL,    -- o anki config
    threshold_critical INTEGER NOT NULL,
    is_warning INTEGER NOT NULL,           -- 0/1
    is_critical INTEGER NOT NULL,          -- 0/1
    webview2_version TEXT NOT NULL,        -- örn. "146.0.3856.59"
    os_build INTEGER NOT NULL              -- örn. 26200 (Win11)
);

CREATE INDEX idx_gdi_timestamp ON gdi_samples(timestamp_ms);

CREATE TABLE restart_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp_ms INTEGER NOT NULL,
    trigger_kind TEXT NOT NULL,            -- 'gdi_critical' | 'ipc_critical' | 'manual' | 'app_update'
    gdi_at_trigger INTEGER,                -- tetik anındaki GDI sayısı
    gap_ms INTEGER NOT NULL,               -- recreate süresi (ms)
    drafts_saved INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL               -- 0/1
);

CREATE INDEX idx_restart_timestamp ON restart_events(timestamp_ms);

CREATE TABLE watchdog_config_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp_ms INTEGER NOT NULL,
    config_json TEXT NOT NULL,             -- {"gdi_warning":7000,"gdi_critical":9000,...}
    changed_by TEXT NOT NULL               -- 'user' | 'auto' | 'update'
);

CREATE TABLE session_summary (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_start_ms INTEGER NOT NULL,
    session_end_ms INTEGER,
    total_restarts INTEGER NOT NULL DEFAULT 0,
    peak_gdi INTEGER NOT NULL DEFAULT 0,
    avg_gdi REAL,
    drafts_saved_total INTEGER NOT NULL DEFAULT 0
);
```

### 2.4 API

```rust
// crates/viscos-telemetry/src/lib.rs

use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::Arc;

pub struct TelemetryConfig {
    pub db_path: PathBuf,                  // %APPDATA%/Viscos/telemetry.db
    pub retention_days: u32,               // default: 30
    pub max_size_mb: u32,                  // default: 100
    pub enabled: bool,                     // default: true (user opt-out)
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            db_path: dirs::data_dir()
                .unwrap_or_default()
                .join("Viscos")
                .join("telemetry.db"),
            retention_days: 30,
            max_size_mb: 100,
            enabled: true,
        }
    }
}

pub struct Telemetry {
    db: Arc<tokio::sync::Mutex<Connection>>,
    config: TelemetryConfig,
}

#[derive(Debug, Clone, Copy)]
pub struct GdiSample {
    pub gdi: u32,
    pub delta_per_sec: f64,
    pub is_warning: bool,
    pub is_critical: bool,
    pub threshold_warning: u32,
    pub threshold_critical: u32,
}

impl Telemetry {
    pub fn new(config: TelemetryConfig) -> anyhow::Result<Self> {
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(&config.db_path)?;
        db.execute_batch(include_str!("../migrations/001_init.sql"))?;
        Ok(Self { db: Arc::new(tokio::sync::Mutex::new(db)), config })
    }

    pub async fn record_gdi_sample(&self, sample: GdiSample) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        let (os_build, webview2_version) = platform_info();
        db.execute(
            "INSERT INTO gdi_samples
             (timestamp_ms, gdi_count, delta_per_sec, threshold_warning,
              threshold_critical, is_warning, is_critical,
              webview2_version, os_build)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                chrono::Utc::now().timestamp_millis(),
                sample.gdi,
                sample.delta_per_sec,
                sample.threshold_warning,
                sample.threshold_critical,
                sample.is_warning as i32,
                sample.is_critical as i32,
                webview2_version,
                os_build,
            ],
        )?;
        Ok(())
    }

    pub async fn record_restart(&self, event: RestartEvent) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO restart_events
             (timestamp_ms, trigger_kind, gdi_at_trigger, gap_ms, drafts_saved, success)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                chrono::Utc::now().timestamp_millis(),
                event.trigger_kind,
                event.gdi_at_trigger,
                event.gap_ms,
                event.drafts_saved,
                event.success as i32,
            ],
        )?;
        Ok(())
    }

    /// Son 24 saatteki restart sayısı (Faz 1.6 tetikleme kriteri)
    pub async fn restarts_last_24h(&self) -> anyhow::Result<u32> {
        let db = self.db.lock().await;
        let cutoff = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
        let count: u32 = db.query_row(
            "SELECT COUNT(*) FROM restart_events
             WHERE timestamp_ms > ? AND success = 1",
            params![cutoff],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Son 7 gündeki GDI peak değeri
    pub async fn peak_gdi_last_7d(&self) -> anyhow::Result<u32> {
        let db = self.db.lock().await;
        let cutoff = chrono::Utc::now().timestamp_millis() - 7 * 24 * 60 * 60 * 1000;
        let peak: u32 = db.query_row(
            "SELECT COALESCE(MAX(gdi_count), 0) FROM gdi_samples WHERE timestamp_ms > ?",
            params![cutoff],
            |row| row.get(0),
        )?;
        Ok(peak)
    }

    /// Rolling retention: 30 günden eski kayıtları sil
    pub async fn vacuum(&self) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        let cutoff = chrono::Utc::now().timestamp_millis()
            - (self.config.retention_days as i64) * 24 * 60 * 60 * 1000;
        db.execute("DELETE FROM gdi_samples WHERE timestamp_ms < ?", params![cutoff])?;
        db.execute("DELETE FROM restart_events WHERE timestamp_ms < ?", params![cutoff])?;
        db.execute("VACUUM", [])?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RestartEvent {
    pub trigger_kind: &'static str,         // 'gdi_critical' | 'ipc_critical' | 'manual'
    pub gdi_at_trigger: Option<u32>,
    pub gap_ms: u64,
    pub drafts_saved: u32,
    pub success: bool,
}

#[cfg(windows)]
fn platform_info() -> (u32, String) {
    use windows::Win32::System::SystemInformation::*;
    let os_build = unsafe {
        let osvi = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        // Win11 build >= 22000 → GDI leak riski
        osvi.dwBuildNumber
    };
    // webview2 version registry'den
    let webview2_version = read_webview2_version().unwrap_or_else(|| "unknown".to_string());
    (os_build, webview2_version)
}

#[cfg(not(windows))]
fn platform_info() -> (u32, String) { (0, "n/a".to_string()) }
```

### 2.5 Watchdog Entegrasyonu

```rust
// crates/viscos-watchdog/src/lib.rs (değişiklik)

pub struct Watchdog {
    config: WatchdogConfig,
    ipc: Arc<IpcBridge>,
    webview_handle: Arc<dyn WebViewHandle>,
    draft_autosave: Arc<dyn DraftAutosave>,
    telemetry: Arc<Telemetry>,           // YENİ
    restart_start: Option<Instant>,      // YENİ: gap ölçümü
}

impl Watchdog {
    // ... önceki kod

    loop {
        ticker.tick().await;
        let gdi = platform::gdi_count();
        let delta = gdi.saturating_sub(last_gdi);
        last_gdi = gdi;

        let sample = GdiSample {
            gdi,
            delta_per_sec: delta as f64 / self.config.poll_interval.as_secs_f64(),
            is_warning: gdi >= self.config.gdi_warning,
            is_critical: gdi >= self.config.gdi_critical,
            threshold_warning: self.config.gdi_warning,
            threshold_critical: self.config.gdi_critical,
        };
        let _ = self.telemetry.record_gdi_sample(sample).await;

        if gdi >= self.config.gdi_critical {
            self.restart_start = Some(Instant::now());

            let drafts_saved = self.draft_autosave.snapshot_open_composers().unwrap_or(0);
            let success = self.webview_handle.dispose_and_recreate().is_ok();
            let gap_ms = self.restart_start.unwrap().elapsed().as_millis() as u64;

            self.telemetry.record_restart(RestartEvent {
                trigger_kind: "gdi_critical",
                gdi_at_trigger: Some(gdi),
                gap_ms,
                drafts_saved: drafts_saved as u32,
                success,
            }).await.ok();

            // Faz 1.6 tetikleme kriteri kontrolü (her restart'ta değil, günde 1 kez)
            if self.telemetry.restarts_last_24h().await.unwrap_or(0) >= 5 {
                self.ipc.emit(IpcEvent::WatchdogAlert {
                    kind: WatchdogKind::GdiLeakCritical,
                    message: "≥5 restart/24h: Faz 1.6 (CEF default) önerisi".to_string(),
                });
            }

            last_gdi = 0;
        }
        // ... warning, IPC buffer aynı
    }
}
```

---

## 3. Tray Icon Badge (Şeffaflık)

Kullanıcı watchdog restart'larından haberdar olmalı. Tray icon'da badge:

```rust
// crates/viscos-shell/src/tray.rs (ekleme)

use tao::tray::TrayIconBuilder;

pub async fn update_tray_badge(
    tray: &tao::tray::TrayIcon,
    telemetry: &Telemetry,
) -> anyhow::Result<()> {
    let restarts = telemetry.restarts_last_24h().await?;
    let tooltip = if restarts > 0 {
        format!("Viscos ({} restart today)", restarts)
    } else {
        "Viscos".to_string()
    };
    tray.set_tooltip(Some(&tooltip))?;
    Ok(())
}
```

**Tooltip format:** `Viscos (3 restart today)` — restart yoksa sadece `Viscos`.

> Trade-off: Şeffaflık vs alarm yorgunluğu. Kullanıcı her restart'ı görüyor → "neden?" sorusunu soruyor → docs'a yönlendiriliyor.

---

## 4. Pull-Based IPC Audit Tool (KORUNDU)

Önceki plandaki `viscos-ipc-audit` CLI tool Faz 1.5'te **korunuyor**. İçerik değişmiyor, sadece dosya adı bu faz dosyasında kaldı.

> **Referans:** Bu tool'un tam implementasyonu orijinal `phase-1.5-mouse-throttling.md`'de vardı. Davranış: Tüm `eval_script` call'larını enumerate eder, payload >10KB → CI fail.

```rust
// crates/viscos-ipc-audit/src/main.rs (KORUNDU)
// Değişiklik yok. CI entegrasyonu:
```

```yaml
# .github/workflows/ci.yml
- name: IPC audit
  run: cargo run -p viscos-ipc-audit --release
```

**Davranış:** Flagged > 0 ise CI fail.

---

## 5. Channel Callback Cleanup Pattern (KORUNDU)

Önceki plandaki `delete onmessage` pattern Faz 1.5'te **korunuyor**. Bu upstream bug (`tauri-apps/tauri#13133`) hâlâ açık ve WebView2'yi değil Tauri IPC channel'ı etkiliyor; Viscos'ta `tauri` kullanmasak da `MessageChannel` pattern'i aynı.

```typescript
// frontend/src/cleanup.ts (KORUNDU)

export function useIpcChannel<T>(channelName: string, handler: (msg: T) => void) {
  const channel = new MessageChannel();
  channel.port1.onmessage = (e) => handler(e.data);

  window.viscos.invoke({ type: 'SubscribeChannel', data: { name: channelName } });

  // KRİTİK: tauri#13133 — cleanup
  onUnmounted(() => {
    channel.port1.onmessage = null;
    channel.port1.close();
    channel.port2.close();
    delete (channel.port1 as any).onmessage;
  });

  return channel;
}
```

**Test:** 1000 channel aç-kapa → heap snapshot, referans sayımı sabit.

---

## 6. Draft Autosave Hardening (Faz 2 Öncesi Stub)

Faz 1'de watchdog pre-restart'ta `DraftAutosave::snapshot_open_composers()` çağırıyor. Faz 1'de bu in-memory stub; Faz 1.5'te SQLite (Faz 2 öncesi) stub:

```rust
// crates/viscos-shell/src/draft_autosave.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub channel_id: String,
    pub content: String,
    pub last_edit_ms: i64,
}

pub struct SqliteDraftAutosave {
    drafts: Arc<RwLock<HashMap<String, Draft>>>,
}

impl SqliteDraftAutosave {
    pub fn new() -> Self {
        Self { drafts: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn set(&self, channel_id: String, content: String) {
        let mut drafts = self.drafts.write().await;
        drafts.insert(channel_id.clone(), Draft {
            channel_id,
            content,
            last_edit_ms: chrono::Utc::now().timestamp_millis(),
        });
    }

    pub async fn get(&self, channel_id: &str) -> Option<Draft> {
        self.drafts.read().await.get(channel_id).cloned()
    }
}

impl DraftAutosave for SqliteDraftAutosave {
    fn snapshot_open_composers(&self) -> anyhow::Result<usize> {
        let drafts = self.drafts.blocking_read();
        // Watchdog pre-restart hook'tan çağrılır (async context değil)
        // Telemetry'ye "X draft saved" raporla
        Ok(drafts.len())
    }
}
```

**Faz 2'de:** `viscos-cache` (SQLite) ile tam entegrasyon, draft'lar `drafts` tablosuna yazılır, restart sonrası `bridge.ts` otomatik restore eder.

---

## 7. Faz 1.6 Tetikleme Kriteri (CEF Default)

Faz 1.6 Win11 default CEF'e geçiş **koşullu**. Telemetry verisi karar verir:

```rust
// crates/viscos-shell/src/cef_decision.rs

use viscos_telemetry::Telemetry;

pub enum CefRolloutRecommendation {
    /// Restart sıklığı kabul edilebilir, WebView2 kalabilir
    StayWebView2,
    /// Restart fazla, CEF default'a geçiş öner
    RecommendCefDefault,
    /// Restart kabul edilemez (ör. 24 saatte >10), agresif aksiyon gerek
    ForceCefDefault,
}

pub async fn evaluate_cef_rollout(
    telemetry: &Telemetry,
) -> anyhow::Result<CefRolloutRecommendation> {
    let restarts_24h = telemetry.restarts_last_24h().await?;
    let peak_gdi_7d = telemetry.peak_gdi_last_7d().await?;

    // Faz 1.5 kabul kriteri
    if restarts_24h >= 10 {
        Ok(CefRolloutRecommendation::ForceCefDefault)
    } else if restarts_24h >= 5 || peak_gdi_7d >= 8500 {
        Ok(CefRolloutRecommendation::RecommendCefDefault)
    } else {
        Ok(CefRolloutRecommendation::StayWebView2)
    }
}
```

**Davranış:**
- `StayWebView2`: Tray icon badge normal, "Viscos" tooltip.
- `RecommendCefDefault`: Tray icon badge + notification: "GDI leak yaşıyor olabilirsiniz. CEF backend denemek ister misiniz?"
- `ForceCefDefault`: Sonraki açılışta backend seçim wizard'ı otomatik gösterilir (Faz 8.5'teki `BackendWizard`'ın lightweight versiyonu).

---

## 7.5 Anti-Bot Heuristic Parite (ADR-0012 §3 — YENİ)

Discord mühendisleri üçüncü parti client'ları "deliberately banlamıyoruz ama heuristics ile yakalayabiliyoruz" diye beyan etti (HN, 2022-2024). İki ek savunma katmanı:

### A) Discord Client Fingerprint Parite

Viscos'un X-Super-Properties header'ı Discord'un "modified client" heuristic'ini tetiklememeli. Bunu sağlamak için:

**1. `client_build_number` haftalık GitHub Action sync** (Faz 2.0 plan'ında var):
- Discord web client JS bundle'ından `release_channel` ve `build_number` parse.
- Eski değer → PR otomatik açılır, insan review.

**2. WebGL hash backend parity check (YENİ — Faz 1.5'te telemetry backend ile entegre):**

```rust
// crates/viscos-telemetry/src/fingerprint_parity.rs

pub struct FingerprintParity {
    pub backend: BackendKind,                    // WebView2 | Cef
    pub webgl_hash: String,                      // renderer'dan alınan
    pub user_agent: String,
    pub client_build_number: u32,
    pub os_build: u32,
    pub webview_version: String,                 // Edge | CEF Chromium
}

impl FingerprintParity {
    pub fn drift_check(&self, reference: &FingerprintParity) -> ParityResult {
        // Drift > %5 → uyarı PR'ı
        // build_number gap > 14 gün → uyarı
        // OS build mismatch → OK (kullanıcı OS güncelleyebilir)
        // ...
    }
}
```

**3. Aylık GitHub Action (her ay 1, 5 dakika):**
- Self-hosted runner'da Viscos'u Windows + CEF + Windows + WebView2 ile başlat.
- `crates/viscos-telemetry/src/bin/parity_check.rs` → fingerprint'i JSON dump.
- Aynı tarihli Discord stable client fingerprint'i ile karşılaştır (Discord stable CDN'den çek).
- Drift > %5 → uyarı issue aç.

### B) İlk 24 Saat Shadow Mode

Yeni login olduğunda ilk 24 saat **sadece REST** kullan:

```rust
// crates/viscos-auth/src/shadow_mode.rs (YENİ)

pub struct ShadowMode {
    enabled: bool,                               // default: true
    login_at: SystemTime,
    duration: Duration,                          // default: 24h
}

impl ShadowMode {
    pub fn can_write(&self) -> bool {
        !self.enabled || self.login_at.elapsed().unwrap_or_default() >= self.duration
    }
    
    pub fn opt_out(&mut self) { self.enabled = false; }
}

// IPC integration:
// IpcCommand::SendMessage { .. } → shadow_mode.can_write() check
//   false → return error "Shadow mode: X saat kaldı"
//   true → forward to twilight-http
```

**Kullanıcı UX:**
- Login sonrası modal: "Hesabınız yeni, ilk 24 saat Discord'un davranış analizi için bekleme süresi. Mesaj gönderebilirsiniz ama bazı yazma işlemleri ısınma sonrası aktif olur."
- Ayarlar → Gelişmiş → "Shadow mode atla" (agresif kullanıcılar için, ToS riski kendilerine ait).

**Gerekçe:** kind'ın geliştiricisi benzer bir "warmup period" uyguluyor (HN beyanı, 2025). Discord'un anti-spam heuristic'i yeni token + yeni fingerprint kombinasyonunu "olası self-bot" olarak işaretleyebilir; 24 saat read-only trafik bu riski azaltır.

---

## 7.6 CEF Self-Update Feed (ADR-0012 §CefUpdate — YENİ)

Faz 8.5'te planlanan CEF self-update "ayda bir kontrol" şeklinde. **Haziran 2026 eki:** Chromium security advisory feed haftalık scrape + kritik CVE çıkarsa acil update.

```rust
// crates/viscos-update/src/chromium_feed.rs (Faz 1.5 — telemetry backend hazır olduğunda skeleton)

pub struct ChromiumAdvisoryFeed {
    feed_url: &'static str,                     // https://chromereleases.googleblog.com/feeds/posts/default
    last_check: Mutex<Option<SystemTime>>,
    critical_cves: Mutex<Vec<CveAlert>>,
}

impl ChromiumAdvisoryFeed {
    pub async fn check(&self) -> anyhow::Result<Vec<CveAlert>> {
        // Google Chrome Releases blog'u scrape (RSS/Atom)
        // Son 7 gündeki "Stable channel update" + "Security" tag'li post'ları al
        // CVE ID'leri parse et (CVE-YYYY-NNNNN format)
        // Severity > High ise critical listesine ekle
    }
    
    pub async fn should_force_update(&self) -> bool {
        // Son 7 günde >= 1 Critical CVE → force update
        // High CVE'ler haftalık routine update'te yapılır
    }
}
```

**Faz 1.5'te:** Skeleton implementasyon + unit test (mock feed). Gerçek feed scrape Faz 8.5'te.

---

## 8. Test Stratejisi (Faz 1.5)

| Test | Tip | Kabul |
|------|-----|-------|
| Telemetry DB oluşturma | Unit (mock path) | Schema doğru |
| GDI sample insert/query | Integration | Round-trip OK |
| 24h restart count query | Integration | Doğru sayım |
| Peak GDI query | Integration | Doğru max |
| Vacuum (eski kayıt sil) | Integration | 30 günden eski yok |
| Audit tool temiz kod | CI | Flagged = 0 |
| Audit tool büyük payload | CI | Flagged > 0 → fail |
| Channel cleanup 1000 iter | Lokal | Heap sabit |
| Draft autosave snapshot | Unit | N=count doğru |
| CefRolloutRecommendation | Unit (mock telemetry) | Threshold doğru |
| Fingerprint parity drift | Unit (mock feed) | >%5 drift → uyarı |
| Shadow mode 24h gate | Unit | Yeni login'de write block, 24h sonra OK |
| Shadow mode opt-out | Unit | Ayar değişikliği etkinleşir |
| Chromium feed parse | Unit (mock RSS) | CVE-YYYY-NNNNN extract |

---

## 9. Kabul Kriterleri (Definition of Done)

- [ ] `viscos-telemetry` crate oluşturuldu, DB schema migrate edildi
- [ ] Watchdog her 30s'de GDI sample telemetry'ye yazıyor
- [ ] Restart event'leri telemetry'ye yazılıyor (gap_ms, drafts_saved, success)
- [ ] Tray icon tooltip "X restart today" badge'i gösteriyor
- [ ] `viscos-ipc-audit` CI'da çalışıyor, temiz codebase'de 0 flag
- [ ] Channel cleanup wrapper: 1000 iter test'te leak yok
- [ ] Draft autosave SQLite stub çalışıyor (Faz 2 öncesi yeterli)
- [ ] `evaluate_cef_rollout` doğru öneri veriyor (mock telemetry ile)
- [ ] 24 saatlik lokal soak test'te **<5 restart** + **gap <2s** + **draft kaybı 0**
- [ ] `cargo clippy -- -D warnings` temiz
- [ ] `cargo test` tüm geçer
- [ ] `webview2-hardening.md` Katman 2 throttle bölümü "etkisiz kanıtlandı" notuyla güncel

---

## 10. Karar Noktası (Faz 1.5 Sonu)

> 🔵 **İNSAN:** Telemetry retention süresi? (Önerilen: 30 gün)
> - 7 gün: düşük disk, kısa trend analizi
> - 30 gün (önerilen): aylık regression tespiti için yeterli
> - 90 gün: yıllık bazeline karşılaştırma

> 🔵 **İNSAN:** Tray badge formatı?
> - "3 restart today" (önerilen): şeffaf, alarm değil
> - Sadece icon (tooltip'te): daha az göze batıyor
> - Bildirim popup: alarm yorgunluğu riski

> 🔵 **İNSAN:** Faz 1.6 tetikleme kriteri? (Önerilen: 5 restart/24h OR peak GDI ≥8500)
> - Agresif: 3 restart → CEF (kullanıcıyı koru)
> - Orta (önerilen): 5 restart → CEF
> - Gevşek: 10 restart → CEF (çoğu kullanıcı WebView2'de kalır)

> 🔵 **İNSAN:** Telemetry opt-in mi opt-out mu?
> - Opt-out (varsayılan açık, önerilen): MVP'de feature gate olarak çalışır
> - Opt-in: GDPR/privacy daha temiz ama telemetry verisi az olur
> - Not: Viscos GPL-3.0 + lokal-only telemetry, network yok, opt-out yeterli

---

## 11. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| Telemetry DB şişmesi | Disk | 30 gün retention + 100MB cap + vacuum |
| Telemetry disk I/O | Performance | Sample insert async, batch write her 5dk |
| Opt-out kullanıcı | Veri yok | Default opt-out olsa bile restart davranışı aynı |
| Tray badge spam | UX | Günde 1 notification, diğerleri sadece tooltip |
| CEF rollout false positive | Gereksiz binary | Telemetry 7 günlük pencere, trend analizi |
| Draft autosave race condition | Mesaj kaybı | RwLock + atomic snapshot, no I/O during snapshot |

---

## 12. Çıkış → Faz 1.6 (Koşullu) veya Faz 2.0

Bu faz tamamlandığında:
- Telemetry backend canlı, restart trendleri raporlanıyor
- Pre-restart draft autosave stub çalışıyor (Faz 2'de SQLite tam entegrasyon)
- IPC audit + channel cleanup korunuyor
- Mouse throttle **kanıtlanmış etkisiz** olarak belgelendi
- **Discord fingerprint parite check** skeleton + aylık GitHub Action (ADR-0012 §3.A)
- **İlk 24 saat shadow mode** + kullanıcı opt-out (ADR-0012 §3.B)
- **Chromium advisory feed** skeleton (Faz 8.5'te gerçek feed scrape)

**Faz 1.6 (koşullu):**
- Eğer telemetry 24 saatte ≥5 restart gösteriyorsa → Win11 default CEF backend
- Win10 default WebView2 kalır (leak riski düşük, daha hafif binary)
- Tray badge önerisi + opt-out wizard

**Faz 2.0 (normal akış):** Discord REST API + Auth (token storage, login flow, MFA).
