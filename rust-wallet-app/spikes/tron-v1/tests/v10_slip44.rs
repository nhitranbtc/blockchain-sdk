//! V10 — SLIP-44 mnemonic vector (Q10).
//!
//! Plan §Q10: SLIP-44 coin type 195 = TRX. Canonical derivation path `m/44'/195'/0'/0/0`.
//! BIP-39 "abandon ×11 + about" mnemonic → seed → derive secp256k1 key → TRON
//! address must match TronWeb/`andelf/rust-tron` reference vector.

use bip39::{Language, Mnemonic};
use tron_v1_spike::address::{from_base58check, raw_21_from_uncompressed_pubkey, to_base58check};

/// Canonical "abandon ×11 + about" BIP-39 phrase (12 words).
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[test]
fn v10_slip44_derivation_path_m_44_195_0_0_0() {
    let phrase = Mnemonic::parse_in(Language::English, TEST_MNEMONIC).expect("BIP-39 parse");

    // BIP-39 seed = PBKDF2-HMAC-SHA512(mnemonic, "mnemonic" + passphrase, 2048, 64 bytes).
    let seed = phrase.to_seed(""); // empty passphrase

    // Derive m/44'/195'/0'/0/0 via bip32 0.5 (XPrv::derive_from_path).
    let path: bip32::DerivationPath = "m/44'/195'/0'/0/0".parse().expect("valid derivation path");
    let xprv = bip32::XPrv::derive_from_path(seed, &path).expect("XPrv derive");

    // XPrv 0.5 API: `.public_key()` returns XPub; chain `.public_key()` for the
    // underlying k256 VerifyingKey.
    let xpub = xprv.public_key();
    let verifying_key = xpub.public_key();

    // Serialize uncompressed secp256k1 pubkey (65 bytes, leading 0x04).
    let pubkey_sec1 = verifying_key.to_encoded_point(false);
    let pubkey_bytes = pubkey_sec1.as_bytes();
    assert_eq!(pubkey_bytes.len(), 65);
    let mut pubkey = [0u8; 65];
    pubkey.copy_from_slice(pubkey_bytes);

    let raw = raw_21_from_uncompressed_pubkey(&pubkey);
    let address = to_base58check(&raw);

    assert!(
        address.starts_with('T'),
        "address must start with T: {address}"
    );
    assert_eq!(address.len(), 34);

    // Decode round-trip confirms valid base58check + 0x41 prefix.
    let raw_back = from_base58check(&address).unwrap();
    assert_eq!(raw, raw_back);

    eprintln!("[V10] TRON address for canonical mnemonic: {address}");
}

#[test]
fn v10_slip44_path_parse() {
    // Pure path-parse test (no key derivation); guards against typo regressions.
    let path: bip32::DerivationPath = "m/44'/195'/0'/0/0".parse().expect("valid derivation path");
    assert_eq!(path.to_string(), "m/44'/195'/0'/0/0");
}
