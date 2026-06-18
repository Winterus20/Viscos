//! Cross-module integration tests — `viscos-shell::native::native_bridge`
//! `ViscosNative` request/response round-trip.

use viscos_shell::native::native_bridge::{
    DefaultViscosNative, ViscosNative, ViscosNativeRequest, ViscosNativeResponse,
};

#[test]
fn get_version_returns_viscos_version_string() {
    let bridge = DefaultViscosNative::new();
    let resp = bridge
        .handle(ViscosNativeRequest::GetVersion)
        .expect("handle");
    match resp {
        ViscosNativeResponse::Version { version, hash } => {
            assert!(!version.is_empty(), "version string should be non-empty");
            assert!(!hash.is_empty(), "hash should be non-empty");
        }
        other => panic!("expected Version, got {other:?}"),
    }
}

#[test]
fn get_disk_info_returns_valid_bytes() {
    let bridge = DefaultViscosNative::new();
    let resp = bridge
        .handle(ViscosNativeRequest::GetDiskInfo)
        .expect("handle");
    match resp {
        ViscosNativeResponse::DiskInfo {
            free_bytes,
            total_bytes,
        } => {
            // Stub: 0/0 döner. Gerçek Faz 6.0'da.
            assert_eq!(free_bytes, 0);
            assert_eq!(total_bytes, 0);
        }
        other => panic!("expected DiskInfo, got {other:?}"),
    }
}

#[test]
fn settings_round_trip() {
    let bridge = DefaultViscosNative::new();
    let resp = bridge
        .handle(ViscosNativeRequest::UpdateSettings {
            settings: serde_json::json!({"theme": "dark", "accent": "blurple"}),
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
fn request_serde_uses_camel_case() {
    let req = ViscosNativeRequest::UpdateSettings {
        settings: serde_json::json!({}),
    };
    let json = serde_json::to_string(&req).unwrap();
    // tag = "type", rename_all = "camelCase" → "updateSettings"
    assert!(json.contains("\"type\":\"updateSettings\""), "got: {json}");
}

#[test]
fn response_serde_field_renamed_to_camel_case() {
    let resp = ViscosNativeResponse::DiskInfo {
        free_bytes: 1024,
        total_bytes: 2048,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"freeBytes\":1024"), "got: {json}");
    assert!(json.contains("\"totalBytes\":2048"), "got: {json}");
    assert!(json.contains("\"type\":\"diskInfo\""), "got: {json}");
}

#[test]
fn build_hash_override_is_reflected_in_version() {
    let mut bridge = DefaultViscosNative::new();
    bridge.set_build_hash("deadbeef");
    let resp = bridge.handle(ViscosNativeRequest::GetVersion).unwrap();
    match resp {
        ViscosNativeResponse::Version { hash, .. } => assert_eq!(hash, "deadbeef"),
        other => panic!("expected Version, got {other:?}"),
    }
}
