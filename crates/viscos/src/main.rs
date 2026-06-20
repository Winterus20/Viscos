//! Viscos binary entry point (Faz 1.6 Dalga 1b — MVP-1B).
//!
//! Faz 1.6 akışı:
//! 0. CEF subprocess dispatch (`execute_process_if_subprocess`) — exit
//!    early if running as CEF subprocess (renderer/gpu/network/utility).
//! 1. tracing init (stdout + default filter)
//! 2. CLI parsing (`--backend=webview2|cef|auto`)
//! 3. config load (`config/default.toml` → `config/local.toml` → env)
//! 4. WebView backend resolution (`resolve_backend`: CLI > config > RDP > Win11/CEF > WebView2)
//! 5. IPC router bootstrap (default StubHandler — Faz 2+'da gerçek)
//! 6. Watchdog background task başlat (GDI counter) — kendi thread'inde
//!    tokio runtime ile (çünkü main thread `tao::EventLoop::run` bloklar)
//! 7. ShellBuilder::backend(...) → gerçek `tao::Window` + `wry::WebView`
//!    (Faz 1.6 Dalga 1b)
//! 8. "Viscos ready" loglanır
//! 9. `Shell::run()` event loop'u blokla — pencere X ile kapatılınca veya
//!    Ctrl-C sinyali gelince döner
//!
//! ## Neden sync main?
//!
//! `tao::EventLoop::run()` Windows'ta **main thread'de blocking** olarak
//! çağrılmalıdır (tao::Window ve WebView2 COM nesneleri main-thread affine).
//! Bu yüzden `#[tokio::main]` async main kullanamayız — `event_loop.run()`
//! zaten ana thread'i blokluyor, async runtime ile çakışıyor. Watchdog +
//! Ctrl-C listener kendi thread'lerinde kendi tokio runtime'larına sahip.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

use viscos_config::Config;
use viscos_core::VISCOS_VERSION;
use viscos_ipc::DefaultIpcRouter;
use viscos_shell::ShellBuilder;
use viscos_watchdog::{StubAutosave, Watchdog, WatchdogConfig};
use viscos_webview::{
    BackendKind, CefBackend, WebView2Backend, execute_process_if_subprocess, resolve_backend,
};

#[derive(Debug, Parser)]
#[command(
    name = "viscos",
    version = VISCOS_VERSION,
    about = "Viscos Discord client — Faz 1.6 Dalga 1b (real tao event loop + WebView)",
    long_about = None,
)]
struct Cli {
    /// WebView backend seçimi. Default: `auto` (platform + RDP detection).
    ///
    /// Değerler:
    /// - `webview2`: Microsoft Edge WebView2 (default Win10).
    /// - `cef`: Chromium Embedded Framework (default Win11; feature-gated stub).
    /// - `auto`: platform + RDP detection (CLI yokken default davranış).
    #[arg(long, value_name = "BACKEND", default_value = "auto")]
    backend: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            info!("Viscos shutdown complete");
            ExitCode::SUCCESS
        }
        Err(err) => {
            // Fatal startup hatası: tracing henüz kurulu olmayabilir veya
            // shutdown sırasında olabilir. Stderr'e yazıp çık.
            // (`.cursorrules` Bölüm 5 "üretim kodunda eprintln! YASAK" — bu
            // sadece fatal-exit path için geçerli; logging infrastructure
            // kullanılamadığı durumda stderr tek output mechanism.)
            eprintln!("viscos: fatal: {err:#}");
            for cause in err.chain().skip(1) {
                eprintln!("  caused by: {cause}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    // 0. CEF subprocess dispatch — `cef::execute_process` **her** process'te
    //    ana thread'de `cef::initialize`'dan **önce** çağrılmalıdır (CEF
    //    protokolü, `_cef_main_args_t` standardı). CEF subprocess'leri
    //    (renderer/gpu/network/utility) browser process tarafından
    //    `--type=` command-line flag'i ile spawn edilir; bu fonksiyon
    //    flag'i parse edip subprocess ise `Some(exit_code)` döner.
    //    Browser process'te `None` döner → normal initialization'a devam.
    //
    //    Bu çağrı tracing init'ten **önce** yapılır: subprocess
    //    terminate edilecekse logging overhead'i gereksiz.
    //
    //    Feature kapalıyken (`cef-backend` off) stub: `None` → atlanır.
    if let Some(exit_code) = execute_process_if_subprocess() {
        // CEF subprocess kendi yaşam döngüsünü tamamladı, exit code propagate.
        // `std::process::exit` cleanup handler'larını atlar; CEF subprocess
        // için kabul edilen pattern (C++ CefExecuteProcess örnekleriyle uyumlu).
        std::process::exit(exit_code);
    }

    // 1. Logging — önce init et ki aşağıdaki log'lar yakalansın.
    viscos_log::init();

    info!(
        version = VISCOS_VERSION,
        "Viscos starting up (Faz 1.6 Dalga 1b — real tao event loop + WebView)"
    );

    // 2. CLI parsing.
    let cli = Cli::parse();
    info!(
        backend_override = %cli.backend,
        "CLI parsed"
    );

    // 3. Config — layered load (default → local → env override).
    let config = Config::load().context("loading viscos configuration")?;
    info!(
        app = %config.app.name,
        data_dir = %config.app.data_dir,
        log_level = %config.logging.level,
        window = format!("{}x{} ({})", config.window.width, config.window.height, config.window.title),
        webview_backend = %config.webview.backend,
        watchdog_critical = config.watchdog.gdi_critical,
        "config loaded"
    );

    // 4. WebView backend seçimi (CLI > config > RDP > Win11/CEF > WebView2).
    let backend_kind = resolve_backend(Some(&cli.backend), Some(&config.webview.backend), None)
        .context("resolving WebView backend")?;
    info!(
        backend = backend_kind.as_str(),
        "WebView backend selected (Faz 1.6 Dalga 1b — real wry/CEF runtime)"
    );

    // 5. IPC router bootstrap.
    let _router = DefaultIpcRouter::new();
    info!("IPC router ready (default StubHandler — Faz 2+'da gerçek handler'lar)");

    // 6. Watchdog background task — kendi thread'inde, kendi tokio runtime'ı
    //    ile. Çünkü `tao::EventLoop::run()` main thread'i blokluyor;
    //    `#[tokio::main]` async main kullanamıyoruz.
    //
    //    Watchdog `tokio::spawn` + `tokio::time::interval` kullanıyor; bu
    //    yüzden ayrı bir `current_thread` runtime'a ihtiyacı var.
    //    Thread uygulama ömrü boyunca yaşar (process exit'te OS reclaim).
    spawn_watchdog(&config);

    // 7. Backend instantiate et ve ShellBuilder'a bağla.
    //
    //    B1 kararı: backend seçimine göre `WebView2Backend` veya
    //    `CefBackend` (feature-gated stub veya real) instantiate edilir.
    //    `SharedBackend = Arc<dyn WebViewBackend>` trait object — Faz 1.6
    //    Dalga 1b'nin gerçek event loop'u bu handle üzerinden pencere
    //    oluşturur.
    let backend: Arc<dyn viscos_webview::WebViewBackend> = match backend_kind {
        BackendKind::WebView2 => Arc::new(WebView2Backend::new()),
        BackendKind::Cef => Arc::new(CefBackend::new()),
    };
    info!(
        backend = backend.name(),
        version = backend.version(),
        "Backend instantiated (gerçek runtime attach Shell::run içinde)"
    );

    // 8. Shell oluştur (backend bağlı — gerçek event loop).
    let shell = ShellBuilder::new()
        .window(viscos_webview::WindowConfig {
            title: config.window.title.clone(),
            width: config.window.width,
            height: config.window.height,
            theme: config.window.theme.clone(),
            initial_url: config.window.initial_url.clone(),
        })
        .tray_enabled(config.window.tray_enabled)
        .devtools_enabled(config.window.devtools_enabled)
        .backend(backend)
        .build();

    info!(
        backend = backend_kind.as_str(),
        window_title = %shell.config().window.title,
        tray_items = shell.tray_menu().items().len(),
        has_backend = shell.has_backend(),
        "Viscos ready — pencere X ile veya Ctrl-C ile kapatılabilir"
    );

    // 9. Shell::run() blocking call — tao::EventLoop::run() main thread'i
    //    bloklar, pencere X ile kapatılana veya Ctrl-C sinyali gelene
    //    kadar. Bu fonksiyon döndüğünde event loop sonlanmıştır →
    //    process temiz shutdown.
    shell.run().context("shell run")?;

    Ok(())
}

/// Watchdog'u ayrı bir OS thread'inde, kendi tokio runtime'ı ile spawn et.
///
/// `tao::EventLoop::run()` main thread'i blokladığı için watchdog'u
/// ayrı bir thread'de çalıştırıyoruz. Thread `current_thread` flavor
/// tokio runtime kurup `watchdog.spawn()` çağırır; runtime'ı yaşatmak
/// için `block_on(pending)` ile blokluyoruz (process exit'e kadar).
///
/// Watchdog'un `spawn()` method'u `tokio::spawn` çağırır — bu yüzden
/// `runtime.enter()` ile runtime context'i thread-local olarak set
/// etmemiz gerekiyor. Aksi halde "no reactor running" panic'i alırız.
fn spawn_watchdog(config: &viscos_config::Config) {
    let autosave: Arc<dyn viscos_watchdog::DraftAutosave> = Arc::new(StubAutosave::new());
    let restart_signal = viscos_watchdog::RestartSignal::default();
    let wd_config = WatchdogConfig {
        gdi_warning: config.watchdog.gdi_warn,
        gdi_critical: config.watchdog.gdi_critical,
        sample_interval: Duration::from_secs(config.watchdog.sample_interval_secs),
        warmup_samples: config.watchdog.warmup_samples,
    };
    let watchdog = Watchdog::new(wd_config, restart_signal, autosave);

    let build_result = std::thread::Builder::new()
        .name("viscos-watchdog".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    tracing::error!(?err, "watchdog tokio runtime kurulamadı");
                    return;
                }
            };

            // Runtime context'i thread-local olarak set et. Bu olmadan
            // `watchdog.spawn()` içindeki `tokio::spawn` panic atar.
            // `Runtime::enter` bir guard döner; guard scope'tan düşünce
            // context otomatik olarak restore edilir.
            let _guard = runtime.enter();

            // Watchdog task'i `tokio::spawn` ile background'a atılır;
            // artık `runtime.block_on(pending)` ile runtime'ı yaşatıyoruz
            // (process exit'e kadar blokla).
            watchdog.spawn();

            runtime.block_on(std::future::pending::<()>());
        });

    match build_result {
        Ok(_) => info!(
            interval_secs = config.watchdog.sample_interval_secs,
            gdi_warn = config.watchdog.gdi_warn,
            gdi_critical = config.watchdog.gdi_critical,
            "Watchdog spawned (GDI counter, kendi thread + current_thread runtime)"
        ),
        Err(err) => tracing::error!(?err, "watchdog thread spawn başarısız"),
    }
}

#[cfg(test)]
mod tests {
    use viscos_webview::BackendKind;

    #[test]
    fn cli_default_backend_is_auto() {
        let kind = viscos_webview::resolve_backend(None, Some("auto"), None).unwrap();
        assert!(matches!(kind, BackendKind::WebView2 | BackendKind::Cef));
    }

    #[test]
    fn cli_explicit_backend_passed_through() {
        let kind = viscos_webview::resolve_backend(Some("cef"), Some("webview2"), None).unwrap();
        assert_eq!(kind, BackendKind::Cef);
        let kind = viscos_webview::resolve_backend(Some("webview2"), Some("cef"), None).unwrap();
        assert_eq!(kind, BackendKind::WebView2);
    }

    #[test]
    fn execute_process_if_subprocess_in_main_returns_none() {
        let result = viscos_webview::execute_process_if_subprocess();
        assert!(
            result.is_none(),
            "test process'te CEF subprocess dispatch None dönmeli (ana process): got {result:?}"
        );
    }
}
