//! Secrecy audit — `expose_secret()` call site'ları static kontrol.
//!
//! **Prensip:** `secrecy::Secret*::expose_secret()` yalnızca gerekli yerlerde
//! çağrılmalıdır (network I/O, storage, UI). Bu test `viscos-auth` ve
//! `viscos-api` içindeki `expose_secret()` çağrılarını sayar ve mantıklı bir
//! üst sınırla karşılaştırır. **Mevcut sayı çok büyürse** (yeni secret
//! sızıntısı potansiyeli), test fail eder ve insan review'a düşer.

use std::fs;

#[test]
fn expose_secret_call_sites_within_budget() {
    // 1) viscos-auth/src
    let auth_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let auth_calls = count_expose_secret(auth_dir);

    // 2) viscos-api/src
    let api_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../viscos-api/src");
    let api_calls = count_expose_secret(api_dir);

    let total = auth_calls + api_calls;

    // Üst sınır: v1'de beklenen call site'ları:
    //  - mfa.rs: 2 (format validation + TOTP build)
    //  - storage.rs: 2 (store/load)
    //  - login.rs: 1 (test only)
    //  - rest.rs: 1 (expose_token helper)
    // Toplam ~6. Gelecek follow-up'lar için 12'lik tavan.
    assert!(
        total <= 12,
        "expose_secret() call site sayısı {total} (auth={auth_calls}, api={api_calls}) — bütçe aşımı, insan review gerekli"
    );
}

fn count_expose_secret(dir: &str) -> usize {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += count_expose_secret_dir(&path);
            }
        }
    }
    total
}

fn count_expose_secret_dir(path: &std::path::Path) -> usize {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += count_expose_secret_dir(&p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("rs")
                && let Ok(content) = fs::read_to_string(&p)
            {
                total += content.matches("expose_secret").count();
            }
        }
    }
    total
}
