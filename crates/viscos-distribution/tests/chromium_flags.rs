//! Integration test — `ChromiumFlags` config loader + deny-list.

use viscos_distribution::{ChromiumFlags, DEFAULT_DENY_FLAGS};

#[test]
fn default_flags_loaded() {
    let flags = ChromiumFlags::default();
    assert!(
        flags
            .flags
            .iter()
            .any(|f| f.contains("msSmartScreenProtection")),
        "default must include msSmartScreenProtection disable flag"
    );
    assert!(flags.validate().is_ok(), "default flags must validate");
}

#[test]
fn load_from_config_returns_default() {
    let flags = ChromiumFlags::load_from_config().expect("load");
    assert!(flags.validate().is_ok());
    assert!(!flags.flags.is_empty());
}

#[test]
fn denied_flag_is_rejected() {
    for denied in DEFAULT_DENY_FLAGS {
        let flags = ChromiumFlags::with_flags(vec![denied.to_string()]);
        let result = flags.validate();
        assert!(result.is_err(), "denied flag must be rejected: {denied}");
    }
}

#[test]
fn disable_web_security_is_in_deny_list() {
    let deny: std::collections::HashSet<&str> = DEFAULT_DENY_FLAGS.iter().copied().collect();
    assert!(deny.contains("--disable-web-security"));
    assert!(deny.contains("--single-process"));
    assert!(deny.contains("--disable-gpu"));
    assert!(deny.contains("--no-sandbox"));
}

#[test]
fn invalid_flag_format_rejected() {
    let flags = ChromiumFlags::with_flags(vec!["no-dashes-flag".to_string()]);
    let result = flags.validate();
    assert!(result.is_err(), "flag without -- prefix must be rejected");
}

#[test]
fn custom_legitimate_flags_pass_validation() {
    let flags = ChromiumFlags::with_flags(vec![
        "--disable-features=Translate".to_string(),
        "--disable-background-networking".to_string(),
        "--no-first-run".to_string(),
    ]);
    assert!(flags.validate().is_ok());
}

#[test]
fn as_args_returns_string_slices() {
    let flags = ChromiumFlags::with_flags(vec![
        "--disable-features=X".to_string(),
        "--no-first-run".to_string(),
    ]);
    let args = flags.as_args();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "--disable-features=X");
    assert_eq!(args[1], "--no-first-run");
}

#[test]
fn empty_flags_list_validates() {
    let flags = ChromiumFlags::with_flags(vec![]);
    assert!(flags.validate().is_ok());
}
