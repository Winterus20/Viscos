//! Captcha redirect integration test (ADR-0011 §7.1).
//!
//! Discord `/auth/login` hCaptcha döndüğünde `LoginResult::CaptchaRequired`
//! varyantıyla UI'a yansıtılır. Bu test `RawLoginResponse` JSON parse
//! yoluyla bu senaryoyu kanıtlar.

use viscos_auth::login::RawLoginResponse;

#[test]
fn captcha_response_parses_to_url() {
    let json = r#"{
        "kind": "captcha_required",
        "captcha_url": "https://discord.com/captcha?token=ABC&sid=42",
        "sitekey": "hcaptcha-sitekey-xyz",
        "rqtoken": "rqtoken-123"
    }"#;
    let parsed: RawLoginResponse = serde_json::from_str(json).expect("parse");
    match parsed {
        RawLoginResponse::CaptchaRequired {
            captcha_url,
            sitekey,
            rqtoken,
        } => {
            assert!(captcha_url.starts_with("https://discord.com/captcha"));
            assert!(sitekey.starts_with("hcaptcha-"));
            assert!(!rqtoken.is_empty());
        }
        other => panic!("expected captcha, got: {other:?}"),
    }
}

#[test]
fn success_response_parses() {
    let json = r#"{
        "kind": "success",
        "token": "MzI5.MQ.ABC.XYZ",
        "user_id": "111222333444555666",
        "username": "tester"
    }"#;
    let parsed: RawLoginResponse = serde_json::from_str(json).expect("parse");
    match parsed {
        RawLoginResponse::Success {
            token,
            user_id,
            username,
        } => {
            assert_eq!(token, "MzI5.MQ.ABC.XYZ");
            assert_eq!(user_id, "111222333444555666");
            assert_eq!(username, "tester");
        }
        other => panic!("expected success, got: {other:?}"),
    }
}

#[test]
fn mfa_response_parses() {
    let json = r#"{
        "kind": "mfa_required",
        "ticket": "ticket-abc-123",
        "mfa_type": "totp"
    }"#;
    let parsed: RawLoginResponse = serde_json::from_str(json).expect("parse");
    match parsed {
        RawLoginResponse::MfaRequired { ticket, mfa_type } => {
            assert_eq!(ticket, "ticket-abc-123");
            assert_eq!(mfa_type, "totp");
        }
        other => panic!("expected mfa, got: {other:?}"),
    }
}
