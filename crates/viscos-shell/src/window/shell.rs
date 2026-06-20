//! `Shell` struct, `ShellBuilder`, and `ResizeObserver`.
//!
//! `Shell` owns the `tao::EventLoop` + `WebViewWindow` lifecycle in
//! Faz 1.6 Dalga 1b. When constructed with a `SharedBackend` (via
//! [`ShellBuilder::backend`]), [`Shell::run`] creates a real
//! `tao::event_loop::EventLoop`, asks the backend to construct the
//! native window + WebView, and runs the event loop until the window
//! is closed or Ctrl-C is signalled.
//!
//! When constructed without a backend (legacy / test path), [`Shell::run`]
//! preserves the Faz 1.0 stub behaviour — it logs the configuration and
//! returns `Ok(())` immediately. This keeps the existing CI unit tests
//! stable on headless runners without requiring a display.
//!
//! `ShellBuilder` provides a fluent API for constructing a `Shell`.
//! `ResizeObserver` is a placeholder frame-timing probe (Faz 1.5 will add real metrics).

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use tao::window::WindowBuilder;
use viscos_webview::{SharedBackend, WebViewBackend, WebViewWindow, WindowConfig};

use super::config::{ShellConfig, TrayMenu};
use super::tray::default_tray_menu;

/// Resize davranışı gözlemcisi (placeholder).
///
/// Faz 1.0'da sadece struct + method imzaları. Faz 1.5 telemetry'sinde
/// actual frame time / lag metric'leri toplanacak.
#[derive(Debug, Clone, Default)]
pub struct ResizeObserver;

impl ResizeObserver {
    /// Yeni resize observer oluştur.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Frame time ölçümü (microseconds). Stub: sabit 16_667µs (60 FPS).
    #[must_use]
    pub const fn frame_time_us(&self) -> u64 {
        16_667
    }

    /// Resize laggy mi? Stub: false (gerçek ölçüm Faz 1.5'te).
    #[must_use]
    pub const fn is_laggy(&self) -> bool {
        false
    }
}

/// Process-global Ctrl-C flag.
///
/// Set by an OS thread spawned in [`Shell::run`] that owns its own
/// tokio runtime + listens on `tokio::signal::ctrl_c`. The tao
/// event-loop callback polls this flag each iteration and exits
/// gracefully when the user presses Ctrl-C.
///
/// We use a process-global rather than `Arc<AtomicBool>` on `Shell`
/// because tao's event-loop callback must be `'static` — we cannot
/// borrow from `&self` inside the closure.
static CTRL_C_RECEIVED: AtomicBool = AtomicBool::new(false);

/// `Shell` handle.
///
/// Faz 1.6 Dalga 1b: real `tao::EventLoop` + backend-attached `WebViewWindow`.
/// Constructed without a backend (legacy path) the shell preserves the
/// Faz 1.0 stub behaviour for CI unit tests on headless runners.
pub struct Shell {
    config: ShellConfig,
    tray_menu: TrayMenu,
    resize_observer: ResizeObserver,
    /// WebView backend (`None` → stub mode, used by tests).
    backend: Option<SharedBackend>,
}

impl fmt::Debug for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shell")
            .field("config", &self.config)
            .field("tray_menu", &self.tray_menu)
            .field("resize_observer", &self.resize_observer)
            .field("has_backend", &self.backend.is_some())
            .finish()
    }
}

impl Shell {
    /// Yeni `Shell` instance'ı (stub mode — no backend).
    ///
    /// Faz 1.0 uyumluluğu: `ShellBuilder::build()` bu constructor'ı çağırır
    /// ve `Shell::run()` no-op davranır. Production binary'ler
    /// `ShellBuilder::backend(...)` çağırarak gerçek event loop'u aktive eder.
    #[must_use]
    pub fn new(config: ShellConfig) -> Self {
        Self {
            config,
            tray_menu: default_tray_menu(),
            resize_observer: ResizeObserver::new(),
            backend: None,
        }
    }

    /// Yeni `Shell` instance'ı (real backend — Faz 1.6 Dalga 1b).
    #[must_use]
    pub fn with_backend(config: ShellConfig, backend: SharedBackend) -> Self {
        Self {
            config,
            tray_menu: default_tray_menu(),
            resize_observer: ResizeObserver::new(),
            backend: Some(backend),
        }
    }

    /// Shell konfigürasyonu (read-only).
    #[must_use]
    pub const fn config(&self) -> &ShellConfig {
        &self.config
    }

    /// Tray menü (read-only).
    #[must_use]
    pub const fn tray_menu(&self) -> &TrayMenu {
        &self.tray_menu
    }

    /// Resize observer (read-only).
    #[must_use]
    pub const fn resize_observer(&self) -> &ResizeObserver {
        &self.resize_observer
    }

    /// Backend atanmış mı? (real event loop aktif mi?)
    #[must_use]
    pub const fn has_backend(&self) -> bool {
        self.backend.is_some()
    }

    /// Event loop'u başlat.
    ///
    /// **Stub mode (no backend):** Faz 1.0 uyumlu — sadece konfigürasyon
    /// doğrulaması yapar ve "Shell ready" loglanır. Test/CI runner'larında
    /// GUI loop gerekmediğinden tercih edilir.
    ///
    /// **Real mode (backend set):** `tao::event_loop::EventLoop::new()` +
    /// `backend.create_window(&target, &config)` + `event_loop.run(...)`
    /// ile gerçek native pencere açar ve WebView'i attach eder. Fonksiyon
    /// pencere X ile kapatılana veya Ctrl-C sinyali gelene kadar bloklar
    /// (event loop blocking call). Bu, Faz 1.6 Dalga 1b'nin ana düzeltmesi —
    /// önceki implementasyon sadece log basıp dönüyordu (audit §7.2).
    ///
    /// # Errors
    ///
    /// Stub mode: her zaman `Ok(())`.
    ///
    /// Real mode:
    /// - `tao::EventLoop::new()` başarısız → anyhow error propagate.
    /// - `backend.create_window(...)` platform/runtime hatası
    ///   (WebView2 missing, libcef.dll missing, vb.) → `ViscosError`
    ///   `Media`/`Unimplemented` backend'ten propagate edilir.
    /// - `event_loop.run(...)` Windows API hatası → tao'dan propagate.
    pub fn run(&self) -> anyhow::Result<()> {
        match &self.backend {
            None => self.run_stub(),
            Some(backend) => self.run_event_loop(backend.as_ref()),
        }
    }

    /// Stub davranışı — gerçek event loop yok, sadece log + return.
    fn run_stub(&self) -> anyhow::Result<()> {
        tracing::info!(
            title = %self.config.window.title,
            width = self.config.window.width,
            height = self.config.window.height,
            tray_enabled = self.config.tray_enabled,
            devtools_enabled = self.config.devtools_enabled,
            "Shell ready (Faz 1.0 stub — backend atanmamış, event loop başlatılmadı)"
        );
        Ok(())
    }

    /// Gerçek `tao::EventLoop` + WebView backend wiring.
    ///
    /// Faz 1.6 Dalga 1b ana implementasyonu. Akış:
    ///
    /// 1. `tao::EventLoop::new()` — main-thread affine event loop.
    /// 2. Backend'i kullanarak pencere + WebView oluştur (target = &event_loop).
    /// 3. Ctrl-C handler thread'i spawn et (kendi tokio runtime'ında).
    /// 4. `event_loop.run(|event, _, control| ...)` blokla — pencere
    ///    kapatılana veya Ctrl-C gelene kadar.
    /// 5. Cleanup logla, dön.
    fn run_event_loop(&self, backend: &dyn WebViewBackend) -> anyhow::Result<()> {
        // 1. Event loop oluştur.
        let event_loop = EventLoop::<()>::new();

        // 2. Backend üzerinden pencere + WebView oluştur.
        //    `create_window` target olarak `&event_loop`'u alır; tao::Window
        //    ve WebView'i tek atomik adımda kurar. Bu Faz 1.6'da MVP-1B'nin
        //    tamamlanmış hali (audit §2.2).
        let window = backend
            .create_window(&event_loop, &self.config.window)
            .map_err(|e| {
                anyhow::anyhow!(
                    "webview backend '{}' pencere oluşturamadı: {e}",
                    backend.name()
                )
            })?;

        tracing::info!(
            backend = backend.name(),
            version = backend.version(),
            title = %self.config.window.title,
            width = self.config.window.width,
            height = self.config.window.height,
            tray_enabled = self.config.tray_enabled,
            devtools_enabled = self.config.devtools_enabled,
            initial_url = %self.config.window.initial_url,
            window_id = window.id(),
            "Shell ready (Faz 1.6 Dalga 1b — real tao event loop + WebView)"
        );

        // 3. Ctrl-C listener thread'i spawn et.
        //    tao::EventLoop::run() main thread'i blokladığı için
        //    async signal handler'ı ayrı bir thread'de çalıştırıyoruz.
        //    O thread bir `current_thread` tokio runtime kurar ve
        //    `tokio::signal::ctrl_c().await` ile bekler; sinyal gelince
        //    `CTRL_C_RECEIVED` flag'ini set eder. Ana thread'in event
        //    loop callback'i bu flag'i her iterasyonda kontrol eder.
        spawn_ctrl_c_listener();

        // 4. Event loop'u çalıştır (blocking).
        //
        //    Closure'ın `'static` olması gerekiyor (tao::EventLoop::run
        //    `FnMut(Event, &EventLoopWindowTarget, &mut ControlFlow)` +
        //    `'static`). Bu yüzden `self`'i move yerine closure'a
        //    capture edemiyoruz — bunun yerine sadece flag'leri ve
        //    pencere/webview handle'larını closure içinde taşıyoruz.
        //
        //    NOT: `window` bir `Box<dyn WebViewWindow>`; trait object
        //    Send + Sync gerektiriyor (`unsafe impl` webview2.rs ve
        //    cef.rs'te). Closure thread-local state'de tutulduğu için
        //    Send sorunu yok.
        run_loop(event_loop, window);

        // 5. Cleanup logla.
        tracing::info!("Shell::run event loop exited — graceful shutdown");

        Ok(())
    }
}

/// Ctrl-C listener thread'i spawn et (process-global flag ile haberleşir).
///
/// `tao::EventLoop::run()` main thread'i blokladığı için async
/// `tokio::signal::ctrl_c().await`'ı ayrı bir OS thread'inde çalıştırıyoruz.
/// Sinyal geldiğinde `CTRL_C_RECEIVED` flag'ini set eder; ana thread'in
/// event loop callback'i bu flag'i kontrol eder ve `ControlFlow::Exit`
/// ile loop'u sonlandırır.
///
/// Thread leak: bu thread uygulama ömrü boyunca yaşar (Ctrl-C'yi süresiz
/// dinler). Process exit'te OS tarafından reclaim edilir — sorun değil.
fn spawn_ctrl_c_listener() {
    use std::thread;

    thread::Builder::new()
        .name("viscos-ctrlc".into())
        .spawn(|| {
            // Her ne kadar main thread'de `tokio::runtime::Runtime` kurulu
            // olmasa da (sync main), bu thread kendi runtime'ını kurup
            // async signal'i bekleyebilir. `current_thread` flavor yeterli
            // çünkü yalnızca tek bir future (ctrl_c) poll ediyoruz.
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    tracing::error!(?err, "ctrl-c tokio runtime kurulamadı");
                    return;
                }
            };

            runtime.block_on(async {
                if let Err(err) = tokio::signal::ctrl_c().await {
                    tracing::error!(?err, "ctrl-c handler başarısız");
                    return;
                }
                tracing::warn!("Ctrl-C received (Faz 1.6 event loop path)");
                CTRL_C_RECEIVED.store(true, Ordering::SeqCst);
            });
        })
        .expect("viscos-ctrlc thread spawn başarısız");
}

/// `tao::EventLoop::run` blocking call'unu çalıştır.
///
/// Ayrı fonksiyon olarak tutmamızın sebebi: closure içinde `Box<dyn WebViewWindow>`
/// ve diğer non-`'static` referansları değil, sadece `'static` uyumlu
/// handle'ları kullanabiliyoruz. Fonksiyon argümanları `move` ile
/// transfer edildiğinden borrow checker sorunu yok.
fn run_loop(event_loop: EventLoop<()>, window: Box<dyn WebViewWindow>) {
    let webview_id = window.id();
    event_loop.run(
        move |event, _target: &EventLoopWindowTarget<()>, control: &mut ControlFlow| {
            // Ctrl-C polling — her event'te kontrol et. `Ordering::Relaxed`
            // yeterli: write bir kere yapılıyor ve yalnızca exit tetiklemek
            // için okunuyor (memory order gerektirmeyen bir boolean flag).
            if CTRL_C_RECEIVED.load(Ordering::Relaxed) {
                tracing::info!(window_id = webview_id, "Ctrl-C: event loop exit");
                *control = ControlFlow::Exit;
                return;
            }

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    tracing::info!(window_id = webview_id, "window close requested");
                    // `Box<dyn WebViewWindow>`'un `close()` methodu WebView
                    // dispose eder + tao::Window'u yok eder; gerçek kapatma
                    // `Drop` ile olur. Burada sadece event loop'a exit sinyali
                    // veriyoruz.
                    if let Err(err) = window.close() {
                        tracing::warn!(?err, "WebViewWindow::close hata (devam ediliyor)");
                    }
                    *control = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::Destroyed,
                    ..
                } => {
                    tracing::debug!(window_id = webview_id, "window destroyed");
                    // Pencere destroy edildi → loop'tan çık. (Normal close
                    // path'te CloseRequested önce gelir; bu sadece OOM/crash
                    // gibi beklenmedik durumlar için.)
                    *control = ControlFlow::Exit;
                }
                _ => {
                    // Diğer event'ler (mouse, keyboard, resize, scale_factor
                    // changed, vb.) Faz 5.0 native UI ve Faz 6.0 hotkey'lerde
                    // handle edilecek. Şimdilik consume edip ignore.
                }
            }
        },
    );
}

/// `Shell` builder (fluent API).
#[derive(Default)]
pub struct ShellBuilder {
    config: ShellConfig,
    backend: Option<SharedBackend>,
}

impl fmt::Debug for ShellBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShellBuilder")
            .field("config", &self.config)
            .field("has_backend", &self.backend.is_some())
            .finish()
    }
}

impl ShellBuilder {
    /// Yeni builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Window config set.
    #[must_use]
    pub fn window(mut self, window: WindowConfig) -> Self {
        self.config.window = window;
        self
    }

    /// Tray toggle.
    #[must_use]
    pub const fn tray_enabled(mut self, enabled: bool) -> Self {
        self.config.tray_enabled = enabled;
        self
    }

    /// DevTools toggle.
    #[must_use]
    pub const fn devtools_enabled(mut self, enabled: bool) -> Self {
        self.config.devtools_enabled = enabled;
        self
    }

    /// WebView backend ata (Faz 1.6 Dalga 1b — gerçek event loop).
    ///
    /// Verilen backend `Shell::run()` sırasında `tao::EventLoop` üzerinden
    /// `create_window(...)` çağrısıyla gerçek native pencere + WebView
    /// oluşturur. Backend atanmamış `Shell` stub modunda kalır (test/CI).
    #[must_use]
    pub fn backend(mut self, backend: SharedBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Shell oluştur.
    #[must_use]
    pub fn build(self) -> Shell {
        match self.backend {
            Some(backend) => Shell::with_backend(self.config, backend),
            None => Shell::new(self.config),
        }
    }
}

// (WindowBuilder / Window / WindowEvent / Event / EventLoop types re-exported
// via `tao` at the crate root — direct module paths used above for clarity.)
#[allow(dead_code)]
fn _assert_windowbuilder_in_scope(_: WindowBuilder, _: LogicalSize<u32>) {}
