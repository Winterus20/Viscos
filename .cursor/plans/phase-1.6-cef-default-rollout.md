---
name: Phase 1.6 — CEF Default Rollout (Win11)
overview: Faz 1.5 telemetry verisine göre Win11 kullanıcılarına CEF backend default olarak sunulur. Win10 default WebView2 kalır. Microsoft Edge WebView2 GDI leak (#5536 — Haziran 2026 STATE: OPEN) yapısal olarak CEF'e geçişle çözülür. **Öne alındı**: Faz 8.5'ten buraya, MVP'nin parçası olarak.
isProject: false
todos:
  - id: cef-deps
    content: cef-rs Cargo dependency (Faz 1'deki trait ile)
    status: pending
  - id: cef-backend-impl
    content: CefWebViewBackend implementasyonu (WebViewBackend trait)
    status: pending
  - id: backend-detection
    content: Windows version detection (Win10 vs Win11 build 22000+)
    status: pending
  - id: cef-default-config
    content: Config-driven default: Win11 → CEF, Win10 → WebView2
    status: pending
  - id: cef-bundle-build
    content: CEF bundle build target (--features cef-backend)
    status: pending
  - id: msi-webview2
    content: MSI: viscos-webview2.msi (15-25MB, Win10 + opt-in için)
    status: pending
  - id: msi-cef
    content: MSI: viscos-cef.msi (220-300MB, Win11 default)
    status: pending
  - id: ci-dual-pipeline
    content: CI'da iki backend için ayrı test pipeline
    status: pending
  - id: tray-cef-suggestion
    content: Tray icon "GDI leak detected, CEF backend deneyin" bildirimi (telemetry-driven)
    status: pending
  - id: backend-switch-ui
    content: Ayarlar'da backend değiştirme UI (iced, Faz 5 öncesi minimal)
    status: pending
  - id: rdp-auto-detect
    content: RDP session auto-detect (GetSystemMetrics(SM_REMOTESESSION) → CEF zorla) — ADR-0012 §6
    status: pending
  - id: bridge-resilience-ref
    content: Frontend bridge.ts ADR-0012 §2 selector resilience kuralları referansı
    status: pending
---

# Phase 1.6 — CEF Default Rollout (Win11)

> **Süre:** 1–2 hafta
> **Hedef:** Microsoft WebView2 GDI leak'ine karşı **yapısal çözüm**: Win11 kullanıcılarına CEF backend default, Win10'a WebView2 default. Faz 1.5 telemetry verisi kararı tetikler.
> **Kritik referans:** [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md) Bölüm 3.7, [`webview2-hardening.md`](./webview2-hardening.md) Bölüm 4 Katman 3
> **Önceki faz:** [`phase-1.5-telemetry-and-restart-optimization.md`](./phase-1.5-telemetry-and-restart-optimization.md)
> **Sonraki faz:** [`phase-2.0-discord-api.md`](./phase-2.0-discord-api.md)

---

## 1. Neden Bu Faz Var?

**Sorun:** Microsoft Edge WebView2 [issue #5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536) Haziran 2026'da hâlâ açık. Win11 + WebView2 146+ üzerinde mouse hover ile ~4000 GDI obje / 30sn leak. Faz 1'deki watchdog bu leak'i maskeliyor (soft restart), Faz 1.5 telemetry restart sıklığını ölçüyor. **24 saatte ≥5 restart** = kabul edilemez UX.

**Çözüm:** Leak'i **çözmek yerine**, leak'ten **kaçınmak**. CEF (Chromium Embedded Framework) kendi runtime'ını kullanır, Win32 GDI'ya bağımlı değildir → leak yok.

**MVP'de neden zorunlu?** Viscos hedef kitlesi = Win11 kullanıcılarının çoğunluğu. Bu kullanıcıların %100'ü bug'a açık. Restart spam yapan bir Discord client "MVP" değil.

**Zamanlama:** Faz 1.5 telemetry verisi olmadan karar veremeyiz (kaç kullanıcı etkileniyor bilmiyoruz). Bu yüzden Faz 1.5'ten **hemen sonra**, Faz 2'den **önce**. Faz 8.5'e bırakmak MVP'yi riske atar.

---

## 2. Mimari Karar: WebViewBackend Trait (Zaten Var)

Faz 1'de `WebViewBackend` trait tanımlandı (`crates/viscos-webview/src/backend.rs`). Bu trait abstraction'ı sayesinde Faz 1.6'da sadece `CefWebViewBackend` implementasyonu ekliyoruz — geri kalan kod (watchdog, IPC, frontend bridge) değişmiyor.

```rust
// crates/viscos-webview/src/backend.rs (Faz 1'den, korunuyor)

pub trait WebViewBackend: Send + Sync {
    fn create(&self, window: &tao::window::Window, config: &WebViewConfig,
              ipc: std::sync::Arc<viscos_ipc::IpcBridge>) -> anyhow::Result<Box<dyn WebViewHandle>>;
    fn version(&self) -> &'static str;
    fn known_issues(&self) -> &[&'static str];
    fn post_shared_buffer(&self, _bytes: &[u8], _metadata: &str) -> anyhow::Result<()> {
        anyhow::bail!("post_shared_buffer Faz 4'te implemente edilecek")
    }
}

pub enum BackendKind {
    WebView2,
    Cef,
}

impl Default for BackendKind {
    fn default() -> Self {
        // Faz 1.6: BackendKind::default() hâlâ WebView2.
        // Gerçek default selection `select_default_backend()` ile yapılır (Bölüm 3).
        BackendKind::WebView2
    }
}
```

---

## 3. Backend Selection Logic

### 3.1 Platform Detection

```rust
// crates/viscos-shell/src/backend_selection.rs

use viscos_webview::{BackendKind, WebViewBackend};
use viscos_telemetry::Telemetry;

pub fn select_default_backend(telemetry: Option<&Telemetry>) -> BackendChoice {
    let platform_default = platform_recommended_default();
    let telemetry_override = telemetry.map(evaluate_cef_recommendation);

    match (platform_default, telemetry_override) {
        (BackendChoice::Cef, _) => BackendChoice::Cef,
        (BackendChoice::WebView2, Some(CefRolloutRecommendation::ForceCefDefault)) => {
            tracing::warn!("Telemetry: ≥10 restart/24h, CEF default forced");
            BackendChoice::Cef
        }
        (BackendChoice::WebView2, Some(CefRolloutRecommendation::RecommendCefDefault)) => {
            tracing::info!("Telemetry: ≥5 restart/24h OR peak GDI ≥8500, CEF recommended");
            // Kullanıcıya wizard göster, default'u değiştirme (öneri)
            BackendChoice::WebView2
        }
        (BackendChoice::WebView2, _) => BackendChoice::WebView2,
    }
}

#[cfg(windows)]
fn platform_recommended_default() -> BackendChoice {
    use windows::Win32::System::SystemInformation::*;
    let os_build = unsafe {
        let osvi = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        osvi.dwBuildNumber
    };

    if os_build >= 22000 {
        // Win11: GDI leak riski yüksek
        BackendChoice::Cef
    } else {
        // Win10: WebView2 sorunsuz, daha hafif binary
        BackendChoice::WebView2
    }
}

#[cfg(not(windows))]
fn platform_recommended_default() -> BackendChoice {
    // v1 sadece Windows, fallback
    BackendChoice::WebView2
}
```

### 3.2 Config Override

Kullanıcı `config.toml`'da override edebilir:

```toml
# %APPDATA%/Viscos/config.toml

[backend]
kind = "cef"  # "webview2" | "cef"

[backend.cef]
cache_dir = "%APPDATA%/Viscos/cef-cache"
disable_gpu = false
chromium_flags = ["--disable-features=Translate"]

[backend.webview2]
user_data_dir = "%APPDATA%/Viscos/webview2-cache"
disable_gpu = false
```

**Öncelik:** `config.toml kind` > telemetry force > platform default.

### 3.3 RDP Session Auto-Detect (ADR-0012 §6 — YENİ)

RDP üzerinden Discord kullanan IT/admin kullanıcıları için **zorla CEF default** — Microsoft WebView2 RDP'de GDI region leak yapıyor (WV2#5266, düzeltilmedi):

```rust
// crates/viscos-shell/src/backend_selection.rs (ekleme)

#[cfg(windows)]
fn is_rdp_session() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics;
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}

#[cfg(not(windows))]
fn is_rdp_session() -> bool { false }

pub fn select_default_backend(telemetry: Option<&Telemetry>) -> BackendChoice {
    let platform_default = platform_recommended_default();
    
    // ADR-0012 §6: RDP session → CEF zorla (WebView2 RDP'de leak)
    if is_rdp_session() {
        tracing::info!("RDP session detected, forcing CEF backend");
        return BackendChoice::Cef;
    }
    
    let telemetry_override = telemetry.map(evaluate_cef_recommendation);
    match (platform_default, telemetry_override) {
        // ... mevcut match
    }
}
```

**UX:**
- RDP tespit edildiğinde tray notification: "RDP oturumu algılandı, CEF backend kullanılıyor (WebView2 RDP'de GDI leak yapıyor)."
- Kullanıcı manuel override edebilir (config.toml `backend.kind = "webview2"`).

**Referans:** Microsoft WebView2 RDP issue (#5266), düzeltilmedi. Detay: [`docs/CEF-VS-WEBVIEW2.md` RDP bölümü](../../CEF-VS-WEBVIEW2.md).

### 3.4 BackendFactory

```rust
// crates/viscos-shell/src/backend_factory.rs

use viscos_webview::{BackendKind, WebViewBackend, WebViewConfig};
use viscos_webview::webview2::WryWebView2Backend;
#[cfg(feature = "cef-backend")]
use viscos_webview::cef::CefWebViewBackend;
use tao::window::Window;
use std::sync::Arc;
use viscos_ipc::IpcBridge;

pub fn create_backend(
    kind: BackendKind,
) -> Box<dyn WebViewBackend> {
    match kind {
        BackendKind::WebView2 => Box::new(WryWebView2Backend::new()),
        #[cfg(feature = "cef-backend")]
        BackendKind::Cef => Box::new(CefWebViewBackend::new()),
        #[cfg(not(feature = "cef-backend"))]
        BackendKind::Cef => {
            tracing::error!("CEF backend not compiled, falling back to WebView2");
            Box::new(WryWebView2Backend::new())
        }
    }
}
```

---

## 4. CEF Backend Implementation

### 4.1 Cargo Workspace

```toml
# Cargo.toml workspace
[workspace.dependencies]
# Faz 1'den
wry = "0.55"

# Faz 1.6 — YENİ
# cef-rs Tauri ekibinden, BSD-2-Clause, GPL-3.0 uyumlu
cef-rs = { git = "https://github.com/tauri-apps/cef-rs", tag = "cef-v148.3.0+148.0.9", optional = true }
# Latest: cef-v148.3.0+148.0.9 (2026-05-30), Chromium 148 tabanlı
```

```toml
# crates/viscos-webview/Cargo.toml

[features]
default = ["wry-backend"]
wry-backend = ["dep:wry"]
cef-backend = ["dep:cef-rs", "viscos-webview/cef-impl"]

[dependencies]
wry = { version = "0.55", optional = true }
cef-rs = { git = "https://github.com/tauri-apps/cef-rs", tag = "cef-v148.3.0+148.0.9", optional = true }
tao = "0.35"
```

### 4.2 CEF Backend Kod

```rust
// crates/viscos-webview/src/cef.rs

use cef::{BrowserSettings, BrowserHost, WindowInfo, Client, ProcessId, ProcessMessage};
use std::sync::Arc;
use tao::window::Window;
use viscos_ipc::IpcBridge;
use super::backend::{WebViewBackend, WebViewConfig, WebViewHandle};

pub struct CefWebViewBackend;

impl CefWebViewBackend {
    pub fn new() -> Self { Self }
}

impl WebViewBackend for CefWebViewBackend {
    fn create(
        &self,
        window: &Window,
        config: &WebViewConfig,
        ipc: Arc<IpcBridge>,
    ) -> anyhow::Result<Box<dyn WebViewHandle>> {
        // CEF one-time initialization (process başına 1 kez)
        // Not: Bu genellikle main.rs'de çağrılır, backend init'ten önce
        // _ = cef::api_hash_sum(0, std::ptr::null());

        let window_info = WindowInfo {
            parent_window: Some(window.hwnd().0 as *mut _),
            width: 1200,
            height: 800,
            ..Default::default()
        };

        let client = ViscosCefClient::new(ipc.clone());
        let browser = BrowserHost::create_browser_sync(
            window_info,
            client,
            config.url.clone(),
            BrowserSettings::default(),
        )?;

        Ok(Box::new(CefWebViewHandle { browser }))
    }

    fn version(&self) -> &'static str { "CEF (Chromium Embedded Framework)" }

    fn known_issues(&self) -> &[&'static str] {
        &[
            "220-300 MB binary (Chromium runtime)",
            "Disk alanı +150 MB (cache)",
            "Kendi Chromium'unu güncellemek gerekir (Faz 8.0 auto-update)",
        ]
    }
}

pub struct CefWebViewHandle {
    browser: cef::Browser,
}

impl WebViewHandle for CefWebViewHandle {
    fn dispose_and_recreate(&self) -> anyhow::Result<()> {
        // CEF için dispose + recreate: yeniden oluşturmak yerine
        // browser.close() + yeni browser yarat. Process aynı kalır.
        self.browser.close(true)?;
        // Yeniden oluşturma caller'a düşer (WebViewManager)
        Ok(())
    }

    fn ipc_buffer_size(&self) -> usize {
        // CEF MessageRouter üzerinden IPC, buffer tracking WebView2'den farklı
        // Faz 4'te SharedMemoryRegion kullanılacak
        0
    }
}

struct ViscosCefClient {
    ipc: Arc<IpcBridge>,
}

impl ViscosCefClient {
    fn new(ipc: Arc<IpcBridge>) -> Self { Self { ipc } }
}

impl Client for ViscosCefClient {
    fn on_process_message_received(
        &self,
        _browser: cef::Browser,
        _frame: cef::Frame,
        _source_process: ProcessId,
        message: ProcessMessage,
    ) -> bool {
        // CEF IPC: JS → Rust
        let payload = message.name();
        if let Ok(cmd) = serde_json::from_str::<viscos_ipc::IpcCommand>(payload) {
            self.ipc.handle_command(cmd);
        }
        true
    }
}
```

### 4.3 Frontend Bridge (CEF uyumlu)

`frontend/src/bridge.ts` değişmiyor (zaten generic). CEF tarafında `cefQuery` API'si kullanılır (Discord'un web client'ı zaten `window.chrome.webview` veya `window.cefQuery` ile çalışır — Viscos `window.chrome.webview.postMessage` shim'i sağlar).

```typescript
// frontend/src/bridge.ts (Faz 1'den korunuyor, CEF MessageRouter ile uyumlu)
// CEF MessageRouter threshold-based: küçük komutlar → JSON postMessage,
// büyük blob'lar → SharedMemoryRegion (Faz 4'te)
```

---

## 5. CEF Binary Bundle Yapısı

### 5.1 İki Ayrı MSI

| MSI | Binary | Hedef |
|-----|--------|-------|
| `viscos-webview2.msi` | 15–25 MB | Win10, küçük binary tercih eden power user |
| `viscos-cef.msi` | 220–300 MB | Win11 default, 7/24 açık bırakan |

**Bundle:** Tek `viscos-setup.exe` indirilir → ilk çalıştırma `select_default_backend()` ile backend seçer → gerekli dosyaları indirir veya kullanıcıya MSI seçtirir.

### 5.2 GitHub Actions

```yaml
# .github/workflows/release.yml
jobs:
  build-webview2:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build WebView2 MSI
        run: |
          cargo build --release
          cargo wix
      - name: Upload
        uses: actions/upload-artifact@v4
        with:
          name: viscos-webview2
          path: target/wix/viscos-*.msi

  build-cef:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build CEF MSI
        run: |
          cargo build --release --features cef-backend
          cargo wix --features cef-backend
      - name: Upload
        uses: actions/upload-artifact@v4
        with:
          name: viscos-cef
          path: target/wix/viscos-cef-*.msi
```

### 5.3 WinGet Manifests

İki ayrı manifest:
- `winget/viscos.viscos.webview2.yaml` → `viscos-webview2.msi`
- `winget/viscos.viscos.cef.yaml` → `viscos-cef.msi`

---

## 6. İlk Çalıştırma Wizard'ı (Lightweight)

Faz 8.5'teki tam `BackendWizard` yerine Faz 1.6'da **lightweight tray notification**:

```rust
// crates/viscos-shell/src/cef_rollout_notification.rs

pub async fn show_cef_suggestion_if_needed(
    tray: &tao::tray::TrayIcon,
    telemetry: &Telemetry,
) -> anyhow::Result<()> {
    let rec = evaluate_cef_rollout(telemetry).await?;
    if matches!(rec, CefRolloutRecommendation::RecommendCefDefault) {
        let title = "Viscos: GDI leak tespit edildi";
        let body = "Son 24 saatte birden fazla WebView yeniden başlatması yaşandı. \
                    CEF backend (Chromium) denemek ister misiniz? \
                    [Ayarlar] > [Backend] > CEF seçin veya viscos-cef.msi indirin.";
        // Tray notification
        tray.show_notification(title, body)?;
    }
    Ok(())
}
```

**Davranış:**
- Kullanıcı her gün en fazla 1 bildirim alır (notification cooldown)
- Bildirim metni net: trade-off (binary büyüklüğü vs leak'siz)
- Kullanıcı isterse Ayarlar → Backend değiştirir

---

## 7. Backend Switch (Settings UI, Minimal)

Faz 5 (Native UI) tamamlanana kadar backend değişikliği için minimal CLI:

```rust
// crates/viscos-cli/src/main.rs (veya viscos shell alt komutu)
// cargo run -- viscos-backend set cef
// cargo run -- viscos-backend set webview2
// cargo run -- viscos-backend status

use clap::{Parser, Subcommand};
use viscos_config::Config;

#[derive(Parser)]
#[command(name = "viscos-backend")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Set { kind: String },
    Status,
    /// Telemetry'ye göre öneri göster
    Recommend,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load()?;
    match cli.cmd {
        Cmd::Set { kind } => {
            config.set("backend.kind", kind)?;
            config.save()?;
            println!("Backend set to: {} (restart required)", kind);
        }
        Cmd::Status => {
            println!("Backend: {}", config.get_str("backend.kind")?);
            println!("Binary: {}", env!("CARGO_PKG_NAME"));
        }
        Cmd::Recommend => {
            let telemetry = Telemetry::new(TelemetryConfig::default())?;
            let rec = evaluate_cef_rollout(&telemetry).await?;
            println!("Öneri: {:?}", rec);
        }
    }
    Ok(())
}
```

**Faz 5 sonrası:** Aynı işlem iced Ayarlar UI'ından yapılır.

---

## 8. CI Dual Pipeline

```yaml
# .github/workflows/test.yml

jobs:
  test-webview2:
    runs-on: windows-latest
    strategy:
      matrix:
        rust: [stable, beta]
    steps:
      - uses: actions/checkout@v4
      - run: rustup toolchain install ${{ matrix.rust }}
      - run: cargo test --workspace
      - run: cargo bench --workspace --no-run
      # 24h soak test scheduled job'da (Faz 1.5 telemetry)

  test-cef:
    runs-on: windows-latest
    strategy:
      matrix:
        rust: [stable]
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --workspace --features cef-backend
      - run: cargo bench --workspace --features cef-backend --no-run
      # CEF 24h soak test scheduled job'da
```

**Binary budget gate (CI):** Her iki backend için ayrı budget:
- WebView2: 25 MB hard limit
- CEF: 320 MB hard limit (Chromium runtime dahil)

---

## 9. Trade-off Dokümantasyonu

`docs/CEF-VS-WEBVIEW2.md` (Faz 8.5 plan'ından kopyalanır, güncellenir):

```markdown
# WebView2 vs CEF — Hangisini Seçmeli?

## WebView2

**Avantajlar:**
- 15-25 MB binary (çok hafif)
- OS WebView, Edge güncellemesi ile güvenlik fix otomatik
- Cold start < 1.5s
- Düşük idle RAM (~150-200 MB)

**Dezavantajlar:**
- Win11 GDI object leak (WebView2Feedback#5536) — Microsoft upstream'te çözümsüz (Haziran 2026)
- Faz 1'deki watchdog gerekiyor (soft restart her 5dk aktif kullanımda)
- RDP üzerinden GDI region leak (WV2#5266)

**Öner:** Win10, sık kapat-aç, disk alanı kısıtlı

## CEF (Chromium Embedded Framework)

**Avantajlar:**
- Leak'siz (Chromium olgun, Win32 GDI'ya bağımlı değil)
- Cross-platform tutarlılık (v2'de Linux/macOS aynı engine)
- RDP güvenli (WV2#5266'ya tabi değil)
- 7/24 açık bırakılabilir
- Faz 1.6 sonrası: Win11 default

**Dezavantajlar:**
- 220-300 MB binary (Chromium runtime)
- Cold start 1.5-2.5s
- Idle RAM +50-100 MB
- Kendi Chromium'unu güncellemek gerekir (Faz 8.0 auto-update)

**Öner:** Win11, 7/24 açık bırakan, RDP kullanan, disk alanı bol

## Hızlı Karar Tablosu

| Senaryo | Öneri |
|---------|-------|
| Win10 + günlük kullanım | WebView2 |
| Win11 + 7/24 açık | **CEF (default)** |
| Win11 + disk alanı kısıtlı | WebView2 + agresif watchdog 5000 |
| RDP üzerinden | **CEF (zorunlu)** |
| Sık kapat-aç | WebView2 |
| Multi-platform (Linux/macOS ilerde) | CEF |
```

---

## 10. Test Stratejisi (Faz 1.6)

| Test | Tip | Kabul |
|------|-----|-------|
| BackendKind detection | Unit | Win10/11 doğru |
| CEF initialize | Integration (lokal) | Browser oluşuyor |
| CEF IPC (Rust ↔ JS) | Integration | `window.chrome.webview.postMessage` shim çalışıyor |
| CEF cold start | Benchmark (lokal) | <2.5s |
| CEF 24 saatlik soak | Lokal | Leak yok, GDI sabit |
| Backend switch CLI | Integration | config.toml doğru yazılıyor |
| Backend wizard (lightweight) | Lokal | Notification görünüyor |
| Config override | Unit | Platform default geçersiz kılınıyor |
| Telemetry-driven recommendation | Unit (mock) | Threshold doğru |

---

## 11. Kabul Kriterleri (Definition of Done)

- [ ] `cef-rs` workspace dependency eklendi (`cef-v148.3.0+148.0.9`)
- [ ] `CefWebViewBackend` implementasyonu `cargo build --features cef-backend` ile derleniyor
- [ ] Win11 detection logic çalışıyor (build ≥22000)
- [ ] `select_default_backend()` telemetry override'ı doğru uyguluyor
- [ ] Config'te `backend.kind` override'ı çalışıyor
- [ ] İki MSI build: `viscos-webview2.msi` (≤25MB), `viscos-cef.msi` (≤320MB)
- [ ] CI dual pipeline: her iki backend için ayrı test/bench job
- [ ] CEF 24 saatlik soak: leak yok (GDI peak <5000, restart count 0)
- [ ] `viscos-backend` CLI komutu çalışıyor
- [ ] Tray notification "GDI leak tespit edildi" doğru tetikleniyor (mock telemetry)
- [ ] `docs/CEF-VS-WEBVIEW2.md` yayında
- [ ] MVP'de: Win11 default CEF, Win10 default WebView2

---

## 12. Karar Noktası (Faz 1.6 Sonu)

> 🔵 **İNSAN:** Win11 default CEF mi yoksa kullanıcıya wizard mı?
> - **Default CEF (önerilen):** Sıfır friction, Win11 kullanıcıları leak'ten korunur
> - Wizard ile sormak: Kullanıcıya seçim hakkı, ama friction
> - Default WebView2 + agresif öneri: Çoğu kullanıcı WebView2'de kalır, leak yaşar

> 🔵 **İNSAN:** CEF bundle boyutu kabul edilebilir mi? (170 MB artış)
> - **Evet (önerilen):** Hedef kitle power user, disk alanı yeterli
> - Hayır, sadece opt-in: Win11 kullanıcıları default WebView2'de kalır, leak yaşar
> - Hybrid: Win11 build 26200+ → CEF, eski Win11 → WebView2 (daha küçük risk grubu)

> 🔵 **İNSAN:** Telemetry force override agresifliği?
> - 5 restart/24h → Recommend (default değişmez, sadece bildirim)
> - 10 restart/24h → Force (sonraki açılışta CEF zorla)
> - Asla force (önerilen): Kullanıcıya saygı, leak ile yaşamayı seçebilir

> 🔵 **İNSAN:** CEF versiyonu nasıl pin?
> - Stable (önerilen): CEF stable tag, güvenlik
> - LTS (planlanmamış): CEF LTS yok, manual pin
> - Beta: Chromium birkaç minor önde, riskli

> 🔵 **İNSAN (Haziran 2026 — ADR-0012):** RDP session auto-detect kabul edilsin mi?
> - **Auto-detect CEF (önerilen):** Microsoft RDP bug'ı yapısal çözümsüz, kullanıcıyı korur
> - Sadece bildirim: Kullanıcı karar versin, friction
> - Devre dışı: Power user tercihi, RDP bilinçli kullanıyorsa
> - **Trade-off:** auto-detect CEF zorla = +220 MB binary RDP kullanıcısı için ama leak yok

> 🔵 **İNSAN (Haziran 2026 — ADR-0012 §2):** Frontend bridge.ts için [`bridge-resilience-research.md`](./bridge-resilience-research.md) referansı Faz 1.0 deliverable olarak yeterli mi?
> - **Evet (önerilen):** Faz 1.0'da yayınlanır, Faz 1.6 CEF IPC shim yazarken uygulanır
> - Erken spike: Faz 1.0 ilk günü
> - Geç: Faz 5'te (side panel eklenirken)

---

## 13. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| CEF binary corruption | Çalışmama | sha256 integrity check (Faz 8 distribution) |
| CEF versiyon disk space | Büyük | Bundle içinde değil, runtime download (Faz 8 self-update) |
| CEF update karmaşıklığı | Bakım | self_update ile birlikte güncelle (Faz 8) |
| CEF Discord Web uyumsuzluğu | Render bug | CEF stable + Discord Web stable test, v1.0.1 hotfix |
| Kullanıcı backend değiştiremezse | Sıkışma | CLI + config.toml + (Faz 5 sonrası) Ayarlar UI |
| Telemetry data kaybı | Öneri yanlış | SQLite WAL, opt-out default |
| CEF IPC shim bug | Discord client bozuk | Faz 1.6 sonunda acceptance test, gerçek hesap login |
| iki build matrix CI maliyeti | $ | Her backend için ayrı workflow, matrix 2× |

---

## 14. Çıkış → Faz 2.0

Bu faz tamamlandığında:
- Win11 kullanıcıları leak'siz Discord client deneyimi yaşıyor
- Win10 default WebView2 (daha hafif binary)
- Telemetry-driven recommendation çalışıyor
- Kullanıcı backend'i istediği zaman değiştirebilir (CLI/config)
- Faz 8.5'teki tam `BackendWizard` hala ileride, ama artık lightweight notification yeterli

**Faz 2.0:** Discord REST API + Auth (token storage, login flow, MFA). CEF backend stabilitesi kanıtlanmış, MVP'nin geri kalanı normal akışta ilerler.

**Faz 8.5:** Tam `BackendWizard` (önceki plandaki gibi) — artık sadece "ilk kez CEF'e geçiş" wizard'ı değil, **backend yönetim UI'ı**. Viscos v2.0'a kadar korunur.
