//! MFA backup code integration test (ADR-0011 §4.1).
//!
//! 10 adet 8-karakterli alphanumeric backup code üretimi + format validasyonu
//! + v1 inert hash fonksiyonu (v2.0'da Argon2 PHC).

use viscos_auth::mfa::{
    generate_backup_codes, hash_backup_code, is_valid_backup_code_format, verify_backup_code,
};

#[test]
fn issue_10_valid_codes() {
    let codes = generate_backup_codes();
    assert_eq!(codes.len(), 10);
    for c in &codes {
        assert!(is_valid_backup_code_format(c), "invalid format: {c}");
    }
}

#[test]
fn codes_are_unique() {
    let codes = generate_backup_codes();
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(unique.len(), 10, "duplicate backup codes generated");
}

#[test]
fn hash_verify_roundtrip_v1_inert() {
    // v1: hash = plaintext, verify = constant-time compare.
    let code = "ABCD-1234";
    let hash = hash_backup_code(code);
    assert_eq!(hash, code, "v1 hash should be identity");
    assert!(verify_backup_code(code, &hash));
    assert!(!verify_backup_code("OTHER-CODE", &hash));
}

#[test]
fn backup_code_alphabet_is_uppercase_alphanumeric() {
    // Tüm kodlar büyük harf + rakam + opsiyonel tek tire'dan oluşmalı.
    let codes = generate_backup_codes();
    for c in &codes {
        let chars: Vec<char> = c.chars().collect();
        assert_eq!(chars.len(), 9, "Discord format xxxx-xxxx: {c}"); // 8 + 1 tire
        for (i, ch) in chars.iter().enumerate() {
            if i == 4 {
                assert_eq!(*ch, '-', "tire pozisyon 4: {c}");
            } else {
                assert!(
                    ch.is_ascii_uppercase() || ch.is_ascii_digit(),
                    "non-[A-Z0-9] char: {ch} in {c}"
                );
            }
        }
    }
}
