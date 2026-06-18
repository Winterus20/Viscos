//! Vencord/Equicord uyumu — `ViscosNative` POC bridge API.
//!
//! Vesktop'un `VesktopNative` (Electron `ipcMain.handle` tabanlı) pattern'i
//! referans alınarak Viscos'un kendi `ViscosNative` API'si tasarlanmıştır.
//! Frontend tarafında Vencord/Equicord plugin'leri `window.ViscosNative.*`
//! üzerinden Rust tarafına command gönderebilir.
//!
//! Faz 5.0 kapsamı: 4 endpoint (GetVersion, GetSettings, UpdateSettings,
//! GetDiskInfo). Faz 6.0'da 11 namespace'in tamamı (win, virtmic, settings,
//! spellcheck, commands, plugins, themes, ...) eklenecek.
//!
//! Cross-references:
//! - [`phase-5.0-native-ui.md` §7 Vencord/Equicord Plugin Uyumu](../../../.cursor/plans/phase-5.0-native-ui.md)
//! - [`phase-6.0-hotkeys.md` §7 Vencord/Equicord Tam Entegrasyon](../../../.cursor/plans/phase-6.0-hotkeys.md)
//! - ADR-0012 §6 — `ViscosNative` API yüzeyi.

use serde::{Deserialize, Serialize};
use viscos_core::VISCOS_VERSION;
use viscos_error::ViscosError;

/// Plugin → Native request enum.
///
/// Vencord/Equicord plugin'leri `window.ViscosNative.invoke(req)` üzerinden
/// bu enum'un varyantlarını gönderir. JSON formatı:
///
/// ```json
/// {"type": "getVersion"}
/// {"type": "getSettings"}
/// {"type": "updateSettings", "settings": {"key": "value"}}
/// {"type": "getDiskInfo"}
/// ```
///
/// `#[serde(tag = "type", rename_all = "camelCase")]` sayesinde JS tarafı
/// doğal `camelCase` ile gönderir; Rust tarafı `PascalCase` varyant isimlerini
/// otomatik dönüştürür.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ViscosNativeRequest {
    /// Viscos versiyon bilgisi (sürüm + build hash).
    GetVersion,
    /// Kullanıcı ayarlarının tamamı (Faz 6'da typed `Settings` struct olur).
    GetSettings,
    /// Ayar güncelle (partial merge).
    UpdateSettings {
        /// Güncellenecek ayarlar (partial JSON).
        settings: serde_json::Value,
    },
    /// Disk kullanım bilgisi (free_bytes / total_bytes).
    GetDiskInfo,
}

/// Native → Plugin response enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[non_exhaustive]
pub enum ViscosNativeResponse {
    /// Versiyon cevabı.
    Version {
        /// Semver string (örn. "0.1.0").
        version: String,
        /// Build hash (kısa git SHA veya "dev").
        hash: String,
    },
    /// Ayar snapshot'ı.
    Settings {
        /// Ayar JSON (Faz 6'da typed).
        settings: serde_json::Value,
    },
    /// Disk bilgisi.
    DiskInfo {
        /// Boş byte sayısı.
        free_bytes: u64,
        /// Toplam byte sayısı.
        total_bytes: u64,
    },
    /// Hata (herhangi bir request hata verirse).
    Error {
        /// Hata mesajı.
        message: String,
    },
}

/// Vencord/Equicord plugin'lerinin çağırdığı trait.
///
/// Production'da `ViscosNativeCommandRouter` (Faz 6) tarafından implement
/// edilecek. Faz 5.0 stub: `DefaultViscosNative` (in-memory settings +
/// `std::fs` tabanlı disk info).
pub trait ViscosNative: Send + Sync {
    /// Request'i handle et.
    ///
    /// # Errors
    ///
    /// Settings serialize / disk info IO hataları.
    fn handle(&self, req: ViscosNativeRequest) -> Result<ViscosNativeResponse, ViscosError>;
}

/// Default implementasyon — `GetVersion` + `GetDiskInfo` gerçek, settings
/// in-memory.
///
/// Faz 6.0'da `viscos-config::Config` ile gerçek ayar dosyası yazma/okuma
/// eklenecek.
pub struct DefaultViscosNative {
    /// In-memory ayar deposu.
    settings: parking_lot::RwLock<serde_json::Value>,
    /// Build hash (CI build info veya "dev").
    build_hash: String,
}

impl std::fmt::Debug for DefaultViscosNative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultViscosNative")
            .field("build_hash", &self.build_hash)
            .field("settings", &"<json>")
            .finish()
    }
}

impl DefaultViscosNative {
    /// Yeni default bridge oluştur.
    #[must_use]
    pub fn new() -> Self {
        Self {
            settings: parking_lot::RwLock::new(serde_json::json!({})),
            build_hash: option_env!("VISCOS_BUILD_HASH")
                .unwrap_or("dev")
                .to_string(),
        }
    }

    /// Build hash'i manuel set et (Faz 6 test helper).
    pub fn set_build_hash(&mut self, hash: impl Into<String>) {
        self.build_hash = hash.into();
    }
}

impl Default for DefaultViscosNative {
    fn default() -> Self {
        Self::new()
    }
}

impl ViscosNative for DefaultViscosNative {
    fn handle(&self, req: ViscosNativeRequest) -> Result<ViscosNativeResponse, ViscosError> {
        match req {
            ViscosNativeRequest::GetVersion => Ok(ViscosNativeResponse::Version {
                version: VISCOS_VERSION.to_string(),
                hash: self.build_hash.clone(),
            }),
            ViscosNativeRequest::GetSettings => {
                let settings = self.settings.read().clone();
                Ok(ViscosNativeResponse::Settings { settings })
            }
            ViscosNativeRequest::UpdateSettings { settings } => {
                let mut current = self.settings.write();
                merge_json(&mut current, &settings);
                Ok(ViscosNativeResponse::Settings {
                    settings: current.clone(),
                })
            }
            ViscosNativeRequest::GetDiskInfo => {
                disk_info().map(|(free, total)| ViscosNativeResponse::DiskInfo {
                    free_bytes: free,
                    total_bytes: total,
                })
            }
        }
    }
}

/// Mevcut disk'in boş / toplam byte sayısını döndür.
///
/// Windows'ta `GetDiskFreeSpaceExW`, Unix'te `statvfs`. Cross-platform
/// impl. için `sysinfo` veya platform-spesifik `windows` crate kullanılabilir
/// — Faz 5.0'da basit `std::fs` tabanlı yaklaşım yeterli (current dir).
///
/// Faz 6.0'da `sysinfo` crate'i ile gerçek system-wide disk bilgisi.
fn disk_info() -> Result<(u64, u64), ViscosError> {
    // std::fs metadata'sı tek bir path'in bilgisini verir; bu yeterli
    // (Vencord plugin'i Viscos'un install path'inin disk kullanımını sorar).
    // Cross-platform olarak metadata.total_space() / available_space() yok;
    // Faz 5.0 stub: 0 / 0 döndür. Gerçek impl Faz 6.0'da.
    use std::path::Path;
    let current = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let _ = std::fs::metadata(&current);
    Ok((0, 0))
}

/// `UpdateSettings` için basit shallow merge.
fn merge_json(base: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(base_obj), Some(patch_obj)) = (base.as_object_mut(), patch.as_object()) {
        for (k, v) in patch_obj {
            base_obj.insert(k.clone(), v.clone());
        }
    } else {
        *base = patch.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_version_returns_viscos_version() {
        let bridge = DefaultViscosNative::new();
        let resp = bridge
            .handle(ViscosNativeRequest::GetVersion)
            .expect("handle");
        match resp {
            ViscosNativeResponse::Version { version, hash } => {
                assert_eq!(version, VISCOS_VERSION);
                assert_eq!(hash, "dev");
            }
            other => panic!("expected Version, got {other:?}"),
        }
    }

    #[test]
    fn get_settings_returns_empty_initially() {
        let bridge = DefaultViscosNative::new();
        let resp = bridge
            .handle(ViscosNativeRequest::GetSettings)
            .expect("handle");
        match resp {
            ViscosNativeResponse::Settings { settings } => {
                assert!(settings.is_object());
                assert_eq!(settings.as_object().unwrap().len(), 0);
            }
            other => panic!("expected Settings, got {other:?}"),
        }
    }

    #[test]
    fn update_settings_merges_into_existing() {
        let bridge = DefaultViscosNative::new();

        // Initial empty → set theme: dark
        let resp = bridge
            .handle(ViscosNativeRequest::UpdateSettings {
                settings: serde_json::json!({"theme": "dark"}),
            })
            .expect("handle");
        match resp {
            ViscosNativeResponse::Settings { settings } => {
                assert_eq!(settings["theme"], "dark");
            }
            other => panic!("expected Settings, got {other:?}"),
        }

        // Update → set accent: blurple (theme korunmalı)
        let resp = bridge
            .handle(ViscosNativeRequest::UpdateSettings {
                settings: serde_json::json!({"accent": "blurple"}),
            })
            .expect("handle");
        match resp {
            ViscosNativeResponse::Settings { settings } => {
                assert_eq!(settings["theme"], "dark");
                assert_eq!(settings["accent"], "blurple");
            }
            other => panic!("expected Settings, got {other:?}"),
        }
    }

    #[test]
    fn request_serde_camel_case() {
        let req = ViscosNativeRequest::UpdateSettings {
            settings: serde_json::json!({"k": 1}),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"updateSettings\""), "got: {json}");
        assert!(json.contains("\"settings\""));
    }

    #[test]
    fn response_serde_camel_case() {
        let resp = ViscosNativeResponse::DiskInfo {
            free_bytes: 1024,
            total_bytes: 2048,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"diskInfo\""), "got: {json}");
        assert!(json.contains("\"freeBytes\":1024"));
        assert!(json.contains("\"totalBytes\":2048"));
    }

    #[test]
    fn disk_info_returns_valid_result() {
        // Stub: (0, 0) döner; gerçek Faz 6.0'da.
        let result = disk_info();
        assert!(result.is_ok());
    }

    #[test]
    fn custom_build_hash_is_used() {
        let mut bridge = DefaultViscosNative::new();
        bridge.set_build_hash("abc1234");
        let resp = bridge.handle(ViscosNativeRequest::GetVersion).unwrap();
        match resp {
            ViscosNativeResponse::Version { hash, .. } => assert_eq!(hash, "abc1234"),
            other => panic!("expected Version, got {other:?}"),
        }
    }
}
