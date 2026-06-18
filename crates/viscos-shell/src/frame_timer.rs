//! Frame timing ölçümü (native frame drop <%1 hedefi).
//!
//! Faz 1.0'da `iced 0.14` spike'ında kullanılacak: 1 saniye boyunca frame
//! sayısını loglar, drop oranı %1'i aşarsa uyarı verir.
//!
//! Cross-reference: [`phase-1.0-window-webview.md` iced-spike](../../.cursor/plans/phase-1.0-window-webview.md#iced-spike).

use std::time::{Duration, Instant};

/// Bir frame tick ölçer.
#[derive(Debug, Clone)]
pub struct FrameTimer {
    start: Instant,
    frame_count: u64,
    drop_count: u64,
    last_tick: Option<Instant>,
    target_frame_budget: Duration,
}

impl Default for FrameTimer {
    fn default() -> Self {
        // 60 FPS hedefi: ~16.67ms / frame. %1 drop toleransı için ~16.83ms.
        Self::new(60)
    }
}

impl FrameTimer {
    /// Yeni frame timer oluştur (target FPS).
    ///
    /// # Examples
    ///
    /// ```
    /// use viscos_shell::FrameTimer;
    ///
    /// let timer = FrameTimer::new(60);
    /// assert_eq!(timer.target_fps(), 60);
    /// ```
    #[must_use]
    pub fn new(target_fps: u32) -> Self {
        let budget_micros = 1_000_000_u64
            .checked_div(u64::from(target_fps))
            .unwrap_or(16_667);
        Self {
            start: Instant::now(),
            frame_count: 0,
            drop_count: 0,
            last_tick: None,
            target_frame_budget: Duration::from_micros(budget_micros),
        }
    }

    /// Target FPS'i döndürür.
    #[must_use]
    pub fn target_fps(&self) -> u32 {
        // ~16.67ms @ 60 FPS. 16_667µs round-trip.
        let micros = self.target_frame_budget.as_micros();
        let safe_micros = if micros == 0 { 1 } else { micros };
        (1_000_000 / safe_micros as u32).max(1)
    }

    /// Yeni frame'i işaretle.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(prev) = self.last_tick {
            let elapsed = now.duration_since(prev);
            if elapsed > self.target_frame_budget {
                self.drop_count += 1;
            }
        }
        self.last_tick = Some(now);
        self.frame_count += 1;
    }

    /// Şu ana kadar birikmiş istatistikler.
    #[must_use]
    pub fn stats(&self) -> FrameStats {
        let total_elapsed = self.start.elapsed();
        let observed_fps = if total_elapsed.as_secs_f64() > 0.0 {
            self.frame_count as f64 / total_elapsed.as_secs_f64()
        } else {
            0.0
        };
        let drop_ratio = if self.frame_count > 0 {
            self.drop_count as f64 / self.frame_count as f64
        } else {
            0.0
        };
        FrameStats {
            frame_count: self.frame_count,
            drop_count: self.drop_count,
            observed_fps,
            drop_ratio,
            elapsed: total_elapsed,
        }
    }

    /// Timer'ı sıfırla.
    pub fn reset(&mut self) {
        self.start = Instant::now();
        self.frame_count = 0;
        self.drop_count = 0;
        self.last_tick = None;
    }
}

/// Birikmiş frame istatistikleri.
#[derive(Debug, Clone, Copy)]
pub struct FrameStats {
    /// Toplam frame sayısı.
    pub frame_count: u64,
    /// Budget aşımı sayısı (drop).
    pub drop_count: u64,
    /// Gözlemlenen FPS (frame_count / elapsed_secs).
    pub observed_fps: f64,
    /// Drop oranı (drop_count / frame_count). %1 altı olmalı (hedef).
    pub drop_ratio: f64,
    /// Geçen toplam süre.
    pub elapsed: Duration,
}

impl FrameStats {
    /// Drop oranı hedefin (%1) üstünde mi?
    #[must_use]
    pub fn exceeds_target(&self) -> bool {
        self.drop_ratio > 0.01
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn default_timer_targets_60_fps() {
        let timer = FrameTimer::default();
        assert_eq!(timer.target_fps(), 60);
    }

    #[test]
    fn tick_increments_frame_count() {
        let mut timer = FrameTimer::new(60);
        timer.tick();
        timer.tick();
        timer.tick();
        let stats = timer.stats();
        assert_eq!(stats.frame_count, 3);
        assert_eq!(stats.drop_count, 0);
        assert!(!stats.exceeds_target());
    }

    #[test]
    fn slow_frame_triggers_drop() {
        // 1000 FPS hedefi: 1ms budget. 5ms sleep = drop.
        let mut timer = FrameTimer::new(1000);
        timer.tick();
        sleep(Duration::from_millis(5));
        timer.tick();
        let stats = timer.stats();
        assert_eq!(stats.frame_count, 2);
        assert_eq!(stats.drop_count, 1);
        assert!(stats.exceeds_target());
    }

    #[test]
    fn reset_clears_stats() {
        let mut timer = FrameTimer::new(60);
        timer.tick();
        timer.tick();
        timer.reset();
        let stats = timer.stats();
        assert_eq!(stats.frame_count, 0);
        assert_eq!(stats.drop_count, 0);
    }

    #[test]
    fn observed_fps_is_calculated() {
        let mut timer = FrameTimer::new(60);
        for _ in 0..10 {
            timer.tick();
        }
        sleep(Duration::from_millis(20));
        let stats = timer.stats();
        // 10 frame / çok küçük elapsed → observed_fps yüksek olmalı.
        assert!(stats.observed_fps > 0.0);
        assert!(stats.elapsed >= Duration::from_millis(20));
    }
}
