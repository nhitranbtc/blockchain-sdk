//! Phase 6.1 row 5 — Argon2id determinism + parameter pinning + AAD bind.
//!
//! Plan §Phase 6 deep-dive row 5: Argon2id (memory 64MB, t=3, p=1) +
//! AES-GCM. Audit finding P6-5: same password + lower params → distinct
//! derived key. P6-1: AAD tamper breaks decrypt.

use sol_wallet_core::crypto::{self, EncryptedBlob, KdfParams};
use zeroize::Zeroizing;

const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const PASSWORD: &str = "correct horse battery staple";

#[test]
fn encrypt_then_decrypt_round_trips() {
    let plaintext = Zeroizing::new(PHRASE.as_bytes().to_vec());
    let blob = crypto::encrypt_wallet(plaintext.clone(), PASSWORD).expect("encrypt");
    let out = crypto::decrypt_wallet(&blob, PASSWORD).expect("decrypt");
    assert_eq!(out.as_slice(), plaintext.as_slice());
}

#[test]
fn envelope_has_version_one() {
    let plaintext = Zeroizing::new(PHRASE.as_bytes().to_vec());
    let blob = crypto::encrypt_wallet(plaintext, PASSWORD).expect("encrypt");
    assert_eq!(blob.version, crypto::ENVELOPE_VERSION);
}

#[test]
fn determinism_same_password_round_trips() {
    let plaintext1 = Zeroizing::new(b"payload-1".to_vec());
    let plaintext2 = Zeroizing::new(b"payload-2".to_vec());
    let blob1 = crypto::encrypt_wallet(plaintext1.clone(), PASSWORD).expect("e1");
    let blob2 = crypto::encrypt_wallet(plaintext2.clone(), PASSWORD).expect("e2");
    let out1 = crypto::decrypt_wallet(&blob1, PASSWORD).expect("d1");
    let out2 = crypto::decrypt_wallet(&blob2, PASSWORD).expect("d2");
    assert_eq!(out1.as_slice(), b"payload-1");
    assert_eq!(out2.as_slice(), b"payload-2");
}

#[test]
fn reject_wrong_params_produces_decrypt_failure() {
    // Encrypt with default params; tamper JSON to lower memory_kb;
    // AAD mismatch (audit P6-1) → decrypt fails.
    let plaintext = Zeroizing::new(PHRASE.as_bytes().to_vec());
    let mut blob: EncryptedBlob = crypto::encrypt_wallet(plaintext, PASSWORD).expect("encrypt");
    blob.kdf.memory_kb = 8 * 1024;
    let err = crypto::decrypt_wallet(&blob, PASSWORD).unwrap_err();
    assert!(matches!(
        err,
        sol_wallet_core::Error::WalletDecryptFailed { .. }
    ));
}

#[test]
fn kdf_params_constants_pin_desktop_and_mobile() {
    assert_eq!(KdfParams::DESKTOP.memory_kb, 64 * 1024);
    assert_eq!(KdfParams::DESKTOP.iterations, 3);
    assert_eq!(KdfParams::DESKTOP.parallelism, 1);
    assert_eq!(KdfParams::MOBILE.memory_kb, 16 * 1024);
}

#[test]
fn kdf_params_default_matches_platform() {
    let p = KdfParams::default();
    #[cfg(any(target_os = "ios", target_os = "android"))]
    assert_eq!(p, KdfParams::MOBILE);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    assert_eq!(p, KdfParams::DESKTOP);
}
