//! Phase 6.1 row 6 — AES-GCM round-trip + tamper detection + AAD tamper.

use sol_wallet_core::crypto::{self, EncryptedBlob};
use sol_wallet_core::Error;
use zeroize::Zeroizing;

const PAYLOAD: &[u8] = b"the quick brown fox jumps over the lazy dog";
const PASSWORD: &str = "passw0rd!";

fn encrypt(payload: &[u8]) -> EncryptedBlob {
    crypto::encrypt_wallet(Zeroizing::new(payload.to_vec()), PASSWORD).expect("encrypt")
}

#[test]
fn round_trip_preserves_bytes() {
    let blob = encrypt(PAYLOAD);
    let out = crypto::decrypt_wallet(&blob, PASSWORD).expect("decrypt");
    assert_eq!(out.as_slice(), PAYLOAD);
}

#[test]
fn wrong_password_errors() {
    let blob = encrypt(PAYLOAD);
    let err = crypto::decrypt_wallet(&blob, "nope").unwrap_err();
    assert!(matches!(err, Error::WalletDecryptFailed { .. }));
}

#[test]
fn unsupported_version_rejected() {
    let mut blob = encrypt(PAYLOAD);
    blob.version = 99;
    let err = crypto::decrypt_wallet(&blob, PASSWORD).unwrap_err();
    assert!(matches!(err, Error::UnsupportedBlobVersion { .. }));
}

#[test]
fn aad_tamper_memory_kb_errors_p6_1() {
    // Audit P6-1 loud-RED gate: flip one byte of memory_kb in JSON
    // envelope → decrypt must FAIL (AAD bound to params).
    let mut blob = encrypt(PAYLOAD);
    blob.kdf_memory_kb += 1024;
    let err = crypto::decrypt_wallet(&blob, PASSWORD).unwrap_err();
    assert!(matches!(err, Error::WalletDecryptFailed { .. }));
}

#[test]
fn aad_tamper_salt_errors_p6_1() {
    let mut blob = encrypt(PAYLOAD);
    let mut salt_chars: Vec<char> = blob.kdf_salt.chars().collect();
    salt_chars[0] = if salt_chars[0] == '0' { '1' } else { '0' };
    blob.kdf_salt = salt_chars.into_iter().collect();
    let err = crypto::decrypt_wallet(&blob, PASSWORD).unwrap_err();
    assert!(matches!(err, Error::WalletDecryptFailed { .. }));
}
