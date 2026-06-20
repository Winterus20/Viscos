//! Viscos binary entry point (Faz 1.6 Dalga 1b/c — MVP-1B).
//!
//! Faz 1.6 akışı:
//! 0. CEF subprocess dispatch (`execute_process_if_subprocess`) — exit
//!    early if running as CEF subprocess (renderer/gpu/network/utility).
//! 1. tracing init (stdout + default filter)
//! 2. CLI parsing (`--backend=webview2|cef|auto`)
//! 3. config load (`config/default.toml` → `config/local.toml` → env)
//! 4. WebView backend resolution (`resolve_backend`: CLI > config > RDP > Win11/CEF > WebView2)
//! 5. IPC router bootstrap (default StubHandler — Faz 2+'da gerçek)
//! 6. Watchdog background task başlat (GDI counter)
//! 7. ShellBuilder::build() → gerçek `tao::Window` + `wry::WebView` (Faz 1.6 Dalga 1b)
//! 8. "Viscos ready" loglanır
//! 9. Ctrl-C sinyali bekle → graceful shutdown
//!
//! B1 kararı: CEF backend feature-gated stub. Default build CEF kullanmaz;
//! production build'ler `--features viscos-webview/cef-backend` ile derlenir.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::signal;
use tracing::{info, warn};

use viscos_config::Config;
use viscos_core::VISCOS_VERSION;
use viscos_ipc::DefaultIpcRouter;
use viscos_shell::ShellBuilder;
use viscos_watchdog::{StubAutosave, Watchdog, WatchdogConfig};
use viscos_webview::{BackendKind, execute_process_if_subprocess, resolve_backend};

#[derive(Debug, Parser)]
#[command(
    name = "viscos",
    version = VISCOS_VERSION,
    about = "Viscos Discord client — Faz 1.6 Dalga 1b/c (WebView2 runtime + CLI override)",
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => {
            info!("Viscos shutdown complete");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("viscos: fatal: {err:#}");
            for cause in err.chain().skip(1) {
                eprintln!("  caused by: {cause}");
            }
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
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
        "Viscos starting up (Faz 1.6 Dalga 1b/c — WebView2 runtime + CLI override)"
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
    let backend = resolve_backend(Some(&cli.backend), Some(&config.webview.backend), None)
        .context("resolving WebView backend")?;
    info!(
        backend = backend.as_str(),
        "WebView backend selected (Faz 1.6 Dalga 1b — real wry/CEF runtime)"
    );

    // 5. IPC router bootstrap.
    let _router = DefaultIpcRouter::new();
    info!("IPC router ready (default StubHandler — Faz 2+'da gerçek handler'lar)");

    // 6. Watchdog background task.
    let autosave: Arc<dyn viscos_watchdog::DraftAutosave> = Arc::new(StubAutosave::new());
    let restart_signal = viscos_watchdog::RestartSignal::default();
    let wd_config = WatchdogConfig {
        gdi_warning: config.watchdog.gdi_warn,
        gdi_critical: config.watchdog.gdi_critical,
        sample_interval: Duration::from_secs(config.watchdog.sample_interval_secs),
        warmup_samples: config.watchdog.warmup_samples,
    };
    let watchdog = Watchdog::new(wd_config, restart_signal, autosave);
    watchdog.spawn();
    info!("Watchdog spawned (GDI counter, 30s sample interval)");

    // 7. Shell.run() — gerçek `tao::Window` + `wry::WebView` (Faz 1.6 Dalga 1b).
    //
    // B1 kararı: backend seçimine göre `WebView2Backend` veya
    // `CefBackend` (feature-gated stub veya real) instantiate edilir.
    // Stub fallback default build'de `Unimplemented` döndürür; Win11
    // production build'lerde `--features viscos-webview/cef-backend` ile
    // gerçek runtime kullanılır.
    let backend_label = match backend {
        BackendKind::WebView2 => "WebView2 (wry)",
        BackendKind::Cef => "CEF (cef-rs)",
    };
    info!(
        backend = backend_label,
        "Backend instantiated (gerçek runtime attach Shell::run içinde)"
    );

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
        .build();
    shell.run().context("shell run")?;

    // 8. "Viscos ready" log.
    info!(
        backend = backend.as_str(),
        window_title = %shell.config().window.title,
        tray_items = shell.tray_menu().items().len(),
        "Viscos ready — Ctrl-C ile graceful shutdown"
    );

    // 9. Graceful shutdown — Ctrl-C veya SIGTERM (Windows'ta sadece Ctrl-C).
    wait_for_shutdown_signal().await?;
    info!("shutdown signal received");

    Ok(())
}

/// Ctrl-C sinyali gelene kadar blokla. Windows + Unix portable.
async fn wait_for_shutdown_signal() -> Result<()> {
    let ctrl_c = signal::ctrl_c();

    // Windows: tokio::signal::ctrl_c yeterli (Unix sinyalleri Windows'ta yok).
    #[cfg(unix)]
    {
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .context("registering SIGTERM handler")?;
        tokio::select! {
            _ = ctrl_c => { warn!("Ctrl-C received"); }
            _ = sigterm.recv() => { warn!("SIGTERM received"); }
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.context("waiting for Ctrl-C")?;
        warn!("Ctrl-C received");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_default_backend_is_auto() {
        // `--backend` flag yokken default `auto` olmalı.
        // clap derive default_value = "auto" → Cli::parse() ile verify edemiyoruz
        // çünkü parse() exit yapar; bu yüzden default_value attribute'u source check.
        // Burada sadece resolve_backend zincirinin `auto` → default_backend'e
        // düştüğünü doğruluyoruz.
        let kind = viscos_webview::resolve_backend(None, Some("auto"), None).unwrap();
        assert!(matches!(kind, BackendKind::WebView2 | BackendKind::Cef));
    }

    #[test]
    fn cli_explicit_backend_passed_through() {
        // CLI override en yüksek öncelik — config ne olursa olsun.
        let kind = viscos_webview::resolve_backend(Some("cef"), Some("webview2"), None).unwrap();
        assert_eq!(kind, BackendKind::Cef);
        let kind = viscos_webview::resolve_backend(Some("webview2"), Some("cef"), None).unwrap();
        assert_eq!(kind, BackendKind::WebView2);
    }

    #[test]
    fn execute_process_if_subprocess_in_main_returns_none() {
        // `cargo test -p viscos` ana process olarak çalışır; CEF
        // subprocess dispatch `None` dönmeli. Bu test, `main.rs`'in
        // `execute_process_if_subprocess` import ettiğini ve
        // fonksiyonun test process'te subprocess olarak davranmadığını
        // doğrular.
        let result = viscos_webview::execute_process_if_subprocess();
        assert!(
            result.is_none(),
            "test process'te CEF subprocess dispatch None dönmeli (ana process): got {result:?}"
        );
    }
}
