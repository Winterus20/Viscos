//! `viscos-config` — typed configuration loader (ADR-0003: config-rs).
//!
//! Layered loading (12-factor):
//! 1. `config/default.toml` (içerikte gömülü fallback; repo'da committed)
//! 2. `config/local.toml` (geliştirici override'ı; .gitignore'da)
//! 3. Env var override: `VISCOS_APP__NAME=foo`, `VISCOS_LOGGING__LEVEL=debug`
//!    `__` separator ile nested path; `convert-case` crate ile kebab-case ↔ snake_case.

use serde::{Deserialize, Serialize};

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
}
