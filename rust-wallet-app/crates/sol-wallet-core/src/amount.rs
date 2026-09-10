//! `Amount` — Solana-native lamport safety wrapper.
//!
//! V0.1 exposes the same primitive every Phantom-equivalent wallet uses:
//! a `u64` lamport count. The wire format (Solana protocol) has no
//! sub-lamport precision — `1 SOL = 1_000_000_000 lamports` is a
//! hard integer boundary. `Amount` is a thin newtype that:
//!
//! | Method                       | Role                                                   |
//! |------------------------------|--------------------------------------------------------|
//! | `Amount::from_lamports(u64)` | Wire-level constructor (no validation needed)          |
//! | `Amount::from_sol(f64)`      | User-input parser (rejects NaN/Inf/negative/overflow)   |
//! | `.lamports() -> u64`         | Wire-level accessor (returns the raw `u64`)            |
//!
//! `from_sol` is the only boundary that needs validation. The
//! `f64`-to-`u64` conversion is the classic overflow vector — passing
//! `f64::INFINITY` or `1e30` to a naive `as u64` cast yields
//! `u64::MAX` silently, which then gets handed to `system_instruction::transfer`
//! and the validator rejects the tx with `InvalidLamports`. Better
//! to reject at the parser boundary so the wallet UI surfaces a
//! single error instead of an opaque RPC rejection.
//!
//! `Amount` deliberately has no `Display`, `Add`, or `Sub` impls in
//! V0.1: every consumer (Phase 4 SPL, Phase 5 RPC, Phase 7 CLI) only
//! needs the raw `u64` for the wire format. Adding `Display + Sum`
//! without an immediate consumer is scope creep per L13 §karpathy §2.

use crate::error::{Error, Result};

/// Lamports per SOL. Solana's hard integer boundary.
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// Solana-native lamport amount.
///
/// Holds `u64` lamports. Construct via [`Amount::from_lamports`] for
/// wire-level inputs or [`Amount::from_sol`] for user-facing decimal
/// inputs (the latter validates; the former is infallible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Amount(u64);

impl Amount {
    /// Zero lamports — the empty / default amount. Mirrors the
    /// `Phantom::ZERO` UX for the "Send" form's reset button.
    pub const ZERO: Self = Self(0);

    /// Construct from a raw `u64` lamport count.
    ///
    /// Infallible: every `u64` is a valid lamport count at the wire
    /// layer (Solana validators reject 0-amount transfers with
    /// `InvalidLamports` but the builder itself accepts 0 — the
    /// Phase 5 preflight-balance gate surfaces that error upstream).
    pub const fn from_lamports(lamports: u64) -> Self {
        Self(lamports)
    }

    /// Construct from a `f64` SOL count.
    ///
    /// Rejects:
    ///   - `NaN`, `+Inf`, `-Inf` — `f64::is_finite()` gates
    ///   - Negative values — no negative lamports exist on the wire
    ///   - Overflow past `u64::MAX` lamports — SOL supply ceiling
    ///     (~6e8 SOL) is well below the overflow boundary; anything
    ///     larger is a caller bug
    ///   - Sub-lamport precision — fractional lamports below the
    ///     integer boundary are dropped (wire format has no
    ///     sub-lamport precision)
    pub fn from_sol(sol: f64) -> Result<Self> {
        if !sol.is_finite() {
            return Err(Error::InvalidAmount(format!(
                "SOL value must be finite (got {sol})"
            )));
        }
        if sol < 0.0 {
            return Err(Error::InvalidAmount(format!(
                "SOL value must be non-negative (got {sol})"
            )));
        }
        let lamports_f = sol * (LAMPORTS_PER_SOL as f64);
        if lamports_f > u64::MAX as f64 {
            return Err(Error::InvalidAmount(format!(
                "SOL value exceeds u64::MAX lamports (got {sol} SOL)"
            )));
        }
        let lamports = lamports_f as u64;
        debug_assert!(
            (lamports as f64).is_finite(),
            "post-cast finite check: {lamports_f}"
        );
        Ok(Self(lamports))
    }

    /// Raw lamport count. Wire-level accessor — every `tx::builder`
    /// call consumes this `u64` directly.
    pub const fn lamports(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_zero() {
        assert_eq!(Amount::ZERO.lamports(), 0);
    }

    #[test]
    fn from_lamports_round_trips() {
        assert_eq!(Amount::from_lamports(0).lamports(), 0);
        assert_eq!(Amount::from_lamports(1).lamports(), 1);
        assert_eq!(Amount::from_lamports(u64::MAX).lamports(), u64::MAX);
    }

    #[test]
    fn from_sol_one_is_one_billion() {
        assert_eq!(Amount::from_sol(1.0).unwrap().lamports(), 1_000_000_000);
    }

    #[test]
    fn from_sol_truncates_sub_lamport_precision() {
        assert_eq!(Amount::from_sol(0.001).unwrap().lamports(), 1_000_000);
    }

    #[test]
    fn from_sol_rejects_nan_inf_negative() {
        assert!(Amount::from_sol(f64::NAN).is_err());
        assert!(Amount::from_sol(f64::INFINITY).is_err());
        assert!(Amount::from_sol(f64::NEG_INFINITY).is_err());
        assert!(Amount::from_sol(-1.0).is_err());
    }

    #[test]
    fn from_sol_rejects_overflow() {
        assert!(Amount::from_sol(u64::MAX as f64).is_err());
    }
}
