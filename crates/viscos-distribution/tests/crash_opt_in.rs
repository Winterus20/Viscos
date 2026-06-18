//! Integration test — `CrashReporter` opt-in/opt-out semantics.

use std::path::PathBuf;

use viscos_distribution::{CrashConfig, CrashOptInStatus, CrashReporter};

#[test]
fn default_config_has_opt_in_false_and_empty_url() {
    let cfg = CrashConfig::default();
    assert!(!cfg.opt_in);
    assert!(cfg.reporter_url.is_empty());
}

#[test]
fn reporter_with_empty_url_is_disabled() {
    let reporter = CrashReporter::with_defaults();
    assert_eq!(reporter.opt_in_status(), CrashOptInStatus::Disabled);
}

#[test]
fn reporter_with_url_and_opt_in_is_enabled() {
    let cfg = CrashConfig {
        opt_in: true,
        reporter_url: "https://crash.example.com/ingest".to_string(),
        dump_dir: PathBuf::from("/tmp/viscos-crashes"),
    };
    let reporter = CrashReporter::new(cfg);
    assert_eq!(reporter.opt_in_status(), CrashOptInStatus::Enabled);
}

#[test]
fn reporter_with_url_but_opt_out_is_opted_out() {
    let cfg = CrashConfig {
        opt_in: false,
        reporter_url: "https://crash.example.com/ingest".to_string(),
        dump_dir: PathBuf::from("/tmp/viscos-crashes"),
    };
    let reporter = CrashReporter::new(cfg);
    assert_eq!(reporter.opt_in_status(), CrashOptInStatus::OptedOut);
}

#[test]
fn init_stub_succeeds_for_each_status() {
    for status in [
        CrashOptInStatus::Disabled,
        CrashOptInStatus::Enabled,
        CrashOptInStatus::OptedOut,
    ] {
        let cfg = match status {
            CrashOptInStatus::Disabled => CrashConfig::default(),
            CrashOptInStatus::Enabled => CrashConfig {
                opt_in: true,
                reporter_url: "https://crash.example.com".to_string(),
                dump_dir: PathBuf::from("/tmp/viscos-crashes"),
            },
            CrashOptInStatus::OptedOut => CrashConfig {
                opt_in: false,
                reporter_url: "https://crash.example.com".to_string(),
                dump_dir: PathBuf::from("/tmp/viscos-crashes"),
            },
        };
        let reporter = CrashReporter::new(cfg);
        assert!(
            reporter.init().is_ok(),
            "init stub must succeed for {status:?}"
        );
    }
}

#[test]
fn crash_opt_in_status_default_is_disabled() {
    assert_eq!(
        CrashReporter::with_defaults().opt_in_status(),
        CrashOptInStatus::Disabled
    );
}

#[test]
fn crash_config_preserves_dump_dir() {
    let cfg = CrashConfig {
        opt_in: false,
        reporter_url: String::new(),
        dump_dir: PathBuf::from("/custom/crash/dir"),
    };
    let reporter = CrashReporter::new(cfg);
    assert_eq!(
        reporter.config().dump_dir,
        PathBuf::from("/custom/crash/dir")
    );
}
