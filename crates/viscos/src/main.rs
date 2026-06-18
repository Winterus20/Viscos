//! Viscos binary entry point (Faz 1.0 — Shell+WebView+IPC+Watchdog).
//!
//! Faz 1.0 akışı:
//! 1. tracing init (stdout + default filter)
//! 2. config load (`config/default.toml` → `config/local.toml` → env)
//! 3. WebView backend seçimi (`select_default_backend`)
//! 4. IPC router bootstrap (default StubHandler — Faz 2+'da gerçek)
//! 5. Watchdog background task başlat (GDI counter)
//! 6. Shell.run() — pencere + tray stub loglanır (gerçek event loop Faz 1.6)
//! 7. "Viscos ready" loglanır
//! 8. Ctrl-C sinyali bekle → graceful shutdown

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::signal;
use tracing::{info, warn};

use viscos_config::Config;
use viscos_core::VISCOS_VERSION;
use viscos_ipc::DefaultIpcRouter;
use viscos_shell::ShellBuilder;
use viscos_watchdog::{StubAutosave, Watchdog, WatchdogConfig};
use viscos_webview::{BackendKind, select_default_backend};

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
    // 1. Logging — önce init et ki aşağıdaki log'lar yakalansın.
    viscos_log::init();

    info!(
        version = VISCOS_VERSION,
        "Viscos starting up (Faz 1.0 — Shell+WebView+IPC+Watchdog)"
    );

    // 2. Config — layered load (default → local → env override).
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

    // 3. WebView backend seçimi.
    let backend = select_backend_from_config(&config.webview.backend);
    info!(
        backend = backend.as_str(),
        "WebView backend selected (Faz 1.0 stub — runtime attachment in Faz 1.6)"
    );

    // 4. IPC router bootstrap.
    let _router = DefaultIpcRouter::new();
    info!("IPC router ready (default StubHandler — Faz 2+'da gerçek handler'lar)");

    // 5. Watchdog background task.
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

    // 6. Shell.run() — pencere + tray stub (gerçek event loop Faz 1.6).
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

    // 7. "Viscos ready" log.
    info!(
        backend = backend.as_str(),
        window_title = %shell.config().window.title,
        tray_items = shell.tray_menu().items().len(),
        "Viscos ready — Ctrl-C ile graceful shutdown"
    );

    // 8. Graceful shutdown — Ctrl-C veya SIGTERM (Windows'ta sadece Ctrl-C).
    wait_for_shutdown_signal().await?;
    info!("shutdown signal received");

    Ok(())
}

/// Config'ten backend seç.
///
/// `auto` → `select_default_backend()` (Win11 → CEF, Win10 → WebView2).
/// `webview2` veya `cef` → explicit override.
fn select_backend_from_config(setting: &str) -> BackendKind {
    match setting.trim().to_ascii_lowercase().as_str() {
        "webview2" => BackendKind::WebView2,
        "cef" => BackendKind::Cef,
        // "auto" veya bilinmeyen → platform default.
        _ => select_default_backend(),
    }
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
    fn select_backend_from_config_explicit_webview2() {
        assert_eq!(
            select_backend_from_config("webview2"),
            BackendKind::WebView2
        );
    }

    #[test]
    fn select_backend_from_config_explicit_cef() {
        assert_eq!(select_backend_from_config("cef"), BackendKind::Cef);
    }

    #[test]
    fn select_backend_from_config_auto_uses_default() {
        let result = select_backend_from_config("auto");
        // Auto, platform default ile aynı olmalı.
        assert_eq!(result, select_default_backend());
    }

    #[test]
    fn select_backend_from_config_unknown_falls_back_to_default() {
        let result = select_backend_from_config("bogus-backend");
        assert_eq!(result, select_default_backend());
    }

    #[test]
    fn select_backend_from_config_case_insensitive() {
        assert_eq!(
            select_backend_from_config("WEBVIEW2"),
            BackendKind::WebView2
        );
        assert_eq!(select_backend_from_config("Cef"), BackendKind::Cef);
    }

    #[tokio::test]
    async fn shutdown_signal_handler_compiles() {
        let _f: fn() -> _ = wait_for_shutdown_signal;
    }
}
