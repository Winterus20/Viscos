//! 24h shadow mode integration test (Faz 1.5 preview, Faz 2.0 stub).

use std::time::{Duration, SystemTime};

use viscos_auth::shadow_mode::ShadowMode;

#[test]
fn freshly_armed_is_active() {
    let sm = ShadowMode::new(SystemTime::now());
    assert!(sm.is_active());
    assert!(!sm.allows_write());
}

#[test]
fn disabled_shadow_allows_write() {
    let sm = ShadowMode::disabled();
    assert!(!sm.is_active());
    assert!(sm.allows_write());
}

#[test]
fn remaining_time_decreases() {
    // 23 saat önce login → ~1 saat kalmış olmalı.
    let now = SystemTime::now();
    let past = now
        .checked_sub(Duration::from_secs(23 * 3600))
        .unwrap_or(now);
    let sm = ShadowMode::new(past);
    assert!(sm.is_active());
    let remaining = sm.remaining().expect("remaining in 23h window");
    // 1 saatlik pencere, üst-alt sınır dahilinde (60s tolerans).
    assert!(
        remaining.as_secs() <= 3600 + 60 && remaining.as_secs() >= 3600 - 60,
        "remaining ~1h: {:?}",
        remaining
    );
}

#[test]
fn expired_shadow_allows_write() {
    // 25 saat önce login → shadow süresi dolmuş.
    let now = SystemTime::now();
    let past = now
        .checked_sub(Duration::from_secs(25 * 3600))
        .unwrap_or(now);
    let sm = ShadowMode::new(past);
    assert!(!sm.is_active());
    assert!(sm.allows_write());
    assert!(sm.remaining().is_none());
}
