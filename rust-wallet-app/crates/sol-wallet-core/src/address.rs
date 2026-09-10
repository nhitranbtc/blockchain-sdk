//! Solana address surface — base58 round-trip + `is_on_curve` guard
//! + PDA derivation.
//!
//! Phase 2 lands these five wrappers around Anza's `solana_sdk::Pubkey`:
//!
//! | Wrapper                 | Underlying call                          |
//! |-------------------------|------------------------------------------|
//! | `pubkey_from_bytes`     | `Pubkey::new_from_array`                 |
//! | `pubkey_to_base58`      | `Pubkey::to_string` (Phantom canonical)  |
//! | `is_on_curve`           | `Pubkey::is_on_curve`                    |
//! | `parse_user_address`    | `Pubkey::from_str` + `is_on_curve` guard |
//! | `find_pda`              | `Pubkey::find_program_address`           |
//!
//! V0.1 spec: `parse_user_address` rejects PDA-shaped (off-curve) bytes
//! at the parse boundary. Phantom-equivalent wallets refuse to send to
//! off-curve addresses because no private key exists for a PDA — sending
//! would burn funds. Mirrors Phantom's UI guard at the parser layer so
//! the wallet app surfaces a single `Error::InvalidAddress` to the user
//! instead of leaking `solana_sdk::PubkeyError` variants.
//!
//! Deep-dive reference: `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md`
//! §F lines 1679–1729 (Phantom-equivalent API); plan §Phase 2 Task 2.1.

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

use crate::error::{Error, Result};

/// Wrap 32 raw bytes as a Solana `Pubkey`.
///
/// Bytes are interpreted as the Ed25519 verification key in Solana's
/// canonical layout. Caller is responsible for ensuring `bytes`
/// represents a valid point on the Ed25519 curve — use [`is_on_curve`]
/// first when the source is untrusted.
pub fn pubkey_from_bytes(bytes: [u8; 32]) -> Pubkey {
    Pubkey::new_from_array(bytes)
}

/// Base58-encode a `Pubkey` (Phantom canonical form — no prefix).
///
/// 32-byte Ed25519 verification keys produce 32-44 char base58 strings
/// with no `0x` / `solana:` prefix. The output matches
/// `solana_sdk::Pubkey::to_string` and the strings Phantom displays in
/// its "Receive" panel.
pub fn pubkey_to_base58(pk: &Pubkey) -> String {
    pk.to_string()
}

/// Is the 32-byte buffer a point on the Ed25519 curve?
///
/// Used to reject PDA-shaped addresses at the parse boundary. Solana
/// enforces this distinction at the wire level: only on-curve addresses
/// may hold SOL accounts, and only PDAs (off-curve) may be program
/// derived addresses. Wallets that treat an off-curve address as a
/// recipient will burn the funds (no signer exists).
pub fn is_on_curve(bytes: &[u8]) -> bool {
    // `Pubkey::is_on_curve` exists on `&Pubkey` in Anza 4.x; for raw
    // bytes we construct a temporary Pubkey. The constructor does NOT
    // validate the curve — it stores the bytes verbatim — so
    // `is_on_curve` on the constructed value is the curve check.
    debug_assert_eq!(bytes.len(), 32, "Solana pubkeys are 32 bytes");
    let pk = Pubkey::new_from_array(bytes_to_array_32(bytes));
    pk.is_on_curve()
}

/// Parse a user-supplied Solana address string.
///
/// Accepts base58 (Phantom canonical) only — no URI scheme, no
/// `solana:` prefix, no leading whitespace. Rejects:
///
/// 1. malformed base58 (non-alphabet characters, wrong length)
/// 2. off-curve bytes (the PDA footgun — see [`is_on_curve`])
///
/// Returns `Error::InvalidAddress` with a human-readable reason on
/// failure so the wallet UI can surface a single error message instead
/// of pattern-matching `solana_sdk::PubkeyError` variants.
pub fn parse_user_address(s: &str) -> Result<Pubkey> {
    let pk = Pubkey::from_str(s).map_err(|e| Error::InvalidAddress(format!("base58: {e}")))?;
    if !pk.is_on_curve() {
        return Err(Error::InvalidAddress(
            "address is off-curve (PDA-shaped); wallets cannot send to PDAs".to_string(),
        ));
    }
    Ok(pk)
}

/// Program-derived address (PDA) for `seeds` under `program_id`.
///
/// Returns `(pda, bump)` where `pda` is necessarily OFF-curve (Solana
/// enforces this — a PDA that lands on the curve is an exploit vector)
/// and `bump` is the canonical 8-bit seed nonce that produces it.
///
/// Used in V0.1.5 staking flows; V0.1 only needs the surface so the
/// function lives in the public API. Backed by Anza's
/// `Pubkey::find_program_address` — Solana's canonical PDA finder.
pub fn find_pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(seeds, program_id)
}

/// Copy a 32-byte slice into a fixed-size array, panicking on length
/// mismatch. `Pubkey::new_from_array` requires `&[u8; 32]`; `is_on_curve`
/// receives `&[u8]` from the caller (we accept the slice for ergonomic
/// reasons — `&[u8; 32]` would force callers to coerce `Vec<u8>`-shaped
/// buffers).
fn bytes_to_array_32(slice: &[u8]) -> [u8; 32] {
    slice
        .try_into()
        .expect("caller must provide a 32-byte slice")
}

#[cfg(test)]
mod tests {
    //! Unit tests for the address wrappers — integration coverage
    //! (round-trip through Wallet derivation, Phantom canonical
    //! vector, PDA determinism) lives in `tests/address_derivation.rs`.

    use super::*;

    #[test]
    fn pubkey_from_bytes_round_trips_through_to_base58() {
        let bytes = [7u8; 32];
        let pk = pubkey_from_bytes(bytes);
        assert_eq!(pk.to_bytes(), bytes);
        assert_eq!(pubkey_to_base58(&pk), pk.to_string());
    }

    #[test]
    fn is_on_curve_true_for_arbitrary_random_bytes_is_consistent_with_sdk() {
        // Random 32 bytes: roughly 50% chance of being on-curve; the
        // assertion here is that our wrapper agrees with the SDK.
        let bytes = [42u8; 32];
        let pk = Pubkey::new_from_array(bytes);
        assert_eq!(is_on_curve(&bytes), pk.is_on_curve());
    }

    #[test]
    fn parse_user_address_distinguishes_base58_vs_off_curve() {
        // Malformed base58 → InvalidAddress (base58 reason).
        assert!(matches!(
            parse_user_address("not-base58!!!"),
            Err(Error::InvalidAddress(_))
        ));
        // A canonical PDA from `find_pda` is guaranteed OFF-curve.
        // All-zero 32 bytes happen to land on the curve in Ed25519
        // (the identity point), so we can't use them as a fixture;
        // derive a real PDA instead.
        let program_id = Pubkey::new_unique();
        let (pda, _bump) = find_pda(&[b"off-curve-fixture"], &program_id);
        assert!(
            !pda.is_on_curve(),
            "precondition: PDA from find_pda must be off-curve"
        );
        assert!(matches!(
            parse_user_address(&pda.to_string()),
            Err(Error::InvalidAddress(_))
        ));
    }

    #[test]
    fn find_pda_keyword_returned_tuple_is_off_curve() {
        let program_id = Pubkey::new_unique();
        let (pda, _bump) = find_pda(&[b"seed"], &program_id);
        assert!(!pda.is_on_curve(), "PDA must be off-curve");
    }
}
