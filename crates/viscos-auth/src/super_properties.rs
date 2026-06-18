//! X-Super-Properties — Discord Web client fingerprint.
//!
//! **Amaç:** Discord sunucusu, REST ve Gateway request'lerinde
//! `X-Super-Properties` header'ını (base64-encoded JSON) bekler. Bu
//! fingerprint tarayıcı/istemci tipini, build number'ı, OS bilgilerini
//! taşır. Üçüncü parti client'lar için **stable fingerprint** önemli:
//! çok sık değişirse "client modified" heuristic tetiklenir.
//!
//! **Statik alanlar (Viscos identity):**
//! - `os = "Windows"`, `browser = "Viscos"`, `release_channel = "stable"`
//!
//! **Yarı-statik alanlar (haftalık GH Action sync):**
//! - `client_build_number` — `config.toml`'dan gelir (`auth.build_number`),
//!   haftalık GH Action ile Discord Web'in build_number'ıyla sync'lenir.
//!
//! **Cihaz-bağımlı alanlar (Faz 1.6 backend kararı sonrası):**
//! - WebGL hash → WebView2/CEF renderer'dan capture (Faz 1.6).
//!
//! **Sabit alanlar (Discord Web ile aynı görünmeli):**
//! - `browser_user_agent`, `browser_version`, `os_version`, `system_locale`,
//!   `timezone_offset_minutes`.
//!
//! **Not:** v1'de `browser_user_agent` hardcoded. İleride (Faz 5+ polish)
//! kullanıcının Edge sürümüne göre dinamik üretilebilir.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};

/// Viscos'un stable browser fingerprint kimliği.
pub const BROWSER_IDENTITY: &str = "Viscos";

/// Default `client_build_number`. Haftalık GH Action sync'lenene kadar
/// Discord Web'in bilinen en son build_number'ından daha küçük olmamalı.
pub const DEFAULT_BUILD_NUMBER: u64 = 360_000;

/// `X-Super-Properties` JSON üretir.
///
/// # Arguments
/// * `build_number` — Config'ten gelen (GH Action ile senkronlanan) build.
///
/// # Returns
/// `serde_json::Value` — caller (`viscos-api`) base64 encode edip header'a yazar.
#[must_use]
pub fn build_x_super_properties(build_number: u64) -> Value {
    json!({
        "os": "Windows",
        "browser": BROWSER_IDENTITY,
        "device": "",
        "system_locale": "en-US",
        "browser_user_agent":
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "browser_version": "120.0.0.0",
        "os_version": "10",
        "referrer": "",
        "referring_domain": "",
        "referrer_current": "",
        "referring_domain_current": "",
        "release_channel": "stable",
        "client_build_number": build_number,
        "client_event_source": Value::Null,
        "has_client_mods": false,
        "timezone_offset_minutes": -180,
        // WebGL/Canvas hash Faz 1.6 backend kararı sonrası CEF/WebView2'den
        // capture edilip buraya inject edilecek. Şimdilik stable stub.
        "webgl_hash": "stub-pending-backend-decision",
    })
}

/// `X-Super-Properties` JSON'ı base64-encode edip header value olarak üretir.
///
/// Bu helper `viscos-api` tarafından her REST request'inde header olarak
/// eklenir. Default `client_build_number` ile.
#[must_use]
pub fn build_x_super_properties_header() -> String {
    build_x_super_properties_header_with_build(DEFAULT_BUILD_NUMBER)
}

/// Build number ile X-Super-Properties base64 string üret.
#[must_use]
pub fn build_x_super_properties_header_with_build(build_number: u64) -> String {
    let json = build_x_super_properties(build_number);
    let bytes = serde_json::to_vec(&json).expect("super-properties serialization");
    BASE64.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn super_properties_contains_required_fields() {
        let sp = build_x_super_properties(DEFAULT_BUILD_NUMBER);
        // Discord'un parse ettiği zorunlu alanlar.
        assert_eq!(sp["os"], "Windows");
        assert_eq!(sp["browser"], BROWSER_IDENTITY);
        assert_eq!(sp["release_channel"], "stable");
        assert_eq!(sp["client_build_number"], DEFAULT_BUILD_NUMBER);
        assert_eq!(sp["has_client_mods"], false);
        assert!(
            sp["browser_user_agent"]
                .as_str()
                .expect("UA string")
                .contains("Mozilla")
        );
    }

    #[test]
    fn super_properties_is_json_object() {
        let sp = build_x_super_properties(DEFAULT_BUILD_NUMBER);
        assert!(sp.is_object());
        // 15+ beklenen alan.
        assert!(
            sp.as_object().expect("object").len() >= 15,
            "expected at least 15 fields"
        );
    }

    #[test]
    fn build_number_override_is_respected() {
        let sp = build_x_super_properties(420_000);
        assert_eq!(sp["client_build_number"], 420_000);
    }

    #[test]
    fn base64_header_roundtrip() {
        let header = build_x_super_properties_header_with_build(123_456);
        // base64 decode → JSON parse edilebilmeli.
        let decoded = BASE64.decode(header.as_bytes()).expect("base64 decode");
        let parsed: Value = serde_json::from_slice(&decoded).expect("json parse");
        assert_eq!(parsed["os"], "Windows");
        assert_eq!(parsed["client_build_number"], 123_456);
    }

    #[test]
    fn default_header_uses_default_build() {
        let header = build_x_super_properties_header();
        let decoded = BASE64.decode(header.as_bytes()).expect("decode");
        let parsed: Value = serde_json::from_slice(&decoded).expect("parse");
        assert_eq!(parsed["client_build_number"], DEFAULT_BUILD_NUMBER);
    }
}
