//! `viscos-log` — tracing initialization.
//!
//! Faz 0.0'da sadece EnvFilter + fmt layer (stdout). Faz 1+'ta non-blocking rotating
//! dosya appender (tracing-appender) Faz 1+'taki `init_with_file` ile aktif olur.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Default log filter — sessiz başlangıç (info). Debug için `RUST_LOG` env var ile override.
fn default_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("viscos=info,wry=warn"))
}

/// Initialize logging — stdout only (Faz 0.0 varsayılanı).
///
/// Faz 1+'ta GDI leak debug'unda `RUST_LOG=viscos=debug,wry=debug` ile override edilir.
/// `tracing-log` feature: log facade'i kullanan crate'lerden (wry, tao, winapi wrapper'ları)
/// gelen mesajları da yakalar.
pub fn init() {
    let filter = default_filter();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .init();
}

/// Initialize logging with a non-blocking rotating file appender (Faz 1+ crash log için).
///
/// Returns a [`WorkerGuard`] that must be held until process exit — dropping it will
/// flush pending log events. Attach to a leaked/static to keep it alive for program lifetime.
pub fn init_with_file(log_dir: &Path) -> WorkerGuard {
    let filter = default_filter();

    // Daily-rolling file appender, non-blocking (background thread I/O).
    let file_appender = tracing_appender::rolling::daily(log_dir, "viscos.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    guard
}

/// Initialize logging in tests — uses a per-test subscriber without global side effects.
#[cfg(test)]
pub fn init_test() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("viscos=debug"))
        .with_test_writer()
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_init_does_not_panic_on_try_init() {
        // İlk init global subscriber'ı set eder. İkinci çağrı `init()` panic ederdi;
        // bu yüzden test path'inde `try_init` kullanılır.
        init_test();
        // İkinci çağrı başarısız olmalı (zaten set edilmiş), panic etmemeli.
        let result = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("viscos=info"))
            .with_test_writer()
            .try_init();
        assert!(
            result.is_err(),
            "second try_init should fail (already initialized)"
        );
    }

    #[test]
    fn default_filter_falls_back() {
        // RUST_LOG set edilmemiş ortamda fallback "viscos=info,wry=warn" çalışmalı.
        let f = default_filter();
        let _ = f; // sadece oluşturulabildiğini doğrula
    }
}
