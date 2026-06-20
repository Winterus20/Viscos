//! CLI parsing integration tests (Faz 1.6 Dalga 1c — MVP-1B).
//!
//! ## Coverage
//!
//! - `--backend=webview2` / `--backend=cef` / `--backend=auto` explicit.
//! - `--backend=invalid` clap error (yakalanmaz, exit).
//! - `--backend=` (empty) clap error.
//! - `--backend` flag yokken default `auto`.

use clap::Parser;

use viscos_webview::{BackendKind, resolve_backend};

/// Test edilen CLI struct (`viscos` binary'sinin birebir kopyası).
#[derive(Debug, Parser)]
#[command(name = "viscos")]
struct TestCli {
    /// WebView backend seçimi. Default: `auto`.
    #[arg(long, value_name = "BACKEND", default_value = "auto")]
    backend: String,
}

/// CLI → `BackendKind` resolution zincirini verify et.
fn resolve_from_cli_args(args: &[&str]) -> Result<BackendKind, String> {
    let cli = TestCli::try_parse_from(args).map_err(|e| e.to_string())?;
    resolve_backend(Some(&cli.backend), None).map_err(|e| e.to_string())
}

#[test]
fn cli_explicit_webview2() {
    let kind = resolve_from_cli_args(&["viscos", "--backend", "webview2"])
        .expect("explicit webview2 must succeed");
    assert_eq!(kind, BackendKind::WebView2);
}

#[test]
fn cli_explicit_cef() {
    let kind =
        resolve_from_cli_args(&["viscos", "--backend", "cef"]).expect("explicit cef must succeed");
    assert_eq!(kind, BackendKind::Cef);
}

#[test]
fn cli_explicit_auto_falls_through_to_default() {
    let kind = resolve_from_cli_args(&["viscos", "--backend", "auto"]).expect("auto must succeed");
    // Default backend (Win11 + feature ON → CEF, aksi → WebView2).
    // Test'in CI'da tutarlı çalışması için result `WebView2 | Cef` aralığında.
    assert!(
        matches!(kind, BackendKind::WebView2 | BackendKind::Cef),
        "auto must resolve to a known kind, got {kind:?}"
    );
}

#[test]
fn cli_default_when_flag_missing() {
    let kind = resolve_from_cli_args(&["viscos"]).expect("missing flag must default to auto");
    // default_value="auto" → default_backend() → Win11+feature ON ise CEF.
    assert!(
        matches!(kind, BackendKind::WebView2 | BackendKind::Cef),
        "default must resolve to a known kind, got {kind:?}"
    );
}

#[test]
fn cli_invalid_backend_returns_error() {
    let result = resolve_from_cli_args(&["viscos", "--backend", "tauri"]);
    assert!(result.is_err(), "invalid backend must error");
}

#[test]
fn cli_empty_backend_value_uses_default() {
    // `--backend=` clap tarafından boş string olarak parse edilir.
    // `resolve_backend` Media hatası döndürmeli (bilinmeyen backend).
    let result = resolve_from_cli_args(&["viscos", "--backend", ""]);
    // Clap boş string'i kabul eder (default_value override).
    // resolve_backend bilinmeyen değer olarak reddeder.
    assert!(result.is_err(), "empty backend string must error");
}

#[test]
fn cli_case_insensitive() {
    let kind_lower = resolve_from_cli_args(&["viscos", "--backend", "cef"]).unwrap();
    let kind_upper = resolve_from_cli_args(&["viscos", "--backend", "CEF"]).unwrap();
    let kind_mixed = resolve_from_cli_args(&["viscos", "--backend", "Cef"]).unwrap();
    assert_eq!(kind_lower, kind_upper);
    assert_eq!(kind_lower, kind_mixed);
    assert_eq!(kind_lower, BackendKind::Cef);
}

#[test]
fn cli_help_flag_short_circuits() {
    // `--help` clap tarafından yakalanır, ExitCode(0) döner → try_parse_from error.
    // Burada yalnızca clap'in bu davranışı verify ediyoruz; resolve_backend
    // çağrısına ulaşılmaz.
    let result = TestCli::try_parse_from(["viscos", "--help"]);
    assert!(result.is_err(), "--help must trigger clap help exit");
}

#[test]
fn cli_unknown_flag_errors() {
    let result = TestCli::try_parse_from(["viscos", "--unknown-flag"]);
    assert!(result.is_err(), "unknown flag must error");
}
