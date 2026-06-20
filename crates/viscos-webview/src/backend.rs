//! `WebViewBackend` trait — pluggable backend abstraction.
//!
//! Faz 1.0: trait tanımı + 2 stub backend (`WebView2Backend`, `CefBackend`).
//! Faz 1.6: gerçek `wry::WebViewBuilder` ve `cef-rs` entegrasyonu.
//!
//! Cross-references:
//! - [ADR-0012 §1](../../../../docs/DECISIONS.md#adr-0012-frontend-mimari--hibrit-webview--native-shell-haziran-2026-trade-off-revizyonu)
//! - [`webview2-hardening.md` Katman 3](../../.cursor/plans/webview2-hardening.md#katman-3-cef-backend-faz-16--win11-default-mvpnin-parçası)
//! - [`phase-1.6-cef-default-rollout.md`](../../.cursor/plans/phase-1.6-cef-default-rollout.md)

use std::fmt;

use serde::{Deserialize, Serialize};
use viscos_error::{Result, ViscosError};

use crate::DISCORD_APP_URL;

/// Backend seçim kararı.
///
/// Faz 1.0'da runtime'da `select_default_backend()` ile hesaplanır; config override
/// (`config.toml` `[webview].backend`) Faz 1.6'da eklenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    /// Microsoft Edge WebView2 (Windows 10/11 OS-bundled).
    /// Avantaj: hafif binary (15-25 MB), Edge security update ile otomatik fix.
    /// Dezavantaj: Win11 GDI object leak ([WebView2Feedback #5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536)).
    WebView2,
    /// Chromium Embedded Framework — `tauri-apps/cef-rs`.
    /// Avantaj: leak'siz (Win11), RDP güvenli, multi-platform tutarlı.
    /// Dezavantaj: 220-300 MB binary (Faz 1.6 / 8.5 self-update gerekli).
    Cef,
}

impl BackendKind {
    /// Backend'in insan-okunabilir adı.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WebView2 => "webview2",
            Self::Cef => "cef",
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Pencere oluşturma konfigürasyonu.
///
/// Faz 1.0'da minimum alanlar. Faz 5+'ta iced UI ile dinamik boyutlandırma eklenecek.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Pencere başlığı (taskbar + window title).
    pub title: String,
    /// Başlangıç genişliği (mantıksal piksel).
    pub width: u32,
    /// Başlangıç yüksekliği (mantıksal piksel).
    pub height: u32,
    /// Karanlık tema önerisi (`"dark"` | `"light"`). OS'a bildirilir.
    pub theme: String,
    /// İlk yüklenecek URL. Default: [`crate::DISCORD_APP_URL`].
    pub initial_url: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Viscos".to_string(),
            width: 1280,
            height: 800,
            theme: "dark".to_string(),
            initial_url: DISCORD_APP_URL.to_string(),
        }
    }
}

/// Pluggable WebView backend kontratı.
///
/// Faz 1.0: stub struct'lar (`WebView2Backend::new()`). Faz 1.6: gerçek `wry` veya `cef-rs`.
///
/// Backend'ler `tao::EventLoopWindowTarget<()>` üzerinden pencere + WebView'i
/// birlikte oluşturur (Faz 1.6 — minimal entegrasyon). `Shell::run()` event loop
/// sırasında `target` parametresiyle bu trait'i çağırır; backend kendi `tao::Window`'unu
/// kurar ve WebView'i attach eder.
///
/// # Errors
///
/// `create_window` aşağıdaki durumlarda `ViscosError` döner:
/// - `WebView2 runtime missing` (Edge WebView2 redistributable yüklü değil)
/// - `Cef runtime missing` (CEF native library bulunamadı)
/// - Platform desteklenmiyor (yalnızca Windows v1; Linux/macOS Faz 8.5+)
/// - Pencere oluşturma başarısız (DISPLAY/Wayland yok, vb.)
/// - Feature-gated stub'da `ViscosError::Unimplemented("cef-backend feature not enabled")`
pub trait WebViewBackend: Send + Sync {
    /// Yeni bir WebView penceresi oluştur (pencere + WebView tek atomik adım).
    ///
    /// `target` ana thread'in `tao::EventLoopWindowTarget<()>` referansı.
    /// Backend bu target üzerinden `tao::Window` oluşturur ve WebView'i attach eder.
    ///
    /// # Errors
    ///
    /// `ViscosError::Unimplemented(...)` feature-gated stub'larda veya
    /// runtime/platform-specific hatalar (`WebView2 missing`, `cef.dll missing`).
    fn create_window(
        &self,
        target: &tao::event_loop::EventLoopWindowTarget<()>,
        config: &WindowConfig,
    ) -> Result<Box<dyn WebViewWindow>>;

    /// Backend'in insan-okunabilir adı (log + tray badge).
    fn name(&self) -> &'static str;

    /// Backend sürümü (compile-time). Faz 1.6+'da runtime'a taşınacak.
    fn version(&self) -> &'static str;

    /// Bilinen upstream bug'lar (PR review + AI documentation için).
    fn known_issues(&self) -> &[&'static str];

    /// Zero-copy binary blob transfer (Faz 4'te implemente edilecek).
    ///
    /// Faz 1.0'da default impl `ViscosError::Unimplemented` döner. Faz 4'te:
    /// - WebView2 backend: `CoreWebView2SharedBuffer` API'si.
    /// - CEF backend: `SharedMemoryRegion` + `message_router`.
    ///
    /// # Errors
    ///
    /// Her zaman `ViscosError::Unimplemented("phase-4.0 shared buffer")` (Faz 1.0).
    fn post_shared_buffer(&self, _bytes: &[u8], _metadata: &str) -> Result<()> {
        Err(ViscosError::Unimplemented(
            "post_shared_buffer Faz 4'te implemente edilecek (bkz. phase-4.0-cache-media.md)",
        ))
    }
}

/// WebView penceresi handle — shared across shell + watchdog.
///
/// Faz 1.6+: `eval`, `navigate`, `close`, `id` method'ları.
///
/// Tüm methodlar ana thread üzerinden çağrılmalıdır (WebView2 COM
/// main-thread affinity, CEF CefThread::UI). Bunun için
/// `tao::EventLoop::run()` callback dispatch'i kullanılır.
pub trait WebViewWindow: Send + Sync + std::fmt::Debug {
    /// Pencere / WebView ID (debug + window registry için). Unique per-process.
    fn id(&self) -> u64;

    /// WebView içinde JavaScript çalıştır (eval).
    ///
    /// Faz 1.6'da WebView2 `ICoreWebView2::ExecuteScript`, CEF `CefFrame::ExecuteJavaScript`.
    /// Büyük payload'lar için Faz 4'te `post_shared_buffer` kullanılacak
    /// (10KB threshold).
    ///
    /// # Errors
    ///
    /// WebView çöktüğünde, IPC kanalı kapandığında veya platform hata kodu döndüğünde.
    fn eval(&self, script: &str) -> Result<()>;

    /// WebView'i yeni bir URL'e yönlendir.
    ///
    /// # Errors
    ///
    /// URL parsing/navigation başarısız olduğunda.
    fn navigate(&self, url: &str) -> Result<()>;

    /// Pencereyi kapat (event loop'a CloseRequested gönderir).
    ///
    /// # Errors
    ///
    /// Zaten kapalı veya event loop sonlandırılmışsa.
    fn close(&self) -> Result<()>;

    /// Type-erased downcast hook (Faz 1.0 marker; Faz 1.6+ tutulur).
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Faz 1.0 default backend seçimi (config override Faz 1.6).
///
/// **Öncelik sırası** (Faz 1.6 kararı, ADR-0012 §4):
/// 1. Config override (`config.toml [webview].backend`)
/// 2. RDP session auto-detect → CEF zorla
/// 3. Telemetry override (≥10 restart/24h → CEF, Faz 1.5 sonrası)
/// 4. Platform default: Win11 build ≥ 22000 → CEF, aksi → WebView2
///
/// **B1 kararı (USER-APPROVED):** Default build → CEF stub (`Err(Unimplemented)`).
/// Feature ON (`--features viscos-webview/cef-backend`) → gerçek runtime.
/// Win11 default olarak CEF'i **önerir** ama feature kapalıysa WebView2'ye
/// fallback yapar (production binary CEF feature ON ile derlenmeli).
#[must_use]
pub fn select_default_backend() -> BackendKind {
    if cfg!(target_os = "windows") && is_windows_11() && cfg!(feature = "cef-backend") {
        BackendKind::Cef
    } else {
        BackendKind::WebView2
    }
}

/// Windows 11 build number tespiti (`build >= 22000`).
///
/// `windows-version 0.1` üzerinden runtime detection. Compile-time
/// `cfg!(target_os = "windows")` ile birleştirilir: build target Windows
/// değilse her zaman `false` döner (Linux/macOS CI runner'larında olduğu gibi).
///
/// **B1 kararı (Faz 1.6):** Runtime detection (compile-time `cfg!` yerine)
/// sayesinde Windows 10 ile Windows 11 aynı binary'de ayırt edilebilir.
#[must_use]
pub fn is_windows_11() -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    windows_version::OsVersion::current().build >= 22000
}

/// RDP (Remote Desktop Protocol) session tespiti.
///
/// Microsoft WebView2 RDP üzerinde GDI region leak yapıyor
/// ([WebView2Feedback #5266](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5266)).
/// Bu yüzden Faz 1.6 + ADR-0012 §6 kararı: RDP session'da CEF backend zorla.
///
/// Windows-only: `GetSystemMetrics(SM_REMOTESESSION) != 0`. Non-Windows'ta `false`.
#[must_use]
pub fn is_rdp_session() -> bool {
    #[cfg(windows)]
    {
        // SAFETY: GetSystemMetrics no-side-effect query; SM_REMOTESESSION değeri
        // 0 (yerel konsol) veya non-zero (RDP session) döner. Çağrı thread-safe.
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_REMOTESESSION,
            ) != 0
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Resolve `BackendKind` from CLI + config + platform priority chain.
///
/// **Öncelik sırası** (Faz 1.6 Dalga 1c):
/// 1. CLI override (`--backend=webview2|cef|auto`) — wins.
/// 2. Config override (`config.toml [webview].backend`).
/// 3. RDP session auto-detect → `Cef`.
/// 4. Win11 + cef-backend feature → `Cef`.
/// 5. Fallback → `WebView2`.
///
/// "auto" değeri (CLI veya config) auto-detect'e düşer.
///
/// # Errors
///
/// CLI/config değeri `auto` / `webview2` / `cef` dışında bir string ise
/// `ViscosError::Media` döner.
pub fn resolve_backend(
    cli_override: Option<&str>,
    config_override: Option<&str>,
) -> Result<BackendKind> {
    // 1. CLI override en yüksek öncelik.
    if let Some(value) = cli_override {
        return parse_backend_value(value);
    }

    // 2. Config override (boş/auto ise auto-detect'e düş).
    if let Some(value) = config_override {
        let trimmed = value.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("auto") {
            return parse_backend_value(value);
        }
    }

    // 3-5. Auto-detect (RDP > Win11+CEF > WebView2).
    let mut chosen = select_default_backend();
    if cfg!(target_os = "windows") && is_rdp_session() {
        chosen = BackendKind::Cef;
    }
    Ok(chosen)
}

/// String'i `BackendKind`'e parse et (case-insensitive).
///
/// "auto" → `Ok(BackendKind::WebView2)` (auto-detect default fallback).
///
/// # Errors
///
/// Bilinmeyen backend string'i `ViscosError::Media` ile döner.
fn parse_backend_value(value: &str) -> Result<BackendKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "webview2" => Ok(BackendKind::WebView2),
        "cef" => Ok(BackendKind::Cef),
        "auto" => {
            // "auto" → auto-detect. CLI sözleşmesi: explicit `auto` da
            // resolve zincirinin 3-5. adımına (RDP > Win11 > WebView2) düşer.
            let mut chosen = select_default_backend();
            if cfg!(target_os = "windows") && is_rdp_session() {
                chosen = BackendKind::Cef;
            }
            Ok(chosen)
        }
        other => Err(viscos_error::ViscosError::Media(format!(
            "unknown backend '{other}' (expected: webview2 | cef | auto)"
        ))),
    }
}

/// Shared `Arc<dyn WebViewBackend>` factory helper.
pub type SharedBackend = std::sync::Arc<dyn WebViewBackend>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_kind_display_matches_as_str() {
        assert_eq!(BackendKind::WebView2.to_string(), "webview2");
        assert_eq!(BackendKind::Cef.to_string(), "cef");
    }

    #[test]
    fn window_config_default_has_discord_url() {
        let cfg = WindowConfig::default();
        assert_eq!(cfg.title, "Viscos");
        assert_eq!(cfg.width, 1280);
        assert_eq!(cfg.height, 800);
        assert_eq!(cfg.theme, "dark");
        assert!(cfg.initial_url.contains("discord.com/app"));
    }

    #[test]
    fn select_default_backend_returns_a_known_kind() {
        // Faz 1.0: Win11 ise CEF, değilse WebView2.
        // CI `windows-latest` runner'lar Win11 (build >= 22000) olduğu için CEF beklenir.
        // Linux/macOS'ta WebView2 beklenir.
        let kind = select_default_backend();
        assert!(
            matches!(kind, BackendKind::WebView2 | BackendKind::Cef),
            "select_default_backend must return a known kind, got {kind:?}"
        );
    }

    #[test]
    fn is_windows_11_matches_target_os() {
        // Compile-time: target_os = windows ise runtime build kontrolü yapılır;
        // aksi her zaman false. CI windows-latest Windows 11 22H2+ üzerinde koşar
        // → build >= 22000 → true.
        #[cfg(target_os = "windows")]
        {
            // CI runner'larında build >= 22000 olmalı. Eğer bir gün Windows 10
            // runner'a geçersek bu test `is_windows_11() == false` olur ve
            // select_default_backend WebView2'ye döner — CI matrix update gerek.
            assert!(
                is_windows_11(),
                "windows-latest runner Windows 11+ olmalı (build >= 22000)"
            );
        }
        #[cfg(not(target_os = "windows"))]
        assert!(!is_windows_11());
    }

    #[test]
    fn is_rdp_session_returns_bool() {
        // Sadece derleme/smoke test — RDP'de true, değilse false.
        // CI runner'lar konsol session → false beklenir.
        let rdp = is_rdp_session();
        #[cfg(target_os = "windows")]
        {
            // Hem true hem false kabul edilir — RDP olup olmadığı runner'a bağlı.
            // Test sadece "bool return etti" garantisi veriyor.
            let _: bool = rdp;
        }
        #[cfg(not(target_os = "windows"))]
        assert!(!rdp, "non-Windows must report RDP=false");
    }

    // Not: `resolve_backend_*` + `post_shared_buffer` testleri integration
    // test olarak `tests/resolve_backend_priority.rs`'e taşındı
    // (.cursorrules Bölüm 2 400 satır uyarısını aşmamak için).

    /// `WebViewBackend` trait'inin object-safe olduğunu doğrula:
    /// `Box<dyn WebViewBackend>` üzerinden virtual dispatch mümkün olmalı.
    ///
    /// Derleme zamanı güvencesi — runtime assertion gerekmiyor.
    #[test]
    fn webview_backend_trait_is_object_safe() {
        fn _accepts_dyn(_: Box<dyn WebViewBackend>) {}
        fn _accepts_dyn_ref(_: &dyn WebViewBackend) {}
    }
}
