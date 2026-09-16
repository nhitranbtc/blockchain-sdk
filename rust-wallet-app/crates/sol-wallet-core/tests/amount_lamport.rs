//! `amount_lamport` — Phase 3.1 (deep-dive rows 3+4).
//!
//! Covers the lamport safety wrapper that every `tx::builder` consumer
//! passes through. Phantom-equivalent wallets accept `u64` lamports
//! directly — Solana's smallest unit, 1 SOL = 1_000_000_000 lamports.
//!
//! `Amount` is the newtype that:
//!   - holds lamports as `u64` (no float ambiguity at the wire layer),
//!   - provides `from_sol(f64) -> Result<Amount, _>` to convert from the
//!     user-facing decimal (with explicit rejection of `f64` NaN/Inf +
//!     overflow past `u64::MAX`),
//!   - round-trips via proptest to prove `as_lamport() == from_lamports(x).as_lamport()`.
//!
//! Phantom wallet's "Send" screen accepts a `0.001 SOL` decimal; rounding
//! to lamports in `f64` is the classic overflow vector (u64::MAX SOL =
//! ~1.8e10 SOL far exceeds SOL supply ~6e8). The newtype prevents the
//! caller from ever constructing an `Instruction` with a malformed
//! lamport count.

use proptest::prelude::*;
use sol_wallet_core::amount::Amount;

#[test]
fn amount_zero_has_zero_lamports() {
    assert_eq!(Amount::ZERO.lamports(), 0);
}

#[test]
fn amount_from_lamports_round_trips() {
    let cases: &[u64] = &[0, 1, 1_000_000_000, u64::MAX];
    for &l in cases {
        let a = Amount::from_lamports(l);
        assert_eq!(a.lamports(), l, "round-trip from_lamports({l})");
    }
}

#[test]
fn amount_from_sol_one_equals_one_billion_lamports() {
    let a = Amount::from_sol(1.0).expect("1.0 SOL must parse");
    assert_eq!(a.lamports(), 1_000_000_000);
}

#[test]
fn amount_from_sol_handles_fractional_lamports_via_truncation() {
    // 0.001 SOL = 1_000_000 lamports; fractional lamports below the
    // integer lamport are dropped (the SOL→lamport wire format has no
    // sub-lamport precision).
    let a = Amount::from_sol(0.001).expect("0.001 SOL must parse");
    assert_eq!(a.lamports(), 1_000_000);
}

#[test]
fn amount_from_sol_rejects_nan_and_infinity() {
    assert!(Amount::from_sol(f64::NAN).is_err(), "NaN must reject");
    assert!(
        Amount::from_sol(f64::INFINITY).is_err(),
        "+Infinity must reject"
    );
    assert!(
        Amount::from_sol(f64::NEG_INFINITY).is_err(),
        "-Infinity must reject"
    );
}

#[test]
fn amount_from_sol_rejects_negative_values() {
    assert!(
        Amount::from_sol(-1.0).is_err(),
        "negative SOL must reject (no negative lamports exist on the wire)"
    );
}

#[test]
fn amount_from_sol_rejects_overflow_at_u64_max() {
    // u64::MAX SOL = 18_446_744_073_709_551_615 SOL = ~1.8e10 SOL;
    // SOL total supply ≈ 6e8 so this is impossible in practice — but
    // a buggy caller passing `f64` directly to lamport math would
    // silently overflow. Reject at the boundary.
    assert!(
        Amount::from_sol(u64::MAX as f64).is_err(),
        "SOL count exceeding u64::MAX lamports must reject"
    );
}

#[test]
fn amount_from_sol_at_supply_limit_succeeds() {
    // SOL supply ceiling ~6.0e8 SOL — well below u64::MAX. Must parse
    // without overflow.
    let a = Amount::from_sol(600_000_000.0).expect("SOL supply ceiling must parse");
    assert_eq!(a.lamports(), 600_000_000 * 1_000_000_000);
}

proptest! {
    /// Proptest round-trip: any `u64` lamport count survives a
    /// `from_lamports → as_lamport` round-trip unchanged. Proves the
    /// newtype is a true identity wrapper over `u64` (no truncation,
    /// no off-by-one in the lamport representation).
    #[test]
    fn amount_proptest_round_trip(lamports in 0u64..=u64::MAX) {
        let a = Amount::from_lamports(lamports);
        prop_assert_eq!(a.lamports(), lamports);
    }
}
