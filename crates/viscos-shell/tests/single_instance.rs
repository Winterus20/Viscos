//! Cross-module integration tests — `viscos-shell::integration::single_instance`
//! acquisition + drop semantics.

use viscos_shell::integration::single_instance::SingleInstance;

#[test]
fn acquire_and_drop_allows_reacquire() {
    {
        let _first = SingleInstance::acquire().expect("first acquire");
        // drop sonrası lock serbest.
    }
    let second = SingleInstance::acquire();
    assert!(second.is_ok(), "acquire after drop should succeed");
}

#[test]
fn is_held_returns_true_when_held() {
    let instance = SingleInstance::acquire().expect("acquire");
    assert!(instance.is_held());
}
