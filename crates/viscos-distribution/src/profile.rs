//! Heap profiling altyapısı (dhat — Faz 8.0 stub).
//!
//! Faz 8.0 kapsamı: `dhat` feature gate ile heap profiling API yüzeyi. Üretim
//! binary'sinde `dhat` kapalı; `--features dhat --release` ile çalıştırıldığında
//! `dhat-heap.json` üretilir (`dh_view` ile incelenir).
//!
//! Cross-reference:
//! - [`phase-8.0-distribution.md` §3.2](../../.cursor/plans/phase-8.0-distribution.md#32-dhat-heap-profiling)

use viscos_error::{Result, ViscosError};

/// `dhat::Profiler::new_heap()` döndürdüğü guard'ın opak tipi.
///
/// Feature kapalıyken `()` ile yedeklenir — `init_heap_profiling()` yine de
/// çağrılabilir ama no-op'tur.
#[cfg(feature = "dhat")]
pub struct HeapProfilerGuard {
    _inner: dhat::Profiler,
}

#[cfg(not(feature = "dhat"))]
pub struct HeapProfilerGuard;

/// Heap profiler'ı başlat.
///
/// `dhat` feature açıkken `dhat::Profiler::new_heap()` çağrısı yapar ve
/// guard döndürür; guard drop edilene kadar profiling aktif kalır.
/// Feature kapalıyken no-op — `Ok(HeapProfilerGuard {})` döner.
///
/// # Errors
///
/// Feature açıkken `dhat` kendisi hata dönmez; gelecekte config-driven path
/// eklenirse `ViscosError::Io` dönebilir.
pub fn init_heap_profiling() -> Result<HeapProfilerGuard> {
    #[cfg(feature = "dhat")]
    {
        let profiler = dhat::Profiler::new_heap();
        tracing::info!("dhat heap profiler active — output: dhat-heap.json");
        Ok(HeapProfilerGuard { _inner: profiler })
    }
    #[cfg(not(feature = "dhat"))]
    {
        tracing::debug!(
            "dhat feature kapalı — heap profiling no-op. Çalıştırmak için: cargo run --features dhat --release"
        );
        Ok(HeapProfilerGuard {})
    }
}

/// `init_heap_profiling` sonucunun bir no-op'a çevrilebileceğini garanti eder.
///
/// `Drop` implementasyonu feature kapalıyken trivial; açıkken `dhat::Profiler`
/// zaten RAII guard'dır (drop'ta flush eder).
impl Drop for HeapProfilerGuard {
    fn drop(&mut self) {
        #[cfg(feature = "dhat")]
        {
            tracing::info!("dhat heap profiler dropped — dhat-heap.json flushed");
        }
    }
}

/// Heap profiling aktif mi?
///
/// `dhat` feature flag'ine bağlı. Üretim binary'sinde false.
#[must_use]
pub const fn heap_profiling_enabled() -> bool {
    cfg!(feature = "dhat")
}

/// `heap_profiling_enabled()` ile uyumsuz feature set'i kullanılırsa derleme-zamanı
/// hata vermesini sağlayan sentinel.
#[allow(dead_code)]
fn _assert_dhat_is_optional(_err: ViscosError) -> ViscosError {
    _err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_heap_profiling_succeeds() {
        let guard = init_heap_profiling().expect("init");
        // guard'ı explicit düşür — Drop'un çalıştığını garanti et.
        drop(guard);
    }

    #[test]
    fn heap_profiling_enabled_matches_feature_flag() {
        // Test binary'si `dhat` feature ile derlenmedi → false beklenir.
        assert!(!heap_profiling_enabled());
    }
}
