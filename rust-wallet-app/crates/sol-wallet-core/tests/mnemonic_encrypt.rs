//! Phase 6.1 row 7 — mnemonic encrypt-at-rest round-trip + version
//! discriminator + no plaintext leak.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sol_wallet_core::crypto;
use sol_wallet_core::Error;
use zeroize::Zeroizing;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const PASSWORD: &str = "hunter2";

#[test]
fn encrypt_decrypt_mnemonic_round_trip() {
    let blob = crypto::encrypt_wallet(Zeroizing::new(MNEMONIC.as_bytes().to_vec()), PASSWORD)
        .expect("encrypt");
    let out = crypto::decrypt_wallet(&blob, PASSWORD).expect("decrypt");
    let s = std::str::from_utf8(&out).expect("utf8 mnemonic");
    assert_eq!(s, MNEMONIC);
}

#[test]
fn wrong_passphrase_errors() {
    let blob = crypto::encrypt_wallet(Zeroizing::new(MNEMONIC.as_bytes().to_vec()), PASSWORD)
        .expect("encrypt");
    let err = crypto::decrypt_wallet(&blob, "wrong").unwrap_err();
    assert!(matches!(err, Error::WalletDecryptFailed { .. }));
}

#[test]
fn encrypted_blob_does_not_contain_plaintext_mnemonic_substring() {
    let blob = crypto::encrypt_wallet(Zeroizing::new(MNEMONIC.as_bytes().to_vec()), PASSWORD)
        .expect("encrypt");
    let json = serde_json::to_string(&blob).expect("serialize");
    assert!(!json.contains("abandon"));
    let bytes = B64.decode(&blob.encrypted_payload_b64).expect("b64");
    let plaintext_bytes = MNEMONIC.as_bytes();
    for window in plaintext_bytes.windows(7) {
        assert!(
            !bytes.windows(window.len()).any(|w| w == window),
            "mnemonic substring leaked into ciphertext"
        );
    }
}

#[test]
fn version_two_rejected_p6_10() {
    let mut blob = crypto::encrypt_wallet(Zeroizing::new(MNEMONIC.as_bytes().to_vec()), PASSWORD)
        .expect("encrypt");
    blob.version = 2;
    let err = crypto::decrypt_wallet(&blob, PASSWORD).unwrap_err();
    assert!(matches!(
        err,
        Error::UnsupportedBlobVersion { found: 2, .. }
    ));
}

#[test]
fn rng_path_no_unwrap_in_crypto_module() {
    // P6-7 — CI grep audit; asserts the surface stays clean.
    let src = std::fs::read_to_string("src/crypto.rs").expect("read");
    let offenders: Vec<_> = src
        .lines()
        .filter(|l| {
            !l.trim_start().starts_with("//") && (l.contains(".unwrap()") || l.contains(".expect("))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "crypto.rs has RNG-path unwraps: {offenders:?}"
    );
}
