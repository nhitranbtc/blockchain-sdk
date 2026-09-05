//! Spike V10 — SLIP-44 canonical derivation vector.
//!
//! Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`
//! Phase 1 Task 1.4. Feeds the Phase 7 V10 PASS gate.
//!
//! # Where the expected values come from
//!
//! Neither the plan nor the deep-dive records a published TRON address for the
//! canonical `abandon x11 + about` mnemonic. Pinning a number this crate
//! produced and calling it a reference would make the test self-confirming, so
//! both constants below come from outside this crate:
//!
//! 1. [`CROSS_CHECKED_TRON_ADDRESS`] is what `spikes/tron-v1` derives for the
//!    same mnemonic and path using a completely separate stack — the `bip39`,
//!    `bip32`, and `k256` crates with hand-rolled base58check, sharing no code
//!    with `anychain-kms` or `anychain-tron`. Two independent implementations
//!    agreeing is the "hand-rolled cross-check" the deep-dive asks V10 for.
//! 2. [`CANONICAL_ETH_ACCOUNT`] is the Ethereum account this repository already
//!    asserts in five places (`evm-wallet-core`, `polygon-wallet-core`,
//!    `spikes/alloy-v1`, `spikes/polygon-v1`, `eth-wallet-core`). TRON and
//!    Ethereum hash an account identically — `keccak256(uncompressed
//!    pubkey[1..])[12..]` — and differ only in SLIP-44 coin index and the
//!    `0x41` display prefix, so reproducing it exercises the whole BIP-39 seed
//!    -> BIP-32 secp256k1 -> pubkey -> keccak chain.
//!
//! Together they mean a silent `anychain-kms` or `anychain-tron` derivation
//! change fails this file before it can reach a wallet.

use tron_wallet_core::address::Address;
use tron_wallet_core::keys::{derive_keypair, Language, Mnemonic};

/// BIP-39 vector mnemonic for all-zero entropy.
const CANONICAL_PHRASE: &str = "abandon abandon abandon abandon abandon abandon \
     abandon abandon abandon abandon abandon about";

/// TRON account 0 for [`CANONICAL_PHRASE`], derived independently by
/// `spikes/tron-v1/tests/v10_slip44.rs` (bip39 + bip32 + k256 + hand-rolled
/// base58check). Regenerate with:
///
/// ```text
/// cargo test -p tron-v1-spike --test v10_slip44 -- --nocapture
/// ```
const CROSS_CHECKED_TRON_ADDRESS: &str = "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH";

/// Ethereum account 0 for [`CANONICAL_PHRASE`], lowercase and without `0x`.
/// The rest of the repository asserts the mixed-case form
/// `0x9858EfFD232B4033E47d90003D41EC34EcaEda94`.
const CANONICAL_ETH_ACCOUNT: &str = "9858effd232b4033e47d90003d41ec34ecaeda94";

const TRON_PATH: &str = "m/44'/195'/0'/0/0";
const ETHEREUM_PATH: &str = "m/44'/60'/0'/0/0";

fn canonical_mnemonic() -> Mnemonic {
    Mnemonic::from_phrase(CANONICAL_PHRASE, Language::English)
        .expect("canonical BIP-39 phrase must parse")
}

fn address_at(path: &str) -> Address {
    let keypair = derive_keypair(
        &canonical_mnemonic(),
        "",
        &path.parse().expect("valid path"),
    )
    .expect("derivation must succeed");
    Address::from_public_key(keypair.public_key()).expect("address must derive")
}

/// The V10 gate: anychain must land on the address the raw-primitives spike
/// derives for the same mnemonic and path.
#[test]
fn slip44_195_matches_independent_implementation() {
    assert_eq!(
        address_at(TRON_PATH).to_base58(),
        CROSS_CHECKED_TRON_ADDRESS,
        "anychain derivation disagrees with the tron-v1 spike (bip39/bip32/k256)"
    );
}

/// Second anchor, one layer lower: the account hash itself, checked against a
/// vector five other crates in this repository already depend on.
#[test]
fn eth_anchor_reproduces_repo_canonical_vector() {
    let hex = address_at(ETHEREUM_PATH).to_hex().to_lowercase();

    // TronAddress renders 21 bytes: the 0x41 prefix plus the 20-byte account.
    assert_eq!(hex.len(), 42, "expected 21 bytes of hex, got {hex}");
    assert_eq!(
        &hex[2..],
        CANONICAL_ETH_ACCOUNT,
        "anychain derivation drifted from the repo-wide abandon-about vector"
    );
}

#[test]
fn slip44_195_address_uses_tron_prefix() {
    let address = address_at(TRON_PATH);

    let hex = address.to_hex().to_lowercase();
    assert!(
        hex.starts_with("41"),
        "TRON addresses carry the 0x41 type prefix, got {hex}"
    );

    let base58 = address.to_base58();
    assert!(
        base58.starts_with('T'),
        "0x41 base58check-encodes to a leading T, got {base58}"
    );
    assert_eq!(base58.len(), 34, "T-addresses are 34 characters");
}

#[test]
fn slip44_195_address_round_trips_through_base58() {
    let address = address_at(TRON_PATH);
    let parsed: Address = address.to_base58().parse().expect("round-trip must parse");

    assert_eq!(parsed, address);
    assert_eq!(parsed.to_hex(), address.to_hex());
}

/// Guards the failure mode where the coin index never reaches derivation and
/// every chain silently shares one account.
#[test]
fn coin_195_and_coin_60_derive_distinct_accounts() {
    assert_ne!(
        address_at(TRON_PATH).to_hex(),
        address_at(ETHEREUM_PATH).to_hex(),
        "coin index must reach the derivation"
    );
}

/// The BIP-39 passphrase must reach the seed. Without it, passphrase-protected
/// wallets would silently collapse onto the no-passphrase account.
#[test]
fn passphrase_changes_the_derived_account() {
    let path = TRON_PATH.parse().expect("valid path");
    let mnemonic = canonical_mnemonic();

    let plain = derive_keypair(&mnemonic, "", &path).expect("derivation must succeed");
    let salted = derive_keypair(&mnemonic, "TREZOR", &path).expect("derivation must succeed");

    assert_ne!(
        Address::from_public_key(plain.public_key())
            .expect("address")
            .to_base58(),
        Address::from_public_key(salted.public_key())
            .expect("address")
            .to_base58(),
    );
}
