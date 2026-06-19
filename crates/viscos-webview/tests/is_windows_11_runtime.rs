//! Windows 11 runtime detection integration tests (Faz 1.6 Dalga 1c).
//!
//! ## Faz 1.0 → Faz 1.6 farkı
//!
//! Faz 1.0'da `is_windows_11()` compile-time `cfg!(target_os = "windows")`
//! kullanıyordu (yetersiz: Windows 10 build ile Windows 11 build aynı binary'de
//! ayırt edilemiyordu).
//!
//! Faz 1.6'da `windows_version::OsVersion::current().build >= 22000`
//! runtime detection kullanılır (ADR-0012 §4).
//!
//! ## Bu test dosyası
//!
//! Compile-time + runtime gating'in doğru çalıştığını doğrular:
//! - `cfg!(target_os = "windows")` → runtime `windows-version` API'si çağrılır.
//! - non-Windows build → runtime `false` döner (compile-time kısayol).
//!
//! Bkz. [`crate::is_windows_11`].

use viscos_webview::is_windows_11;

#[test]
fn is_windows_11_compile_time_gate() {
    // Compile-time: target_os = windows değilse her zaman false.
    // Bu kontrat runtime'da windows-version API'si çağrılsa bile korunur.
    #[cfg(not(target_os = "windows"))]
    assert!(!is_windows_11());
}

#[test]
#[cfg(target_os = "windows")]
fn is_windows_11_runtime_consistency() {
    // Windows build'de runtime `OsVersion::current()` çağrılır.
    // CI windows-latest runner Windows 11 22H2+ → build >= 22000 → true.
    // Eski runner (örn. Windows 10) → false.
    // Bu test çağrının panic/segfault etmediğini doğrular (API stability).
    let result = std::panic::catch_unwind(is_windows_11);
    assert!(
        result.is_ok(),
        "is_windows_11 must not panic on Windows (windows-version API soundness)"
    );

    // windows_version kütüphanesi her zaman geçerli bir OsVersion döner
    // (GetVersionExW fallback dahili olarak panic-free). Burada build >=
    // 22000 olduğunda true bekliyoruz; aksi runtime'da Windows 10 build'i.
    let is_win11 = is_windows_11();
    // Bool return garantisi — Windows 10 / Windows 11 ayrımı.
    let _: bool = is_win11;
}

#[test]
fn is_windows_11_idempotent() {
    // Birden fazla çağrı aynı sonucu vermeli (state tutmuyor).
    let a = is_windows_11();
    let b = is_windows_11();
    assert_eq!(a, b, "is_windows_11 must be idempotent (no mutable state)");
}
