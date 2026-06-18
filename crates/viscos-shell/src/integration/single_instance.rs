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
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn first_acquire_succeeds() {
        let instance = SingleInstance::acquire();
        assert!(instance.is_ok());
        assert!(instance.unwrap().is_held());
        // drop otomatik olur, lock bırakılır.
    }

    #[test]
    fn drop_releases_lock_for_next_test() {
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
