//! `GdiCounter` — Windows GDI object sayacı.
//!
//! `GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS)` Win32 API'sini
//! kullanır. **GR_GDIOBJECTS = 0** sabit değeri Microsoft'tan.
//!
//! Non-Windows platformlarda stub: her zaman 0 döner (leak tespiti imkansız).

#[cfg(windows)]
mod platform {
    /// `GetGuiResources` GR_GDIOBJECTS sabiti (0).
    ///
    /// `windows 0.58` crate'i `Win32_System_Threading` feature'ı altında
    /// hem `GetCurrentProcess` hem `GetGuiResources` + `GR_GDIOBJECTS` re-export eder.
    ///
    /// [`Win32_UI_WindowsAndMessaging` modülünde değil][docs] — Microsoft metadata
    /// her ikisini de `winuser.h`'tan derive ediyor olsa da `windows-rs` enum'ları
    /// `Threading` modülüne koymuş.
    ///
    /// [docs]: https://docs.rs/windows/0.58.0/windows/Win32/System/Threading/fn.GetGuiResources.html
    pub fn gdi_count() -> u32 {
        use windows::Win32::Foundation::{ERROR_SUCCESS, SetLastError};
        use windows::Win32::System::Threading::{
            GR_GDIOBJECTS, GetCurrentProcess, GetGuiResources,
        };
        // Microsoft issue #1920: `GetGuiResources` zero ile hata kodu arasında
        // ayrım yapmak için önce `SetLastError(ERROR_SUCCESS)` çağır.
        // Safety: `SetLastError` safe; `GetCurrentProcess` kernel handle döner
        // (kapatılmaz); `GetGuiResources` user32.dll'den değer okur.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS)
        }
    }
}

#[cfg(not(windows))]
mod platform {
    /// Non-Windows stub. Faz 1.0 v1 yalnızca Windows; ileride platform-specific
    /// implementasyonlar eklenecek (macOS: `task_info`, Linux: `/proc/self/status`).
    pub fn gdi_count() -> u32 {
        0
    }
}

/// Tek bir GDI sample — timestamp + count + delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GdiSample {
    /// Şu anki GDI object sayısı.
    pub count: u32,
    /// Önceki sample'a göre delta (`saturating_sub`, negatif olamaz).
    pub delta: u32,
    /// Sample alındığı an (epoch microseconds).
    pub timestamp_us: u64,
}

/// GDI counter — sample al + delta hesapla.
#[derive(Debug, Clone)]
pub struct GdiCounter {
    last_count: Option<u32>,
}

impl Default for GdiCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl GdiCounter {
    /// Yeni counter.
    #[must_use]
    pub const fn new() -> Self {
        Self { last_count: None }
    }

    /// Şu anki GDI object sayısını oku.
    #[must_use]
    pub fn current(&self) -> u32 {
        platform::gdi_count()
    }

    /// Yeni sample al.
    ///
    /// İlk çağrıda `delta = 0` (baseline). Sonraki çağrılarda bir önceki
    /// sample'a göre fark.
    pub fn sample(&mut self) -> GdiSample {
        let count = self.current();
        let delta = match self.last_count {
            Some(prev) => count.saturating_sub(prev),
            None => 0,
        };
        self.last_count = Some(count);
        let timestamp_us = std_time_micros();
        GdiSample {
            count,
            delta,
            timestamp_us,
        }
    }

    /// Baseline sıfırla (restart sonrası).
    pub fn reset(&mut self) {
        self.last_count = None;
    }
}

/// `std::time::SystemTime` epoch microseconds.
///
/// Cross-platform (Windows + Unix). Hata durumunda 0.
fn std_time_micros() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_has_zero_delta() {
        let mut counter = GdiCounter::new();
        let sample = counter.sample();
        // İlk sample her zaman delta = 0 (baseline yok).
        assert_eq!(sample.delta, 0, "first sample must have delta 0");
        // CI runner'lar headless olabilir; bu yüzden count > 0 şartı yok.
        // Windows GUI process'te count genelde > 0; headless test'te 0 olabilir.
        assert!(sample.timestamp_us > 0, "timestamp must be set");
    }

    #[test]
    fn subsequent_samples_have_correct_delta() {
        let mut counter = GdiCounter::new();
        let first = counter.sample();
        let second = counter.sample();
        // Delta pozitif veya sıfır olmalı (GDI count asla azalmaz; leak = artış).
        assert!(second.delta <= second.count);
        // GDI sayısı monotonik artmaz, ama delta en fazla `count` kadar olabilir.
        assert!(first.count <= second.count + first.delta);
    }

    #[test]
    fn reset_clears_baseline() {
        let mut counter = GdiCounter::new();
        counter.sample();
        counter.sample();
        counter.reset();
        let sample = counter.sample();
        assert_eq!(sample.delta, 0, "after reset, delta must be 0");
    }

    #[test]
    fn counter_returns_valid_u32_on_all_platforms() {
        // Hem Windows hem non-Windows'ta u32 dönmeli.
        let counter = GdiCounter::new();
        let count = counter.current();
        // Non-Windows stub her zaman 0; Windows GUI process'te > 0;
        // Windows headless test runner'da 0 olabilir.
        if cfg!(not(windows)) {
            assert_eq!(count, 0, "non-Windows stub returns 0");
        }
    }
}
