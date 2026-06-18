//! AES-256-GCM round-trip + nonce uniqueness tests.

use viscos_media::MediaKey;

#[test]
fn round_trip_decrypts_to_plaintext() {
    let key_bytes = MediaKey::generate();
    let key = MediaKey::from_bytes(key_bytes);
    let plaintext = b"hello viscos media cache";
    let blob = key.encrypt(plaintext).expect("encrypt");
    let decrypted = key.decrypt(&blob).expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

#[test]
fn nonce_uniqueness_across_encryptions() {
    let key_bytes = MediaKey::generate();
    let key = MediaKey::from_bytes(key_bytes);
    let b1 = key.encrypt(b"same").expect("encrypt");
    let b2 = key.encrypt(b"same").expect("encrypt");
    assert_ne!(b1.nonce, b2.nonce, "nonce must be unique per encryption");
}

#[test]
fn wrong_key_fails_to_decrypt() {
    let k1 = MediaKey::from_bytes(MediaKey::generate());
    let k2 = MediaKey::from_bytes(MediaKey::generate());
    let blob = k1.encrypt(b"secret").expect("encrypt");
    assert!(
        k2.decrypt(&blob).is_err(),
        "decryption with wrong key must fail"
    );
}
