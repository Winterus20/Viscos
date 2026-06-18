//! X-Super-Properties integration test (ADR-0011 §5.2).
//!
//! Discord Web client fingerprint — Tüm beklenen alanların JSON'da
//! bulunduğunu doğrular.

use viscos_auth::super_properties::{
    BROWSER_IDENTITY, DEFAULT_BUILD_NUMBER, build_x_super_properties,
    build_x_super_properties_header,
};

#[test]
fn build_includes_all_required_fields() {
    let json = build_x_super_properties(DEFAULT_BUILD_NUMBER);
    let obj = json.as_object().expect("must be JSON object");

    // Zorunlu alanlar.
    for field in [
        "os",
        "browser",
        "device",
        "system_locale",
        "browser_user_agent",
        "browser_version",
        "os_version",
        "referrer",
        "referring_domain",
        "referrer_current",
        "referring_domain_current",
        "release_channel",
        "client_build_number",
        "client_event_source",
        "has_client_mods",
    ] {
        assert!(obj.contains_key(field), "missing field: {field}");
    }
}

#[test]
fn os_is_windows_string() {
    let json = build_x_super_properties(DEFAULT_BUILD_NUMBER);
    let os = json.get("os").and_then(|v| v.as_str()).expect("os string");
    assert_eq!(os, "Windows");
    // BROWSER_IDENTITY ayrı bir sabit; super-properties'de kullanılır.
    assert_eq!(BROWSER_IDENTITY, "Viscos");
}

#[test]
fn client_build_number_is_number() {
    let json = build_x_super_properties(DEFAULT_BUILD_NUMBER);
    let bn = json
        .get("client_build_number")
        .and_then(|v| v.as_u64())
        .expect("build_number u64");
    assert_eq!(bn, DEFAULT_BUILD_NUMBER);
}

#[test]
fn has_client_mods_is_false() {
    // v1: Viscos herhangi bir mod enjekte etmiyor.
    let json = build_x_super_properties(DEFAULT_BUILD_NUMBER);
    let mods = json
        .get("has_client_mods")
        .and_then(|v| v.as_bool())
        .expect("has_client_mods bool");
    assert!(!mods);
}

#[test]
fn header_is_valid_base64() {
    let header = build_x_super_properties_header();
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&header)
        .expect("header must be valid base64");
    let json: serde_json::Value =
        serde_json::from_slice(&decoded).expect("decoded header must be valid JSON");
    assert!(json.get("browser").is_some());
}
