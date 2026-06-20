//! `viscos-config` — typed configuration loader (ADR-0003: config-rs).
//!
//! Layered loading (12-factor):
//! 1. `config/default.toml` (içerikte gömülü fallback; repo'da committed)
//! 2. `config/local.toml` (geliştirici override'ı; .gitignore'da)
//! 3. Env var override: `VISCOS_APP__NAME=foo`, `VISCOS_LOGGING__LEVEL=debug`
//!    `__` separator ile nested path; `convert-case` crate ile kebab-case ↔ snake_case.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod path;

pub use path::resolve_cache_dir;

/// Root konfigürasyon tipi.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub webview: WebviewConfig,
    #[serde(default)]
    pub watchdog: WatchdogConfig,
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub data_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "Viscos".to_string(),
            data_dir: "%APPDATA%/Viscos".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub theme: String,
    pub initial_url: String,
    pub tray_enabled: bool,
    pub devtools_enabled: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
            title: "Viscos".to_string(),
            theme: "dark".to_string(),
            initial_url: "https://discord.com/app".to_string(),
            tray_enabled: true,
            devtools_enabled: cfg!(debug_assertions),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebviewConfig {
    /// Backend seçimi (`auto`, `webview2`, `cef`).
    pub backend: String,
    /// RDP session auto-detect → CEF zorla (Faz 1.6).
    pub rdp_force_cef: bool,
    /// Faz 4 SharedBuffer aktif mi? (Faz 1.0'da false).
    pub post_shared_buffer_faz4: bool,
}

impl Default for WebviewConfig {
    fn default() -> Self {
        Self {
            backend: "auto".to_string(),
            rdp_force_cef: true,
            post_shared_buffer_faz4: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConfig {
    pub gdi_warn: u32,
    pub gdi_critical: u32,
    pub sample_interval_secs: u64,
    pub warmup_samples: u32,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            gdi_warn: 7000,
            gdi_critical: 9000,
            sample_interval_secs: 30,
            warmup_samples: 2,
        }
    }
}

/// Cache katmanı konfigürasyonu (Faz 4.0 Dalga 1, ADR-0010).
///
/// Üç alanı tüketir:
/// - `data_dir` — cache kök dizini (varsayılan: `dirs::data_local_dir() + "viscos/cache"`).
/// - `sqlite_path` — SQLite WAL dosya yolu (`data_dir/cache.db` default).
/// - `max_size_mb` — RAM tier (moka) kapasite üst sınırı; default 64 MB (ADR-0010 §B).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache kök dizini (parent SQLite dosyaları için kullanılır).
    pub data_dir: PathBuf,
    /// SQLite WAL dosya yolu. `data_dir/cache.db` default'u ile doldurulur.
    pub sqlite_path: PathBuf,
    /// moka RAM tier byte cinsinden kapasite. ADR-0010 §B: 64 MB default.
    pub max_size_mb: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        let data_dir = resolve_cache_dir().unwrap_or_else(|_| PathBuf::from("./cache"));
        let sqlite_path = data_dir.join("cache.db");
        Self {
            data_dir,
            sqlite_path,
            max_size_mb: 64,
        }
    }
}

impl CacheConfig {
    /// Construct from explicit overrides (used by tests + callers that bypass
    /// the layered loader).
    pub fn new(data_dir: PathBuf, max_size_mb: u64) -> Self {
        let sqlite_path = data_dir.join("cache.db");
        Self {
            data_dir,
            sqlite_path,
            max_size_mb,
        }
    }

    /// Layered load + typed deserialize for the cache section. Returns
    /// [`ConfigError`](config::ConfigError) on parse / IO failure.
    pub fn load() -> Result<Self, config::ConfigError> {
        let cfg = Config::load()?;
        Ok(cfg.cache)
    }

    /// Variant that derives from an already-loaded [`Config`] — preferred entry
    /// point when the application already holds the root config (avoids re-parse).
    #[must_use]
    pub fn from_config(cfg: &Config) -> Self {
        cfg.cache.clone()
    }
}

impl Config {
    /// Layered config load: default → local → env override.
    pub fn load() -> Result<Self, config::ConfigError> {
        let builder = config::Config::builder()
            // 1. Varsayılan: config/default.toml (içerikte gömülü fallback)
            .add_source(config::File::with_name("config/default").required(false))
            // 2. Platforma özel override: config/local.toml (gitignore)
            .add_source(config::File::with_name("config/local").required(false))
            // 3. Env var override: VISCOS_APP__NAME=foo, VISCOS_LOGGING__LEVEL=debug
            .add_source(
                config::Environment::with_prefix("VISCOS")
                    .separator("__")
                    .try_parsing(true),
            );

        builder.build()?.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_default_struct() {
        let cfg = Config {
            app: AppConfig {
                name: "Viscos".to_string(),
                data_dir: "%APPDATA%/Viscos".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
            ..Config::default()
        };

        let s = toml::to_string(&cfg).expect("serialize");
        let back: Config = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.app.name, "Viscos");
        assert_eq!(back.logging.level, "info");
        assert_eq!(back.logging.format, "pretty");
        assert_eq!(back.window.width, 1280);
        assert_eq!(back.window.height, 800);
        assert_eq!(back.webview.backend, "auto");
        assert_eq!(back.watchdog.gdi_critical, 9000);
    }

    #[test]
    fn phase_1_0_defaults_match_plan() {
        let cfg = Config::default();
        assert_eq!(cfg.window.width, 1280);
        assert_eq!(cfg.window.height, 800);
        assert_eq!(cfg.window.theme, "dark");
        assert!(cfg.window.initial_url.contains("discord.com/app"));
        assert!(cfg.window.tray_enabled);
        assert_eq!(cfg.watchdog.gdi_warn, 7000);
        assert_eq!(cfg.watchdog.gdi_critical, 9000);
        assert_eq!(cfg.watchdog.sample_interval_secs, 30);
    }

    #[test]
    fn cache_config_default_provides_paths() {
        let cfg = CacheConfig::default();
        // data_dir'in altında sqlite_path olmalı.
        assert!(cfg.sqlite_path.starts_with(&cfg.data_dir));
        assert_eq!(cfg.max_size_mb, 64);
    }

    #[test]
    fn cache_config_new_computes_sqlite_path() {
        let cfg = CacheConfig::new(PathBuf::from("/tmp/viscos-cache"), 128);
        assert_eq!(cfg.data_dir, PathBuf::from("/tmp/viscos-cache"));
        assert_eq!(cfg.sqlite_path, PathBuf::from("/tmp/viscos-cache/cache.db"));
        assert_eq!(cfg.max_size_mb, 128);
    }

    #[test]
    fn cache_config_from_config_returns_clone() {
        let cfg = Config::default();
        let cache = CacheConfig::from_config(&cfg);
        assert_eq!(cache.max_size_mb, cfg.cache.max_size_mb);
    }

    #[test]
    fn cache_config_round_trip_via_toml() {
        let cfg = CacheConfig::new(PathBuf::from("/var/viscos"), 256);
        let s = toml::to_string(&cfg).expect("serialize");
        let back: CacheConfig = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.data_dir, PathBuf::from("/var/viscos"));
        assert_eq!(back.max_size_mb, 256);
    }
}
