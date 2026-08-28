//! V2 mirror test — derive known ETH address from "abandon"*11 + "about" mnemonic.
//!
//! Per Plan Task 1 step 4. Spike V2 evidence:
//! `rust-wallet-app/spikes/alloy-v1/tests/v2_mnemonic.rs`.

use alloy_primitives::Address;
use bip39::{Language, Mnemonic};
use eth_wallet_core::mnemonic::derive_address;

/// 12-word "abandon" mnemonic — BIP-39 reference test vector.
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Known ETH address for the all-abandon mnemonic at `m/44'/60'/0'/0/0`
/// (Ledger-style path per Q3 resolution).
///
/// Source: spike V2 evidence (commit `08d83a0`); cross-validates against
/// the Test Vector in the official BIP-39 wordlist for entropy
/// `00000000000000000000000000000000`.
const EXPECTED_ADDR: &str = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";

#[test]
fn v2_mirror_derive_address_at_index_zero() {
    let phrase = Mnemonic::parse_in(Language::English, TEST_MNEMONIC)
        .expect("TEST_MNEMONIC must be a valid BIP-39 phrase");
    let derived = derive_address(&phrase, 0);
    let expected: Address = EXPECTED_ADDR
        .parse()
        .expect("hardcoded expected Address must parse");
    assert_eq!(
        derived, expected,
        "V2 mirror: all-abandon mnemonic at m/44'/60'/0'/0/0 must equal the known ETH address"
    );
}

#[test]
fn v2_mirror_distinct_indices_produce_distinct_addresses() {
    let phrase = Mnemonic::parse_in(Language::English, TEST_MNEMONIC)
        .expect("TEST_MNEMONIC must be a valid BIP-39 phrase");
    let addr_0 = derive_address(&phrase, 0);
    let addr_1 = derive_address(&phrase, 1);
    assert_ne!(
        addr_0, addr_1,
        "distinct BIP-44 address indices must produce distinct addresses"
    );
}
