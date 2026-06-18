//! `DraftAutosave` trait + stub implementasyon.
//!
//! Restart öncesi açık mesaj taslaklarını kaydet. Faz 1'de in-memory stub;
//! Faz 2'de `viscos-cache` (SQLite WAL) ile tam entegrasyon.
//!
//! Cross-reference: [`phase-1.0-window-webview.md` §3.4 — Draft Autosave Hook](../../.cursor/plans/phase-1.0-window-webview.md#34-viscos-watchdog-kritik).

use std::sync::atomic::{AtomicUsize, Ordering};

use viscos_error::Result;

/// Restart öncesi draft mesaj taslaklarını kaydet.
///
/// `WebViewHandle` üzerinden DOM hook Faz 5'te eklenecek; Faz 1'de stub yeterli.
pub trait DraftAutosave: Send + Sync {
    /// Açık composer pencerelerini snapshot'la.
    ///
    /// Returns kaydedilen taslak sayısı.
    ///
    /// # Errors
    ///
    /// Faz 2+'da SQLite I/O hataları. Faz 1.0'da her zaman OK.
    fn snapshot_open_composers(&self) -> Result<usize>;
}

/// Stub `DraftAutosave` — sayımı in-memory tutar.
///
/// Faz 1.0'da `watchdog` test'lerinde kullanılır. Faz 2+'da SQLite-backed
/// implementasyon eklenecek.
#[derive(Debug, Default)]
pub struct StubAutosave {
    snapshots_taken: AtomicUsize,
    last_count: AtomicUsize,
}

impl StubAutosave {
    /// Yeni stub autosave.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Şu ana kadar alınan snapshot sayısı.
    #[must_use]
    pub fn snapshots_taken(&self) -> usize {
        self.snapshots_taken.load(Ordering::SeqCst)
    }

    /// Son snapshot'taki taslak sayısı.
    #[must_use]
    pub fn last_count(&self) -> usize {
        self.last_count.load(Ordering::SeqCst)
    }
}

impl DraftAutosave for StubAutosave {
    fn snapshot_open_composers(&self) -> Result<usize> {
        self.snapshots_taken.fetch_add(1, Ordering::SeqCst);
        // Faz 1.0 stub: 2 taslak varsay (Discord varsayılan davranışı).
        let count = 2;
        self.last_count.store(count, Ordering::SeqCst);
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_autosave_returns_consistent_count() {
        let autosave = StubAutosave::new();
        let n1 = autosave.snapshot_open_composers().unwrap();
        let n2 = autosave.snapshot_open_composers().unwrap();
        assert_eq!(n1, n2);
        assert_eq!(autosave.snapshots_taken(), 2);
        assert_eq!(autosave.last_count(), n1);
    }
}
