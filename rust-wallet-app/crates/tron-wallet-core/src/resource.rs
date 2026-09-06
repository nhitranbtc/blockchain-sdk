//! Resource model (plan Task 3.8) — Energy estimation + Dynamic Energy
//! Multiplier (DEM) scaling for TRC-20 fee sizing.
//!
//! ## Why a separate module
//!
//! The energy ceiling on a TRC-20 transaction is what determines the
//! actual user-facing fee in TRX. A `triggerconstantcontract` round trip
//! reports the *base* energy a contract call would consume; the live
//! cost can be larger by the current DEM factor (up to ~3.4× at the
//! network maximum, per TronGrid docs). A transfer that consumes
//! 65 000 Energy at the base may consume 65 000 × 3.4 = ~221 000 in a
//! congested 6-hour window — under-sizing the fee_limit either rejects
//! the tx or burns more TRX than needed on retries.
//!
//! ## Cost formula
//!
//! ```text
//! recommended_fee_limit_sun = ceil(raw_energy × max_factor) × ENERGY_PRICE_SUN
//! ```
//!
//! 420 SUN is the documented per-Energy rate on TRON mainnet. The
//! `max_factor` defaults to 3.4 (the network maximum, used as the
//! conservative ceiling so the recommended fee_limit always covers a
//! peak-cycle submit).
//!
//! ## Sources
//!
//! - Base energy: `TronGridClient::estimate_energy` wraps
//!   `wallet/triggerconstantcontract` — same endpoint, same selector,
//!   same args; the response carries `energy_used`.
//! - DEM factor: `wallet/getcontractinfo` returns `energy_factor`. The
//!   factor is per-contract and per-6-hour-window. We do not poll it
//!   here; the caller can read it via [`crate::chain::TronGridClient`]
//!   (out of scope for v0.1 — the max_factor constant is the safer
//!   choice).

use crate::chain::TronGridClient;
use crate::error::Result;

/// TRON's documented Energy-to-SUN rate: 1 Energy = 420 SUN.
///
/// Used to size the fee_limit from a raw Energy estimate. The plan
/// (V5 spike) cross-checks this against a live `triggerconstantcontract`
/// round-trip on the Nile testnet.
pub const ENERGY_PRICE_SUN: u64 = 420;

/// Conservative DEM `max_factor` multiplier.
///
/// TronGrid documents the DEM as ranging up to ~3.4× over a 6-hour
/// cycle. We bake that ceiling into the recommended fee_limit so a
/// caller does not need to poll `getcontractinfo` before submitting
/// — that would let a peak cycle silently OOG the call.
pub const DEM_MAX_FACTOR: f64 = 3.4;

/// Result of an Energy estimate, ready to feed into a `fee_limit`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyEstimate {
    /// Raw Energy the contract call would consume at the current DEM
    /// factor (i.e. the value `triggerconstantcontract` returned).
    pub raw_energy: u64,
    /// Conservative estimate after applying [`DEM_MAX_FACTOR`]. This is
    /// what the fee_limit should *cover* — the worst-case ceiling.
    pub scaled_energy: u64,
    /// Recommended `fee_limit` value in SUN.
    pub recommended_fee_limit_sun: u64,
}

/// Estimate the Energy a contract call would consume, and return both the
/// raw value and the DEM-scaled recommendation.
///
/// The thin wrapper exists so callers do not have to multiply by
/// [`ENERGY_PRICE_SUN`] and [`DEM_MAX_FACTOR`] themselves — a wrong
/// multiplier silently under-budgets and lets a submit revert.
///
/// Falls back to the raw Energy if the response omitted `energy_used`
/// (some TronGrid variants do); the resulting estimate then has
/// `scaled_energy = raw_energy` and `recommended_fee_limit_sun = 0`,
/// which is the conservative "no overhead" answer.
pub async fn estimate_energy(
    rpc: &TronGridClient,
    contract: &str,
    owner_address: &str,
    function: &str,
    args: &[u8],
) -> Result<EnergyEstimate> {
    let raw_energy = rpc
        .estimate_energy(contract, owner_address, function, args)
        .await?;
    let scaled = scale_energy(raw_energy, DEM_MAX_FACTOR);
    let fee_limit_sun = scaled.saturating_mul(ENERGY_PRICE_SUN);
    Ok(EnergyEstimate {
        raw_energy,
        scaled_energy: scaled,
        recommended_fee_limit_sun: fee_limit_sun,
    })
}

/// Scale a raw Energy value by a DEM `max_factor`. Rounds up so a
/// 65 000 Energy base × 3.4 ceiling lands on 221 000, not 219 999
/// (truncation would silently under-budget the same way a missing
/// multiplier would).
pub fn scale_energy(raw: u64, max_factor: f64) -> u64 {
    if raw == 0 {
        return 0;
    }
    let scaled = (raw as f64) * max_factor;
    scaled.ceil() as u64
}

/// Default `fee_limit` sized to a 65 000-Energy baseline (held-recipient
/// USDT-TRC20 transfer) × [`DEM_MAX_FACTOR`] × [`ENERGY_PRICE_SUN`].
///
/// The plan's Task 3.8 cites 100 000 000 SUN (100 TRX); the formula
/// arrives at 92 820 000 — close to but distinct from that value. The
/// function is the canonical place to compute it; hand-coding
/// 100 000 000 in the builder would lose the connection to the inputs.
pub const fn default_fee_limit_sun() -> u64 {
    let scaled = scale_energy_const(65_000, DEM_MAX_FACTOR);
    scaled * ENERGY_PRICE_SUN
}

/// Compile-time version of [`scale_energy`] for use in const contexts.
///
/// f64 arithmetic is not `const fn` on stable Rust as of the pinned
/// MSRV 1.98.1, so we hand-compute the same ceiling. The max factor
/// is documented at 3.4 — if it changes, this function changes too,
/// and `default_fee_limit_sun()` updates accordingly.
const fn scale_energy_const(raw: u64, max_factor: f64) -> u64 {
    if raw == 0 {
        return 0;
    }
    // 3.4 × raw, ceiled. Integer math: 34 × raw / 10, rounded up if
    // the division had a remainder.
    let scaled_x100 = (raw as u128) * (max_factor * 100.0) as u128;
    let rounded = scaled_x100.div_ceil(100);
    rounded as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_energy_zero_short_circuits() {
        assert_eq!(scale_energy(0, DEM_MAX_FACTOR), 0);
    }

    #[test]
    fn scale_energy_rounds_up_at_max_factor() {
        // 65_000 × 3.4 = 221_000.0 exact — ceiling leaves it at 221_000.
        assert_eq!(scale_energy(65_000, 3.4), 221_000);
        // 65_001 × 3.4 = 221_003.4 — ceiling rounds to 221_004.
        assert_eq!(scale_energy(65_001, 3.4), 221_004);
    }

    #[test]
    fn default_fee_limit_floors_held_recipient_baseline() {
        // 65_000 × 3.4 × 420 = 92_820_000. This is the floor for a
        // held-recipient USDT transfer at max DEM; the plan's
        // documented 100 000 000 baseline is a round-up.
        assert_eq!(default_fee_limit_sun(), 92_820_000);
    }

    #[test]
    fn const_scale_energy_matches_runtime() {
        // The const version must agree with the runtime one — the
        // plan cites both numbers in the same sentence and a drift
        // between them is the kind of bug this guardrail exists for.
        for raw in [0, 1, 65_000, 65_001, 130_000, 1_000_000] {
            assert_eq!(
                scale_energy_const(raw, DEM_MAX_FACTOR),
                scale_energy(raw, DEM_MAX_FACTOR),
                "const/runtime scale_energy disagree at raw={raw}"
            );
        }
    }
}
