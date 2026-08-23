//! V2 verification — `MnemonicBuilder` returns a `PrivateKeySigner` whose
//! `.address()` matches the expected Ethereum address for the BIP-39 test
//! mnemonic at derivation path `m/44'/60'/0'/0/0` (Ledger default, Q3).
//!
//! Issue #293 — verification item V2.
//!
//! Deterministic: no network, no I/O. Always runs.

use alloy_primitives::Address;
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner};

/// BIP-39 test mnemonic (12-word, English). Canonical Ethereum test vector —
/// 11× `abandon` + `about` (BIP-39 wordlist's 12th word is `about`, not
/// `abandon`). Every wallet that follows SLIP-44 + BIP-44 must derive the
/// same address at `m/44'/60'/0'/0/0`.
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// BIP-39 spec test vector 2 (entropy `0x7f7f7f7f...`). Valid BIP-39 phrase
/// with valid checksum; derives a different address at `m/44'/60'/0'/0/0`
/// than `TEST_MNEMONIC`. Used by `v2_mnemonic_builder_wrong_phrase_yields_different_address`
/// to catch regressions where MnemonicBuilder silently reuses cached state.
const DIVERGENT_MNEMONIC: &str =
    "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";

/// Expected address at `m/44'/60'/0'/0/0` (account 0, address index 0).
/// Verified against MetaMask, Ledger Live, MyEtherWallet, ethers-rs, and
/// alloy v1.8.3 reference output. If this test fails after an alloy upgrade,
/// re-derive via `cast wallet derive "$TEST_MNEMONIC" --path "m/44'/60'/0'/0/0"`.
const EXPECTED_ADDRESS: &str = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";

#[test]
fn v2_mnemonic_builder_derives_expected_address() {
    let signer: PrivateKeySigner = MnemonicBuilder::english()
        .phrase(TEST_MNEMONIC)
        .index(0) // default path m/44'/60'/0'/0/0
        .expect("valid account index")
        .build()
        .expect("build signer");

    let addr: Address = signer.address();
    let actual = format!("{addr}");
    // alloy's `Address` Display returns EIP-55 checksummed form; expected
    // constant is also checksummed — compare directly (no lowercase).
    let expected = EXPECTED_ADDRESS;

    assert_eq!(
        actual, expected,
        "MnemonicBuilder derived {actual} but expected {expected} at m/44'/60'/0'/0/0",
    );

    eprintln!("[V2] PASS — mnemonic → address matches expected {expected}");
}

#[test]
fn v2_mnemonic_builder_address_is_deterministic() {
    // Sanity: same mnemonic → same address across two independent builds.
    let s1: PrivateKeySigner = MnemonicBuilder::english()
        .phrase(TEST_MNEMONIC)
        .index(0)
        .expect("valid account index")
        .build()
        .expect("build signer");
    let s2: PrivateKeySigner = MnemonicBuilder::english()
        .phrase(TEST_MNEMONIC)
        .index(0)
        .expect("valid account index")
        .build()
        .expect("build signer");
    assert_eq!(s1.address(), s2.address());
}

#[test]
fn v2_mnemonic_builder_wrong_phrase_yields_different_address() {
    // Catches regressions where MnemonicBuilder silently reuses cached state
    // instead of re-deriving from the new phrase.
    let real: PrivateKeySigner = MnemonicBuilder::english()
        .phrase(TEST_MNEMONIC)
        .index(0)
        .expect("valid account index")
        .build()
        .expect("build signer");
    let divergent: PrivateKeySigner = MnemonicBuilder::english()
        .phrase(DIVERGENT_MNEMONIC)
        .index(0)
        .expect("valid account index")
        .build()
        .expect("build signer");
    assert_ne!(
        real.address(),
        divergent.address(),
        "divergent mnemonic must derive a different address at m/44'/60'/0'/0/0",
    );
}

#[test]
fn v2_mnemonic_builder_drop_does_not_panic() {
    // F47 zeroize gap mirror: construct, query address, let the signer drop.
    // alloy's `PrivateKeySigner` does NOT impl `Zeroize` directly (deep-dive
    // §`alloy-signer-local` risks — Q7 deferred to eth/ crate). This test
    // only proves the drop path runs without panic; full zeroize audit belongs
    // in eth/ crate Phase 1 Task 2.
    let signer: PrivateKeySigner = MnemonicBuilder::english()
        .phrase(TEST_MNEMONIC)
        .index(0)
        .expect("valid account index")
        .build()
        .expect("build signer");
    let _addr = signer.address();
    drop(signer);
    eprintln!("[V2] drop-does-not-panic: PASS (full F47 audit deferred to eth/ crate)");
}
