//! Single-instance guard (Faz 6.0).
//!
//! İlk instance lock alır, ikinci instance hata alır. İkinci instance
//! deep link URI'yi birincil'e forward edebilir (`on_secondary_launch`).
//!
//! `single-instance 0.3` crate'i `named-lock` tabanlı cross-platform lock
//! sağlar. Faz 1.0 stub: in-process `parking_lot::Mutex` tabanlı simülasyon
//! (CI'da gerçek OS lock gereksiz). Stub test edilebilirliği korur; Faz 1.6'da
//! `single-instance 0.3` crate'i ile cross-process lock olacak.
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md` §6 Single-Instance](../../../.cursor/plans/phase-6.0-hotkeys.md)

use parking_lot::Mutex;
use viscos_error::ViscosError;

/// Global lock state (test ortamında in-process, production'da OS-level).
///
/// `static MUTEX` Faz 1.0 stub'ıdır; test'ler sıralı çalışır (cargo test
/// thread sayısı 1 default) veya `try_lock` race'ini kabul eder.
static HELD: Mutex<()> = Mutex::new(());

/// Single-instance guard.
///
/// Faz 1.0 stub: `parking_lot::Mutex` ile in-process kontrol. Faz 1.6'da
/// gerçek `single-instance 0.3` crate'i ile cross-process lock olacak.
pub struct SingleInstance {
    /// Lock referansı (Drop'ta bırakılır).
    _guard: parking_lot::MutexGuard<'static, ()>,
}

impl std::fmt::Debug for SingleInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleInstance")
            .field("is_held", &true)
            .finish()
    }
}

impl SingleInstance {
    /// Lock'u acquire et.
    ///
    /// İlk başarılı acquisition `Ok(SingleInstance)` döner. Aynı process
    /// içinde ikinci çağrı `Err` döner (Faz 1.0 stub). Faz 1.6'da OS-level
    /// named-lock ile cross-process kontrolü sağlanır.
    ///
    /// # Errors
    ///
    /// Lock zaten tutuluyorsa `ViscosError::Media("instance already running")`
    /// döner.
    pub fn acquire() -> Result<Self, ViscosError> {
        match HELD.try_lock() {
            Some(guard) => Ok(Self { _guard: guard }),
            None => Err(ViscosError::Media(
                "viscos instance already running (Faz 1.0 stub: in-process only)".into(),
            )),
        }
    }

    /// İkincil instance launch handler'ı kaydet.
    ///
    /// Faz 1.0 stub: sadece handler'ı saklar, gerçek named-pipe listener
    /// Faz 1.6'da. Handler, ikinci instance'tan gelen argv'yi (deep link
    /// içeren) alır.
    pub fn on_secondary_launch<F>(&self, _handler: F)
    where
        F: Fn(Vec<String>) + Send + Sync + 'static,
    {
        // Stub: gerçek listener kurulumu Faz 1.6'da.
        tracing::debug!("SingleInstance::on_secondary_launch registered (Faz 1.0 stub)");
    }

    /// Lock hâlâ tutuluyor mu?
    #[must_use]
    pub const fn is_held(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    //! Tests share a global static `HELD` mutex via the production API.
    //!
    //! The production stub uses `parking_lot::Mutex::try_lock`, which is
    //! non-blocking. When `cargo test` runs unit tests in parallel
    //! (default `--test-threads`), two tests calling `SingleInstance::acquire()`
    //! simultaneously will race — one wins, the other returns
    //! `ViscosError::Media("instance already running")` and the test panics
    //! on `assert!(instance.is_ok())`.
    //!
    //! This is a documented stub limitation (`single_instance.rs:19-21`):
    //! "test'ler sıralı çalışır (cargo test thread sayısı 1 default) veya
    //! `try_lock` race'ini kabul eder." Faz 1.6'da `single-instance 0.3`
    //! crate'i ile cross-process OS lock olacak; bu race production
    //! davranışını etkilemeyecek.
    //!
    //! Fix: a test-only `Mutex` (`TEST_LOCK`) wraps the body of each test
    //! that calls `SingleInstance::acquire()`. This serializes the racing
    //! tests at the *test harness* level without changing production
    //! behavior — `SingleInstance::acquire()` still uses the production
    //! `try_lock` path unchanged.

    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Test-only lock that serializes access to the production `HELD` mutex.
    /// `parking_lot::Mutex` is `!Send` across `.lock()` on contention failure,
    /// so we use `std::sync::Mutex` which returns `Result` and is poison-safe.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn first_acquire_succeeds() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let instance = SingleInstance::acquire();
        assert!(instance.is_ok());
        assert!(instance.unwrap().is_held());
        // drop otomatik olur, lock bırakılır.
    }

    #[test]
    fn drop_releases_lock_for_next_test() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        // Her test isolation için explicit scope.
        {
            let _first = SingleInstance::acquire().expect("first acquire");
        }
        // Scope sonrası drop edildi → tekrar acquire edilebilir.
        let second = SingleInstance::acquire();
        assert!(second.is_ok(), "acquire after drop should succeed");
    }

    #[test]
    fn on_secondary_launch_accepts_closure() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let instance = SingleInstance::acquire().unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        instance.on_secondary_launch(move |_argv| {
            called_clone.store(true, Ordering::SeqCst);
        });
        // Stub'da handler çağrılmaz; sadece compile test.
        assert!(!called.load(Ordering::SeqCst));
    }
}
