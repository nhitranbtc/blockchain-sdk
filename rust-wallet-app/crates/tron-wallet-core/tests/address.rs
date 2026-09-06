//! Address round-trips, validation, and xpub export.
//!
//! Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`
//! Phase 1 Tasks 1.3 and 1.6.

use tron_wallet_core::address::Address;
use tron_wallet_core::keys::{derive_keypair, xpub, Language, Mnemonic};

const CANONICAL_PHRASE: &str = "abandon abandon abandon abandon abandon abandon \
     abandon abandon abandon abandon abandon about";

const TRON_PATH: &str = "m/44'/195'/0'/0/0";

/// Published `anychain-tron` fixture (`src/address.rs` unit tests): the raw 21
/// bytes `4196a3bace5adacf637eb7cc79d5787f4247da4bbe` render as this address.
const KNOWN_ADDRESS: &str = "TPhiVyQZ5xyvVK2KS2LTke8YvXJU5wxnbN";
const KNOWN_ADDRESS_HEX: &str = "4196a3bace5adacf637eb7cc79d5787f4247da4bbe";

fn canonical_mnemonic() -> Mnemonic {
    Mnemonic::from_phrase(CANONICAL_PHRASE, Language::English).expect("phrase must parse")
}

#[test]
fn parses_a_known_base58_address() {
    let address: Address = KNOWN_ADDRESS.parse().expect("known address must parse");

    assert_eq!(address.to_base58(), KNOWN_ADDRESS);
    assert_eq!(address.to_hex().to_lowercase(), KNOWN_ADDRESS_HEX);
}

#[test]
fn parses_the_hex_form_of_a_known_address() {
    let from_hex: Address = KNOWN_ADDRESS_HEX.parse().expect("hex form must parse");
    let from_base58: Address = KNOWN_ADDRESS.parse().expect("base58 form must parse");

    assert_eq!(from_hex, from_base58);
}

#[test]
fn derived_address_round_trips() {
    let keypair = derive_keypair(&canonical_mnemonic(), "", &TRON_PATH.parse().expect("path"))
        .expect("derivation must succeed");
    let address = Address::from_public_key(keypair.public_key()).expect("address must derive");

    let parsed: Address = address.to_base58().parse().expect("round-trip must parse");
    assert_eq!(parsed, address);
}

#[test]
fn is_valid_accepts_well_formed_addresses() {
    assert!(Address::is_valid(KNOWN_ADDRESS));
    assert!(Address::is_valid(KNOWN_ADDRESS_HEX));
}

#[test]
fn is_valid_rejects_malformed_input() {
    // Empty, truncated, wrong alphabet, and a mutated payload.
    assert!(!Address::is_valid(""));
    assert!(!Address::is_valid("TPhiVyQZ5xyvVK2KS2LTke8YvXJU5wxnb"));
    assert!(!Address::is_valid("not a tron address at all, obviously"));

    let mut corrupted: Vec<char> = KNOWN_ADDRESS.chars().collect();
    corrupted[10] = if corrupted[10] == 'a' { 'b' } else { 'a' };
    let corrupted: String = corrupted.into_iter().collect();
    assert!(
        !Address::is_valid(&corrupted),
        "base58check must reject a mutated payload: {corrupted}"
    );
}

/// Bitcoin-style SLIP-0132 serialization, per Story 19. The account-level path
/// is what a watch-only companion imports, so that is what we export.
#[test]
fn xpub_export_is_slip0132_encoded() {
    let exported = xpub(
        &canonical_mnemonic(),
        "",
        &"m/44'/195'/0'".parse().expect("path"),
    )
    .expect("xpub export must succeed");

    assert!(
        exported.starts_with("xpub"),
        "expected SLIP-0132 xpub prefix, got {exported}"
    );
}

#[test]
fn xpub_differs_per_account() {
    let account_0 = xpub(
        &canonical_mnemonic(),
        "",
        &"m/44'/195'/0'".parse().expect("path"),
    )
    .expect("xpub export must succeed");
    let account_1 = xpub(
        &canonical_mnemonic(),
        "",
        &"m/44'/195'/1'".parse().expect("path"),
    )
    .expect("xpub export must succeed");

    assert_ne!(account_0, account_1);
}
