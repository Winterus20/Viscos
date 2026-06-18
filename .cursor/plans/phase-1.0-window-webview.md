---
name: Phase 1.0 — Window + WebView + GDI Watchdog
overview: tao ile native pencere, wry ile WebView2 (Discord web client), tray icon, IPC köprüsü iskeleti (pull-based), ve KRİTİK olarak viscos-watchdog crate (GDI leak savunması Faz 8'den buraya alındı). Haziran 2026 tradeoff analizi sonrası: iced 0.14, watchdog threshold 7000/9000, draft autosave hook eklendi.
isProject: false
todos:
  - id: shell-crate
    content: viscos-shell crate oluştur (tao event loop, iced renderer altyapısı)
    status: pending
  - id: webview-crate
    content: viscos-webview crate (wry wrapper, Discord.com/app yükle)
    status: pending
  - id: webview-backend-trait
    content: WebViewBackend trait (Faz 1.6 + Faz 8.5 için abstraction)
    status: pending
  - id: ipc-crate-skeleton
    content: viscos-ipc crate iskeleti (pull-based command/event)
    status: pending
  - id: frontend-bridge
    content: frontend/ TS wrapper, window.viscos bridge (preload injection)
    status: pending
  - id: tray-icon
    content: System tray icon + context menu
    status: pending
  - id: watchdog-crate
    content: viscos-watchdog crate (GDI counter, IPC buffer tracker, heap, draft autosave hook)
    status: pending
  - id: watchdog-tests
    content: GDI counter unit testleri (mock handle ile)
    status: pending
  - id: devtools-shortcut
    content: DevTools açma kısayolu (F12)
    status: pending
  - id: preload-script
    content: Preload script injection (window.viscos bridge)
    status: pending
  - id: draft-autosave
    content: Draft mesaj autosave hook (pre-restart koruması, watchdog ile entegre)
    status: pending
  - id: soak-test-plan
    content: 24 saatlik soak test planı (lokal manuel)
    status: pending
  - id: iced-spike
    content: iced 0.14 + WebView overlay spike (1 hf, native side panel + Discord Web aynı pencere, frame timing ölçümü) — ADR-0012 §5
    status: pending
  - id: bridge-resilience
    content: frontend/src/bridge.ts selector resilience kuralları ([aria-label] > [role] > [class*="..."], Vencord findByProps) — ADR-0012 §2
    status: pending
  - id: bridge-resilience-doc
    content: crates/viscos-webview/BRIDGE-RESILIENCE.md (AI-PR review checklist)
    status: pending
---

# Phase 1.0 — Window + WebView + GDI Watchdog

> **Süre:** 2–3 hafta
> **Hedef:** Çalışan pencere + WebView2'de Discord.com/app yüklü, IPC köprüsü iskeleti, GDI watchdog Faz 1'de (Faz 8'den öne alındı çünkü Microsoft upstream'te fix yok).
> **Kritik referans:** [`webview2-hardening.md`](./webview2-hardening.md) ve [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md) (Haziran 2026 karar kaynağı)
> **Önceki faz:** [`phase-0.5-ai-workflow-setup.md`](./phase-0.5-ai-workflow-setup.md)
> **Sonraki faz:** [`phase-1.5-telemetry-and-restart-optimization.md`](./phase-1.5-telemetry-and-restart-optimization.md) (yeniden adlandırıldı, throttle kaldırıldı)

---

## 1. Neden GDI Watchdog Faz 1'de?

Önceki planda watchdog Faz 8'deydi. **Faz 1'e alındı** çünkü:

1. **WebView2 GDI leak (wry#1691 / MicrosoftEdge/WebView2Feedback#5536) Microsoft tarafından kabul edilmiş, fix yok (Haziran 2026 STATE: OPEN).** WebView2 ilk açılışta bile leak başlar; watchdog olmadan test etmek imkansız.
2. **Faz 8'e bırakmak "1 ay sonra çökecek client" demek.** 7/24 açık kalan kullanıcılar için 1-2 hafta içinde crash.
3. **Watchdog'un kendisi küçük (~200 satır Rust),** tek bir `GetGuiResources` polling task. Yeni crate açma maliyeti düşük.
4. **Aktif kullanımda ~24 saatte ~48 restart** (8 saat × 6 restart/saat = 4000 GDI/30sn). Bu restart'lar kabul edilemez UX; bu yüzden watchdog Faz 1.5'te telemetry + draft autosave ile güçlendirilecek ve Faz 1.6'da Win11 default CEF'e geçiş planlanıyor.

**Çözüm:** İlk WebView2 instance'ı oluşturulduğu anda watchdog da başlar. Leak olduğunu **erken tespit** ederiz, auto-restart ile **maskeleriz**, sonraki fazlarda telemetry + draft autosave (Faz 1.5) ve Win11 CEF default (Faz 1.6) ile **yapısal** olarak çözeriz.

Detay: [`webview2-hardening.md` Bölüm 4 Katman 1](./webview2-hardening.md#katman-1-watchdog-faz-1--viscos-watchdog-crate).

---

## 2. Crate Ekleme (Faz 0.0 Workspace'ine)

`Cargo.toml` `members` array'ine ekle:

```toml
[workspace]
members = [
    "crates/viscos-core",
    "crates/viscos-config",
    "crates/viscos-error",
    "crates/viscos-log",
    "crates/viscos-shell",      # YENİ
    "crates/viscos-webview",    # YENİ
    "crates/viscos-ipc",        # YENİ
    "crates/viscos-watchdog",   # YENİ
    "crates/viscos",
]

[workspace.dependencies]
# Pencere (v1 = Windows-only; Leto Discord client kanıtı)
tao = "0.35"  # Mart 2026; tray + menu + global hotkey built-in
wry = "0.55"  # Mayıs 2026; Win11 white flash fix, webview2-com 0.38+

# GUI (Faz 5'te yoğun kullanılacak, Faz 1'de sadece temel)
# iced 0.14 — son deneysel sürüm (1.0 öncesi final API freeze 2026)
# Reactive rendering default, COSMIC resize lag çözüldü, time-travel debugger
iced = { version = "0.14", features = ["wgpu", "tokio"] }

# IPC
serde_json = "1.0"
```

**Versiyon notları (Haziran 2026):**
- `tao = "0.35"`: Tauri ekibi tarafından aktif bakım, tray + menu built-in (alternatif: `winit = "0.31"` beta + `muda` + `tray-icon` external — gerek yok)
- `wry = "0.55"`: latest stable; `webview2-com 0.38` ile Win11 white flash fix
- `iced = "0.14"`: Aralık 2025, son deneysel sürüm, 1.0 freeze'ine yakın. Reactive rendering default. COSMIC resize lag (`pop-os/libcosmic#753`) çözüldü. Halloy, Sniffnet, Neothesia production kanıtı.

Detaylı tradeoff: [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md).

---

## 3. Crate Detayları

### 3.1 `viscos-shell`

Pencere yönetimi, tao event loop, iced renderer altyapısı.

```rust
// crates/viscos-shell/src/lib.rs
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Icon},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, MenuItem, PredefinedMenuItem},
};
use viscos_webview::WebViewManager;
use viscos_ipc::IpcBridge;
use viscos_watchdog::Watchdog;
use std::sync::Arc;

pub struct Shell {
    window: tao::window::Window,
    webview: WebViewManager,
    ipc: Arc<IpcBridge>,
    watchdog: Watchdog,
}

impl Shell {
    pub fn run() -> anyhow::Result<()> {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Viscos")
            .with_inner_size(1200.0, 800.0)
            .build(&event_loop)?;
        
        let ipc = Arc::new(IpcBridge::new());
        let webview = WebViewManager::new(&window, ipc.clone())?;
        let watchdog = Watchdog::new(webview.handle());
        
        // Tray icon
        let tray_menu = Menu::new();
        let status_item = MenuItem::with_id("status", "Online", true, None::<&str>);
        let quit_item = MenuItem::with_id("quit", "Quit", true, None::<&str>);
        tray_menu.append_items(&[&status_item, &PredefinedMenuItem::separator(), &quit_item])?;
        
        let _tray = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Viscos")
            .build()?;
        
        // Watchdog background task başlat
        watchdog.spawn();
        
        let ipc_clone = ipc.clone();
        let webview_handle = webview.handle();
        
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(_) => webview_handle.resize(),
                    _ => {}
                }
                Event::MenuEvent { menu_id, .. } => {
                    if menu_id == "quit" {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                _ => {}
            }
        });
    }
}
```

**Bu fazda:**
- Boş pencere açılır
- WebView2 Discord.com/app yükler
- Tray icon görünür
- F12 → DevTools açılır
- Watchdog background task aktif

### 3.2 `viscos-webview`

wry wrapper, WebViewBackend trait (Faz 8.5 için abstraction).

```rust
// crates/viscos-webview/src/lib.rs
use std::sync::Arc;
use tao::window::Window;
use wry::{WebView, WebViewBuilder, WebContext};
use viscos_ipc::IpcBridge;
use serde::{Serialize, Deserialize};

pub mod backend;
pub use backend::{WebViewBackend, BackendKind};

#[derive(Debug, Clone)]
pub struct WebViewConfig {
    pub url: String,
    pub devtools: bool,
    pub user_agent: Option<String>,
}

pub struct WebViewManager {
    webview: WebView,
    backend: BackendKind,
}

impl WebViewManager {
    pub fn new(window: &Window, ipc: Arc<IpcBridge>) -> anyhow::Result<Self> {
        let config = WebViewConfig {
            url: "https://discord.com/app".to_string(),
            devtools: cfg!(debug_assertions),
            user_agent: None,
        };
        
        // WebViewBackend trait üzerinden oluştur (Faz 1'de sadece WebView2)
        let backend: Box<dyn WebViewBackend> = match BackendKind::default() {
            BackendKind::WebView2 => Box::new(webview2::WryWebView2Backend::new()),
            BackendKind::Cef => unreachable!("CEF Faz 8.5'te aktif"),
        };
        
        let webview = backend.create(window, &config, ipc.clone())?;
        Ok(Self { webview, backend: BackendKind::WebView2 })
    }
    
    pub fn handle(&self) -> &WebView { &self.webview }
    pub fn dispose_and_recreate(&mut self) -> anyhow::Result<()> {
        // Watchdog tetiklediğinde: WebView dispose + recreate
        // (Detay: webview2-hardening.md Katman 1)
        todo!("Faz 1 sonu: implement")
    }
}

// crates/viscos-webview/src/backend.rs
pub trait WebViewBackend: Send + Sync {
    fn create(&self, window: &tao::window::Window, config: &WebViewConfig, ipc: std::sync::Arc<viscos_ipc::IpcBridge>) -> anyhow::Result<wry::WebView>;
    fn version(&self) -> &'static str;
    fn known_issues(&self) -> &[&'static str];

    /// Faz 4'te implemente edilecek: büyük binary blob'ları (avatar, sticker, emoji)
    /// WebView2 SharedBuffer veya CEF SharedMemoryRegion üzerinden zero-copy transfer.
    /// Faz 1'de stub olarak kalır; default implementation hata döner.
    /// Detay: `webview2-hardening.md` Bölüm 3 + `phase-4.0-cache-media.md` Bölüm 4.4.
    fn post_shared_buffer(&self, _bytes: &[u8], _metadata: &str) -> anyhow::Result<()> {
        anyhow::bail!("post_shared_buffer Faz 4'te implemente edilecek (bkz. phase-4.0-cache-media.md)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    WebView2,
    Cef,
}

impl Default for BackendKind {
    fn default() -> Self { BackendKind::WebView2 }
}

// crates/viscos-webview/src/webview2.rs
pub struct WryWebView2Backend;

impl WryWebView2Backend {
    pub fn new() -> Self { Self }
}

impl WebViewBackend for WryWebView2Backend {
    fn create(&self, window: &tao::window::Window, config: &WebViewConfig, ipc: std::sync::Arc<viscos_ipc::IpcBridge>) -> anyhow::Result<wry::WebView> {
        let preload_script = include_str!("../../frontend/dist/preload.js");
        let ipc_for_closure = ipc.clone();
        
        WebViewBuilder::new(window)
            .with_url(&config.url)
            .with_devtools(config.devtools)
            .with_initialization_script(preload_script)
            .with_ipc_handler(move |msg| {
                // JS → Rust: pull-based command
                if let Ok(cmd) = serde_json::from_str::<IpcCommand>(msg.body()) {
                    ipc_for_closure.handle_command(cmd);
                }
            })
            .build()
            .map_err(Into::into)
    }
    
    fn version(&self) -> &'static str { "WebView2 (wry)" }
    fn known_issues(&self) -> &[&'static str] {
        &[
            "wry#1691: GDI object leak on mouse hover (Win11)",
            "tauri#13758: eval_script unmanaged lifecycle (use pull-based)",
            "tauri#13133: channel callback leak (use delete onmessage)",
        ]
    }
}
```

### 3.3 `viscos-ipc` (İskelet)

```rust
// crates/viscos-ipc/src/lib.rs
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IpcCommand {
    GetState { key: String },
    GetMessages { channel_id: String, limit: u32 },
    SendMessage { channel_id: String, content: String },
    // Faz 2'de eklenecek: Auth, vb.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IpcEvent {
    StateChanged { key: String },
    NewMessage { channel_id: String },
    TrayBadgeUpdate { count: u32 },
    WatchdogAlert { kind: WatchdogKind, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchdogKind {
    GdiLeakWarning,
    GdiLeakCritical,
    IpcBufferWarning,
    IpcBufferCritical,
}

pub struct IpcBridge {
    state: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    event_tx: tokio::sync::broadcast::Sender<IpcEvent>,
}

impl IpcBridge {
    pub fn new() -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }
    
    pub async fn handle_command(&self, cmd: IpcCommand) -> Option<serde_json::Value> {
        match cmd {
            IpcCommand::GetState { key } => {
                self.state.read().await.get(&key).cloned()
            }
            IpcCommand::GetMessages { .. } => None, // Faz 2'de implement
            IpcCommand::SendMessage { .. } => None, // Faz 2'de implement
        }
    }
    
    pub fn emit(&self, event: IpcEvent) {
        let _ = self.event_tx.send(event);
    }
    
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<IpcEvent> {
        self.event_tx.subscribe()
    }
}
```

### 3.4 `viscos-watchdog` (KRİTİK)

```rust
// crates/viscos-watchdog/src/lib.rs
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn, error};
use viscos_ipc::{IpcBridge, IpcEvent, WatchdogKind};

#[cfg(windows)]
mod platform {
    use windows::Win32::UI::WindowsAndMessaging::GetGuiResources;
    use windows::Win32::System::Threading::GetCurrentProcess;

    pub fn gdi_count() -> u32 {
        unsafe {
            GetGuiResources(GetCurrentProcess(), 0) // GR_GDIOBJECTS
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn gdi_count() -> u32 { 0 } // Stub
}

pub struct WatchdogConfig {
    pub gdi_warning: u32,    // default: 7000 (Haziran 2026: erken uyarı)
    pub gdi_critical: u32,   // default: 9000 (restart tetikleyici)
    pub ipc_warning: usize,  // default: 50MB
    pub ipc_critical: usize, // default: 100MB
    pub poll_interval: Duration, // default: 30s
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        // Haziran 2026 güncellemesi: 5000/8000 → 7000/9000
        // Sebep: Win11 WebView2 GDI leak (~4000/30sn) için daha erken uyarı,
        // ama restart threshold'unu 9000'e çekerek false-positive'i azalt.
        // Restart arası gap <2s hedefi: 9000'de restart = ~5dk aktif kullanım aralığı.
        // Bkz. window-webview-watchdog-tradeoffs.md Bölüm 4.4.
        Self {
            gdi_warning: 7000,
            gdi_critical: 9000,
            ipc_warning: 50 * 1024 * 1024,
            ipc_critical: 100 * 1024 * 1024,
            poll_interval: Duration::from_secs(30),
        }
    }
}

pub struct Watchdog {
    config: WatchdogConfig,
    ipc: Arc<IpcBridge>,
    webview_handle: Arc<dyn WebViewHandle>,
    draft_autosave: Arc<dyn DraftAutosave>,
}

pub trait WebViewHandle: Send + Sync {
    fn dispose_and_recreate(&self) -> anyhow::Result<()>;
    fn ipc_buffer_size(&self) -> usize;
}

/// Pre-restart hook: mesaj taslaklarını SQLite'a yaz.
/// Restart sonrası kullanıcının yazdığı kaybolmaz.
/// Faz 2'de viscos-cache (SQLite) ile entegre olacak; Faz 1'de in-memory stub yeterli.
pub trait DraftAutosave: Send + Sync {
    fn snapshot_open_composers(&self) -> anyhow::Result<usize>;
}

impl Watchdog {
    pub fn new(
        config: WatchdogConfig,
        ipc: Arc<IpcBridge>,
        webview: Arc<dyn WebViewHandle>,
        draft_autosave: Arc<dyn DraftAutosave>,
    ) -> Self {
        Self { config, ipc, webview_handle: webview, draft_autosave }
    }

    pub fn spawn(self) {
        tokio::spawn(async move {
            let mut ticker = interval(self.config.poll_interval);
            let mut last_gdi: u32 = 0;

            loop {
                ticker.tick().await;

                // GDI kontrol
                let gdi = platform::gdi_count();
                let delta = gdi.saturating_sub(last_gdi);
                last_gdi = gdi;

                if gdi >= self.config.gdi_critical {
                    error!(gdi, delta, "GDI CRITICAL: pre-restart hook tetikleniyor");

                    // 1) Pre-restart hook: draft mesajları SQLite'a yaz (Faz 2'de)
                    let drafts_saved = match self.draft_autosave.snapshot_open_composers() {
                        Ok(n) => { tracing::info!(drafts = n, "Draft autosave OK"); n }
                        Err(e) => { tracing::error!(?e, "Draft autosave başarısız"); 0 }
                    };

                    // 2) Soft restart: WebView dispose + recreate (process yaşar)
                    self.ipc.emit(IpcEvent::WatchdogAlert {
                        kind: WatchdogKind::GdiLeakCritical,
                        message: format!("GDI {} → restart, {} draft saved", gdi, drafts_saved),
                    });
                    if let Err(e) = self.webview_handle.dispose_and_recreate() {
                        error!(?e, "WebView recreate başarısız");
                        // Hard restart fallback (process kill + relaunch) — Faz 8 sonrası
                    }
                    last_gdi = 0; // Reset
                } else if gdi >= self.config.gdi_warning {
                    warn!(gdi, delta, "GDI WARNING");
                    self.ipc.emit(IpcEvent::WatchdogAlert {
                        kind: WatchdogKind::GdiLeakWarning,
                        message: format!("GDI {}", gdi),
                    });
                } else {
                    info!(gdi, delta, "GDI OK");
                }

                // IPC buffer kontrol
                let ipc_size = self.webview_handle.ipc_buffer_size();
                if ipc_size >= self.config.ipc_critical {
                    error!(ipc_size, "IPC BUFFER CRITICAL");
                    self.ipc.emit(IpcEvent::WatchdogAlert {
                        kind: WatchdogKind::IpcBufferCritical,
                        message: format!("IPC {} bytes", ipc_size),
                    });
                } else if ipc_size >= self.config.ipc_warning {
                    warn!(ipc_size, "IPC BUFFER WARNING");
                }
            }
        });
    }
}

// Unit test (mock handle ile)
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct MockHandle {
        gdi: AtomicU32,
        recreate_count: AtomicU32,
    }

    impl WebViewHandle for MockHandle {
        fn dispose_and_recreate(&self) -> anyhow::Result<()> {
            self.recreate_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn ipc_buffer_size(&self) -> usize { 0 }
    }

    struct MockAutosave;
    impl DraftAutosave for MockAutosave {
        fn snapshot_open_composers(&self) -> anyhow::Result<usize> { Ok(2) }
    }

    #[test]
    fn gdi_threshold_triggers_restart() {
        // Mock: gdi_count mock'la simüle et
        // Integration test (gerçek GDI): ayrı #[ignore] test
    }

    #[test]
    fn ipc_critical_emits_event() {
        // ...
    }
}
```

**Not:** Gerçek GDI test mock'lu unit test + lokal manuel integration test. CI'da GDI testi çalıştırma (Windows runner'da GUI yok, yanlış sonuç verir).

**Haziran 2026 değişiklik notları:**
- Threshold **5000/8000 → 7000/9000**: daha erken uyarı + restart'ta biraz daha geç tetikleme (Win11 leak pattern'ine göre).
- **Draft autosave hook** eklendi: pre-restart mesaj taslaklarını korur. Faz 1'de in-memory stub; Faz 2'de `viscos-cache` (SQLite) entegrasyonu.
- Hard restart fallback (process kill) Faz 8 sonrası — Faz 1'de sadece soft restart.

---

## 4. Frontend Bridge

### 4.1 `frontend/src/bridge.ts`

```typescript
// frontend/src/bridge.ts
// Bu dosya WebView2'ye preload.js olarak inject edilir (Vite build → dist/preload.js)

export interface IpcCommand {
  type: string;
  data: any;
}

declare global {
  interface Window {
    viscos: {
      invoke: <T = any>(cmd: IpcCommand) => Promise<T>;
      onEvent: (handler: (event: any) => void) => () => void; // unsubscribe return
    };
  }
}

const pending = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();
let cmdId = 0;

// Pull-based: her zaman JS → Rust invoke
window.viscos = {
  invoke: <T = any>(cmd: IpcCommand): Promise<T> => {
    return new Promise((resolve, reject) => {
      const id = ++cmdId;
      pending.set(id, { resolve, reject });
      // Chrome DevTools Protocol channel yerine doğrudan postMessage (wry IPC)
      window.ipc.postMessage(JSON.stringify({ id, cmd }));
    });
  },
  onEvent: (handler) => {
    const wrapped = (event: MessageEvent) => {
      if (event.data?.kind === 'event') {
        handler(event.data.payload);
      } else if (event.data?.kind === 'response') {
        const { id, result, error } = event.data;
        const p = pending.get(id);
        if (p) {
          pending.delete(id);
          error ? p.reject(error) : p.resolve(result);
        }
      }
    };
    window.addEventListener('message', wrapped);
    // ÖNEMLİ: tauri#13133 — unmount sırasında cleanup
    return () => {
      window.removeEventListener('message', wrapped);
      // Channel callback leak önleme
      delete (wrapped as any).__viscos_handler;
    };
  },
};
```

### 4.2 Frontend Build

`frontend/package.json`:
```json
{
  "name": "viscos-frontend",
  "version": "0.1.0",
  "scripts": {
    "build": "esbuild src/bridge.ts --bundle --format=iife --outfile=dist/preload.js --target=es2022"
  },
  "devDependencies": {
    "esbuild": "^0.21.0",
    "typescript": "^5.4.0"
  }
}
```

`frontend/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "esModuleInterop": true
  },
  "include": ["src/**/*"]
}
```

`frontend/dist/` build çıktısı (gitignore'lı, build sırasında üretilir).

**Build adımı:** CI'da `pnpm install && pnpm build` (frontend için GitHub Actions step).

---

## 5. Cargo Build Profili (Faz 1+)

`Cargo.toml` workspace:

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

**Faz 1 sonunda binary hedefi:** ~ 10–15 MB (boş pencere + WebView, henüz cache/API yok).

---

## 6. Test Stratejisi (Faz 1.0)

| Test | Tip | Not |
|------|-----|-----|
| WebView2 oluşturma | Integration (lokal) | Discord.com/app yükleniyor mu? |
| IPC invoke roundtrip | Integration | JS → Rust → JS response |
| GDI counter polling | Unit (mock) | Threshold logic |
| Watchdog recreate tetikleme | Unit (mock) | 8000+ GDI'de dispose+recreate |
| Tray icon | Manuel | Lokal build, tray görünüyor mu |
| DevTools (F12) | Manuel | Kısayol çalışıyor mu |
| **24 saatlik soak test** | **Lokal manuel** | Mouse hover %50, scroll %30, idle %20 → crash yok |

---

## 7. Kabul Kriterleri (Definition of Done)

- [ ] `cargo build --release` başarılı, binary < 15 MB
- [ ] Uygulama açılınca Discord.com/app yükleniyor
- [ ] F12 → DevTools açılıyor
- [ ] System tray icon görünüyor, context menu çalışıyor
- [ ] `window.viscos.invoke({type: "GetState", data: {key: "test"}})` JS konsolundan çalışıyor
- [ ] Watchdog background task aktif, 30s'de bir GDI sayacı logluyor
- [ ] Mock test ile 8000+ GDI'de dispose+recreate tetikleniyor
- [ ] 24 saatlik soak test'te (lokal) crash yok, GDI 5000'in altında seyrediyor
- [ ] `cargo clippy -- -D warnings` temiz
- [ ] `cargo test` tüm geçer
- [ ] `docs/AI-WORKFLOW.md` güncel (ilk AI task deneyimi buraya not düşülür)

---

## 8. Karar Noktası (Faz 1.0 Sonu)

> 🔵 **İNSAN:** GDI threshold'ları ne olsun? (Haziran 2026 önerisi: 7000/9000)
> - 7000/9000 (default — önerilen): restart arası ~5dk aktif kullanım, restart <2s gap
> - 5000/8000 (eski plan): daha erken restart ama kullanıcı daha sık kesilir
> - Trade-off: erken müdahale = sık recreate, kullanıcı rahatsız; geç müdahale = crash riski

> 🔵 **İNSAN:** Auto-restart agresifliği? (Haziran 2026 önerisi: soft + draft autosave)
> - Soft restart + draft autosave (önerilen): mesaj kaybı 0, gap <2s
> - Dialog göster (12K'da): "Discord'u yeniden başlat" sor
> - Hard restart (process kill): Faz 8 sonrası fallback

> 🔵 **İNSAN:** İlk soak test lokal mi, CI mı?
> - Lokal manuel (önerilen): 24 saat kişisel makinede
> - CI scheduled job: GitHub Actions nightly, 6-8 saat (runner limiti)
> - Trade-off: coverage vs CI maliyeti

> 🔵 **İNSAN:** Faz 1.6 Win11 CEF default'a geçiş kabul edilsin mi? (Trade-off: binary 20 MB → 170 MB, Win11 kullanıcılarının %100'ü bug'a açık)
> - Win11 default CEF (önerilen): disk alanı kabul, leak'siz UX
> - Win11 default WebView2 + agresif watchdog 5000: küçük binary ama sık restart
> - Hybrid: Win11 build 26200+ → CEF, eski → WebView2

> 🔵 **İNSAN (Haziran 2026 — ADR-0012 §5):** `iced 0.14` production spike sonucu nedir?
> - **iced 0.14 onaylandı** (önerilen, Faz 1.0 ilk haftası spike tamamlanırsa): reactive rendering + wgpu + COSMIC resize lag fix üretim kanıtı
> - iced 0.13 downgrade: kanıtlanmış ama yeni feature'lar yok (resize lag fix yok)
> - egui'ye geçiş (immediate-mode): farklı trade-off, native shell için yetersiz
> - **Trade-off:** spike Faz 1.0'a 1 hafta ekler. Olumsuz sonuç = Faz 1.0 ortasında downgrade mümkün (tao + wry etkilenmez)

> 🔵 **İNSAN (Haziran 2026 — ADR-0012 §2):** Bridge.ts selector resilience kuralları ne kadar sıkı uygulansın?
> - **Sıkı (önerilen):** AI-PR review checklist zorunlu, hashed class selector → otomatik reject
> - Orta: code reviewer discretion, en iyi çabayla
> - Gevşek: sadece doküman, AI öğrenir
> - **Trade-off:** sıkı = +5-10 dk/PR review süresi, ama Discord Mart 2025 gibi UI revamp'larında bridge kırılmaz

---

## 9. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| WebView2 dispose crash | Uygulama çöker | Watchdog dispose+recreate try/catch, panic recovery |
| GDI counter yanlış değer | Yanlış alarm | İlk 60s "warmup" skip, baseline tut |
| IPC bridge deadlock | UI donar | Tüm command'lar async, timeout 5s |
| Tray icon path yanlış | Görünmez | Build script'te resource embedding |
| Preload script büyük | Performance | Esbuild minify, gzip |

---

## 4.5 Bridge Resilience Rules (ADR-0012 §2 — YENİ)

Discord sık sık DOM/class değiştiriyor. **Webpack-generated hashed class name'ler** her deploy'da farklı (`message__5126c`, `username_c19a55`). `frontend/src/bridge.ts` için zorunlu best-practices:

### Selector Hiyerarşisi (Öncelik sırasıyla)

```typescript
// frontend/src/bridge.ts — Selector Resilience Rules

// ✅ 1. Öncelik: aria-label ve role attribute selector'ları (Discord bunları değiştirmez)
const messageItem = document.querySelector('[id^="message-content-"]');
const channelName = document.querySelector('[aria-label*="channel"]');

// ✅ 2. Öncelik: Discord'un kendi internal store'una read-only hook
//    Vencord pattern'i: findByProps / findByCode (webpack module proxy)
const userStore = viscos.webpack.findByProps('getCurrentUser');
const unread = userStore.getUnreadCount(channelId);

// ✅ 3. Öncelik: Bridge state pull (native taraftan veri çek)
//    DOM observer'a güvenme, native Rust tarafı truth source olur
const unreadCount = await viscos.invoke({
  type: 'GetUnreadCount',
  data: { channel_id: channelId }
});

// ❌ YANLIŞ: Hashed class name'ler (her deploy'da kırılır)
const messageItem = document.querySelector('.message__5126c');

// ❌ YANLIŞ: querySelector ile React prop'larını okumaya çalışmak
//    (Discord React state'i DOM'a yazmıyor)
const userName = document.querySelector('.username_c19a55')?.textContent;

// ❌ YANLIŞ: MutationObserver + heuristic ile unread count tahmin etmek
const observer = new MutationObserver(() => {
  const count = document.querySelectorAll('[class*="unread"]').length;
});
```

### Vencord Webpack Module Proxy

`crates/viscos-webview/BRIDGE-RESILIENCE.md` Faz 1.0 deliverable olarak yazılır. İçerik:

```typescript
// frontend/src/webpack-shim.ts (Faz 1.0 — preload'a dahil)
// Discord'un kendi webpack instance'ına proxy (Vencord pattern)
// Function.prototype.m setter'ı override ederek webpack chunk factory'lerini yakala

let discordWebpack: any = null;

const originalSetter = Function.prototype.m;
Object.defineProperty(Function.prototype, 'm', {
  set(value) {
    // Discord'un wreq.m (module factories) atamasını yakala
    if (typeof value === 'object' && value !== null && !discordWebpack) {
      // wreq stack trace doğrulaması (Discord React DevTools'tan ayırt et)
      const stack = new Error().stack || '';
      if (stack.includes('discord.com/assets/') && !stack.includes('react-devtools')) {
        discordWebpack = { m: value, c: (this as any).c };
        originalSetter.call(this, value);
        return;
      }
    }
    originalSetter.call(this, value);
  },
  get() { return originalSetter; },
  configurable: true,
});

// Public API
window.viscos.webpack = {
  findByProps: (...propNames: string[]) => {
    if (!discordWebpack) return null;
    for (const id in discordWebpack.c) {
      const module = discordWebpack.c[id]?.exports;
      if (module && propNames.every(p => p in module)) return module;
    }
    return null;
  },
  findByCode: (...codes: string[]) => { /* ... */ },
  waitFor: (filter: (m: any) => boolean) => { /* ... */ },
};
```

### AI-PR Review Checklist

Her bridge.ts PR'ında code reviewer şunları kontrol eder:

- [ ] Yeni `querySelector` call'larında `[aria-label]` veya `[role]` attribute selector öncelikli mi?
- [ ] Hashed class name selector var mı? (`\.message_[a-f0-9]+` pattern) → PR reject
- [ ] MutationObserver + heuristic var mı? → PR reject (bridge pull kullan)
- [ ] `viscos.webpack.findByProps` ile native tarafa veri çekilebilir mi?
- [ ] Cross-reference: [`bridge-resilience-research.md`](./bridge-resilience-research.md) (Haziran 2026)

**Referans kanıt:** browser-cli Discord skill, BetterDiscord styling guide, Vencord webpack integration — hepsi aynı sonuca varıyor. Detay: [`bridge-resilience-research.md`](./bridge-resilience-research.md).

---

## 10. Çıkış → Faz 1.5

Bu faz tamamlandığında:
- Çalışan Viscos penceresi
- WebView2'de Discord yüklü
- IPC köprüsü iskeleti (sadece `GetState` döner)
- GDI watchdog aktif (threshold 7000/9000), pre-restart draft autosave hook'u
- İlk soak test verisi toplanmış
- **`frontend/src/bridge.ts` selector resilience kuralları uygulanmış** (ADR-0012 §2)
- **`crates/viscos-webview/BRIDGE-RESILIENCE.md` AI-PR review checklist yayında**
- **`iced 0.14` + WebView overlay spike tamamlanmış** (ADR-0012 §5)

Faz 1.5 → Telemetry backend + restart optimizasyonu + IPC audit + channel cleanup. Mouse throttle **kaldırıldı** (Microsoft kanıtladı: DirectComposition bypass, etkisiz). Bkz. [`window-webview-watchdog-tradeoffs.md` Bölüm 4.2](./window-webview-watchdog-tradeoffs.md#42-sorun).

Faz 1.6 (koşullu) → Win11 default CEF backend + Win10 default WebView2. CEF artık MVP'nin parçası. RDP auto-detect (Haziran 2026 — ADR-0012 + Faz 1.6 Bölüm 6 eki).
