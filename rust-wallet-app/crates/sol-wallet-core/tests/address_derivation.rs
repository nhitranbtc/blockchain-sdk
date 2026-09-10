//! `address_derivation` — Phase 1.1 + Phase 2.1.
//!
//! Phase 1.1 owns the SLIP-0010 derivation chain + Phantom canonical
//! address assertion (rows 1 + 4). Phase 2.1 owns the address surface:
//! base58 round-trip, `parse_user_address` (accept known valid devnet
//! address; reject invalid base58; reject off-curve bytes), and
//! `find_pda` (off-curve Pubkey + bump byte). See
//! `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`
//! §Phase 2 Task 2.1.

use sol_wallet_core::address::{
    find_pda, is_on_curve, parse_user_address, pubkey_from_bytes, pubkey_to_base58,
};
use sol_wallet_core::wallet::Wallet;
use solana_sdk::pubkey::Pubkey;

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

// ── Phase 2.1 — address surface (base58 + parse + is_on_curve + PDA) ──

/// Known valid Solana address on a development cluster — used to
/// anchor `parse_user_address` accepts-base58 round-trip. Pulled from
/// the plan's Phase 2 Step 6 acceptance criteria; we do not assume a
/// specific cluster, only that the bytes form a valid Pubkey on the
/// Ed25519 curve.
const KNOWN_VALID_ADDRESS: &str = "2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH";

#[test]
fn pubkey_from_bytes_then_to_base58_round_trips() {
    // Phase 2.1 Step 6: bytes → base58 → bytes round-trip.
    let original = Wallet::from_mnemonic(MNEMONIC)
        .expect("mnemonic must parse")
        .public_key();
    let bytes = original.to_bytes();
    assert_eq!(
        pubkey_from_bytes(bytes),
        original,
        "pubkey_from_bytes must round-trip 32 raw bytes into the same Pubkey"
    );
    let b58 = pubkey_to_base58(&original);
    let parsed = parse_user_address(&b58).expect("base58 round-trip must parse");
    assert_eq!(
        parsed, original,
        "base58 round-trip must yield the same Pubkey"
    );
}

#[test]
fn pubkey_to_base58_format_matches_phantom_canonical() {
    // Sanity: the wrapper produces the same base58 string as Anza's
    // `Pubkey::to_string` (which Phantom also emits). Guards against
    // an accidental alphabet or length-prefix drift.
    let wallet = Wallet::from_mnemonic(MNEMONIC).expect("mnemonic must parse");
    let pk = wallet.public_key();
    assert_eq!(
        pubkey_to_base58(&pk),
        pk.to_string(),
        "pubkey_to_base58 must match solana_sdk::Pubkey::to_string (Phantom canonical)"
    );
}

#[test]
fn parse_user_address_accepts_known_valid_devnet_address() {
    let pk =
        parse_user_address(KNOWN_VALID_ADDRESS).expect("known valid devnet address must parse");
    assert_eq!(
        pk.to_string(),
        KNOWN_VALID_ADDRESS,
        "parse_user_address must accept the canonical base58 form and return the same address"
    );
    assert!(
        pk.is_on_curve(),
        "known valid devnet address must lie on the Ed25519 curve"
    );
}

#[test]
fn parse_user_address_rejects_invalid_base58() {
    // Phase 2.1 Step 4: `parse_user_address` rejects invalid base58.
    let err = parse_user_address("not-base58!!!").expect_err("must reject malformed base58");
    let msg = err.to_string();
    assert!(
        msg.contains("base58") || msg.contains("address"),
        "error message must mention base58/address; got: {msg}"
    );
}

#[test]
fn parse_user_address_rejects_off_curve_bytes() {
    // PDA footgun: a 32-byte string that decodes as valid base58 but
    // the resulting y-coordinate does not satisfy the Ed25519 curve
    // equation. We derive a real PDA via `find_pda` (guaranteed
    // off-curve) instead of hand-picking bytes — all-zero 32-byte
    // buffers happen to land ON-curve (the identity point), so they
    // are not a useful negative fixture. Phantom-equivalent wallets
    // MUST refuse to use a non-Pubkey as a recipient — sending to
    // such an address would burn funds (no signer exists for a PDA).
    let program_id = Pubkey::new_unique();
    let (pda, _bump) = find_pda(&[b"off-curve-fixture"], &program_id);
    assert!(
        !pda.is_on_curve(),
        "precondition: PDA from find_pda must be off-curve"
    );
    let b58 = pda.to_string();
    assert!(
        parse_user_address(&b58).is_err(),
        "parse_user_address must reject off-curve addresses"
    );
}

#[test]
fn is_on_curve_wrapper_matches_sdk_for_derived_wallet() {
    // Phase 2.1 Step 3 + Step 6: wrapper agrees with `solana_sdk`.
    let wallet = Wallet::from_mnemonic(MNEMONIC).expect("mnemonic must parse");
    let bytes = wallet.public_key().to_bytes();
    assert_eq!(
        is_on_curve(&bytes),
        wallet.public_key().is_on_curve(),
        "is_on_curve wrapper must agree with solana_sdk::Pubkey::is_on_curve"
    );
    assert!(
        is_on_curve(&bytes),
        "wallet-derived address must satisfy the Ed25519 curve check"
    );
}

#[test]
fn find_pda_returns_off_curve_pubkey_and_bump_byte() {
    // Phase 2.1 Step 5: `find_pda` returns `(Pubkey, u8)` — the PDA
    // address (necessarily OFF-curve) AND the canonical bump byte
    // that makes the seeds+program_id produce it. Used internally for
    // V0.1.5 staking; V0.1 only needs the surface.
    let program_id = Pubkey::new_unique();
    let seeds: &[&[u8]] = &[b"vault", b"treasury"];
    let (pda, bump) = find_pda(seeds, &program_id);
    assert!(
        !pda.is_on_curve(),
        "PDA must be off-curve (Solana forbids PDA-shaped wallets; PDAs are programs only)"
    );
    // Determinism: same inputs → same PDA + bump.
    let (pda2, bump2) = find_pda(seeds, &program_id);
    assert_eq!(
        (pda, bump),
        (pda2, bump2),
        "find_pda must be deterministic for fixed (seeds, program_id)"
    );
}
