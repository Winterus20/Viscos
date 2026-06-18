//! Keyring round-trip integration test (ADR-0011 §6).
//!
//! **CI davranışı:** CI runner'da `windows-native-keyring-store` çalışmaz
//! (sandboxed container). Bu yüzden test `cfg(target_os = "windows")` ve
//! env kontrolü ile koşullu. Normal `cargo test` Windows dev makinede geçer.

#![cfg(target_os = "windows")]

use std::time::{SystemTime, UNIX_EPOCH};

use secrecy::ExposeSecret;
use viscos_auth::{
    AuthStorage,
    storage::{AuthError, StoredAccount},
};

/// Yeni, unique bir user_id üret (her test run'ında izole).
fn unique_user_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("test-user-{}", nanos)
}

#[test]
fn round_trip_store_load_delete() {
    // Install — Windows DPAPI store.
    AuthStorage::install().expect("install keyring");

    let storage = AuthStorage::new();
    let user_id = unique_user_id();
    let secret_token = format!("real-dpapi-token-{}", user_id);

    let account = StoredAccount {
        user_id: user_id.clone(),
        username: "round-trip-tester".to_string(),
        token: secrecy::SecretString::new(secret_token.clone().into_boxed_str()),
        mfa_backup_hashes: vec![],
        super_properties: None,
        created_at: 1_700_000_000,
        last_validated_at: 1_700_000_000,
    };

    // Store
    storage.store_account(&account).expect("store");

    // Load — geri gelen token orijinaliyle birebir eşleşmeli.
    let loaded = storage
        .load_account(&user_id)
        .expect("load ok")
        .expect("account exists");
    assert_eq!(loaded.user_id, user_id);
    assert_eq!(loaded.username, "round-trip-tester");
    assert_eq!(loaded.token.expose_secret(), secret_token);
    assert_eq!(loaded.created_at, 1_700_000_000);

    // Delete + load → None.
    storage.delete_account(&user_id).expect("delete");
    let after = storage.load_account(&user_id).expect("load post-delete");
    assert!(after.is_none());
}

#[test]
fn delete_nonexistent_is_noop() {
    AuthStorage::install().expect("install keyring");
    let storage = AuthStorage::new();
    let phantom = unique_user_id();
    let result: Result<(), AuthError> = storage.delete_account(&phantom);
    // Yoksa silently no-op; NoEntry handle ediliyor (storage.rs).
    assert!(
        result.is_ok(),
        "delete on nonexistent should be Ok: {result:?}"
    );
}

#[test]
fn load_nonexistent_returns_none() {
    AuthStorage::install().expect("install keyring");
    let storage = AuthStorage::new();
    let phantom = unique_user_id();
    let result = storage.load_account(&phantom).expect("load ok");
    assert!(result.is_none());
}
