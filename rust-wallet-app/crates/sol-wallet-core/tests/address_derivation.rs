//! `address_derivation` — Phase 1.1.
//!
//! Asserts the SLIP-0010 derivation chain produces the canonical
//! Phantom-equivalent addresses for the standard BIP-39 test vectors.
//!
//! Deep-dive coverage: row 1 (SLIP-0010 derivation path) + row 4
//! (`is_on_curve` check via `solana_sdk::Pubkey::is_on_curve`; the
//! `sol_wallet_core::address::is_on_curve` wrapper lands in Phase 2).

use sol_wallet_core::wallet::Wallet;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Canonical Phantom-compatible address for `m/44'/501'/0'/0'/0`.
///
/// Reference: Solana CLI `solana-keygen pubkey prompt://` with the
/// same mnemonic. This is the canonical BIP-39 SLIP-0010 test vector
/// for the Solana wallet stack.
const EXPECTED_PUBKEY_M44_501_0_0: &str = "H4G1YxbyeAMCjiQmfyHHFkNyzN8njHKXhKJxTV2YFTJ1";

#[test]
fn default_derivation_matches_phantom_canonical() {
    let w = Wallet::from_mnemonic(MNEMONIC).expect("mnemonic must parse");
    assert_eq!(
        w.public_key().to_string(),
        EXPECTED_PUBKEY_M44_501_0_0,
        "default derivation path m/44'/501'/0'/0' must match the canonical Phantom-compatible address"
    );
}

#[test]
fn from_mnemonic_at_1_0_derives_distinct_address() {
    let w0 = Wallet::from_mnemonic_at(MNEMONIC, 0, 0).expect("mnemonic must parse");
    let w1 = Wallet::from_mnemonic_at(MNEMONIC, 1, 0).expect("mnemonic must parse");
    assert_ne!(
        w0.public_key().to_string(),
        w1.public_key().to_string(),
        "account=1 must produce a different address than account=0"
    );
}

#[test]
fn address_index_advances_distinct_addresses_within_same_account() {
    let w0 = Wallet::from_mnemonic_at(MNEMONIC, 0, 0).expect("mnemonic must parse");
    let w1 = Wallet::from_mnemonic_at(MNEMONIC, 0, 1).expect("mnemonic must parse");
    assert_ne!(
        w0.public_key().to_string(),
        w1.public_key().to_string(),
        "address_index=1 must produce a different address than address_index=0 within the same account"
    );
}

#[test]
fn address_is_on_curve() {
    // Row 4: derived addresses must satisfy is_on_curve. Guards
    // against an accidental off-curve (PDA-shaped) derivation — PDAs
    // are NOT allowed as wallet pubkeys.
    let w = Wallet::from_mnemonic(MNEMONIC).expect("mnemonic must parse");
    assert!(
        w.public_key().is_on_curve(),
        "wallet-derived address must be on the Ed25519 curve"
    );
}
