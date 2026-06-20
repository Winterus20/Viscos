//! Cross-module integration tests — `viscos-shell::integration::single_instance`
//! acquisition + drop semantics.
//!
//! See the `tests` module in `single_instance.rs` for the rationale on why
//! each test acquires `TEST_LOCK` before calling `SingleInstance::acquire()`:
//! the production stub uses non-blocking `try_lock` on a static, so parallel
//! cargo test threads would otherwise race the same lock and flake.

use std::sync::Mutex;
use viscos_shell::integration::single_instance::SingleInstance;

/// Test-only lock that serializes tests against the shared production
/// `HELD` static. Mirror of the module-level `TEST_LOCK` in
/// `single_instance.rs`; integration tests run in a separate binary and
/// therefore need their own instance.
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn acquire_and_drop_allows_reacquire() {
    let _guard = TEST_LOCK.lock().expect("test lock poisoned");
    {
        let _first = SingleInstance::acquire().expect("first acquire");
        // drop sonrası lock serbest.
    }
    let second = SingleInstance::acquire();
    assert!(second.is_ok(), "acquire after drop should succeed");
}

#[test]
fn is_held_returns_true_when_held() {
    let _guard = TEST_LOCK.lock().expect("test lock poisoned");
    let instance = SingleInstance::acquire().expect("acquire");
    assert!(instance.is_held());
}
