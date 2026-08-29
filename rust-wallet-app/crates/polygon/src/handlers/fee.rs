//! Fee-tier resolution for `wallet send` + `wallet send speed-up`.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.7: maps the four CLI-facing tier names
//! (`fastest | half_hour | hour | economy`) to EIP-1559
//! `(max_fee_per_gas, max_priority_fee_per_gas)` multipliers over the
//! per-call `provider.estimate_eip1559_fees()` baseline.
//!
//! Pure: no RPC, no I/O. Lives in its own module so the per-tier table is
//! testable in isolation and so future fee-tier additions (e.g. a
//! `custom` tier) land in one file rather than spreading across the
//! send + speedup handler bodies.
//!
//! T6c5 (Issue #426 sub-task): the `resolve_fee_tier` fn is consumed by
//! `wallet_send_native` + `wallet_send_speedup`. The handler layer is
//! responsible for fetching the baseline estimate; this module only
//! applies the multiplier table.

use polygon_wallet_core::{Error, Result};

/// Four CLI-facing fee tiers per design §3.4 (Story 5 + 6 AC) + user
/// stories doc. Ordered fastest → economy. Multipliers below.
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeTier {
    Fastest,
    HalfHour,
    Hour,
    Economy,
}

/// Multiplier applied to the baseline `max_fee_per_gas` estimate per tier.
/// Priority fee gets the SAME multiplier (conservative — bumps together
/// so RBF replacement always raises the tip in lockstep). Values are
/// `f64` for arithmetic then rounded to `u128` wei at apply time.
///
/// Source: user-stories doc Story 5 AC + design §5.7. Calibrated against
/// Polygon's 2-second block time (per plan §Q5: cached values go stale
/// in <3s, so the per-call estimate is the only honest input).
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
fn tier_multiplier(tier: FeeTier) -> f64 {
    match tier {
        FeeTier::Fastest => 1.20,
        FeeTier::HalfHour => 1.00,
        FeeTier::Hour => 0.90,
        FeeTier::Economy => 0.80,
    }
}

/// Parse the CLI `--fee` string into a `FeeTier`. Unknown values return
/// `Error::InvalidInput` with the canonical list in the message so the
/// operator sees the fix. Case-insensitive (`Half_Hour`, `HALF_HOUR`,
/// `half_hour` all accepted).
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
pub fn parse_fee_tier(s: &str) -> Result<FeeTier> {
    match s.to_ascii_lowercase().as_str() {
        "fastest" => Ok(FeeTier::Fastest),
        "half_hour" | "half-hour" | "halfhour" => Ok(FeeTier::HalfHour),
        "hour" => Ok(FeeTier::Hour),
        "economy" => Ok(FeeTier::Economy),
        other => Err(Error::InvalidInput(format!(
            "unknown --fee tier '{other}'; expected one of: fastest | half_hour | hour | economy"
        ))),
    }
}

/// Apply the tier multiplier to a `(max_fee, priority_fee)` baseline.
/// Returns the new `(max_fee_per_gas, max_priority_fee_per_gas)` tuple
/// in wei. `baseline` comes from `provider.estimate_eip1559_fees()`.
///
/// Always returns the tuple; the caller's responsibility to fetch the
/// baseline + handle provider errors (this fn is pure).
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
pub fn resolve_fee_tier(
    tier: FeeTier,
    baseline_max_fee: u128,
    baseline_priority_fee: u128,
) -> (u128, u128) {
    let m = tier_multiplier(tier);
    let new_max = ((baseline_max_fee as f64) * m).round() as u128;
    let new_prio = ((baseline_priority_fee as f64) * m).round() as u128;
    (new_max, new_prio)
}

#[cfg(test)]
mod tests {
    //! Batch F tests (per design doc §6.6): fee-tier parsing + multiplier
    //! application are pure, so they test here without any RPC.

    use super::{parse_fee_tier, resolve_fee_tier, Error, FeeTier};

    /// S6a (failing seed per design §6.6): parse_fee_tier accepts the
    /// four canonical names case-insensitively.
    #[test]
    fn parse_fee_tier_accepts_all_four_canonical_names() {
        assert_eq!(parse_fee_tier("fastest").unwrap(), FeeTier::Fastest);
        assert_eq!(parse_fee_tier("half_hour").unwrap(), FeeTier::HalfHour);
        assert_eq!(parse_fee_tier("hour").unwrap(), FeeTier::Hour);
        assert_eq!(parse_fee_tier("economy").unwrap(), FeeTier::Economy);
    }

    /// S6b: case-insensitive — `FASTEST` and `Economy` (mixed case)
    /// both parse. Common TTY-input artifact.
    #[test]
    fn parse_fee_tier_is_case_insensitive() {
        assert_eq!(parse_fee_tier("FASTEST").unwrap(), FeeTier::Fastest);
        assert_eq!(parse_fee_tier("Economy").unwrap(), FeeTier::Economy);
    }

    /// S6c: hyphen + no-separator variants accepted for `half_hour`
    /// (the only multi-word tier name). Operator UX shortcut.
    #[test]
    fn parse_fee_tier_accepts_hyphen_and_concatenated_half_hour() {
        assert_eq!(parse_fee_tier("half-hour").unwrap(), FeeTier::HalfHour);
        assert_eq!(parse_fee_tier("halfhour").unwrap(), FeeTier::HalfHour);
    }

    /// S6d (failing seed): unknown tier name returns Error::InvalidInput
    /// with the canonical list in the message (operator sees the fix).
    #[test]
    fn parse_fee_tier_rejects_unknown_with_canonical_list() {
        let r = parse_fee_tier("turbo");
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("fastest") && msg.contains("economy"),
                    "InvalidInput must list all four canonical tiers; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// S6e (failing seed): resolve_fee_tier applies multipliers in the
    /// canonical order Fastest > HalfHour > Hour > Economy. Baseline
    /// 50 gwei max + 30 gwei priority (realistic Polygon values).
    #[test]
    fn resolve_fee_tier_multipliers_apply_in_canonical_order() {
        let baseline = (50_000_000_000u128, 30_000_000_000u128);
        let fastest = resolve_fee_tier(FeeTier::Fastest, baseline.0, baseline.1);
        let half = resolve_fee_tier(FeeTier::HalfHour, baseline.0, baseline.1);
        let hour = resolve_fee_tier(FeeTier::Hour, baseline.0, baseline.1);
        let eco = resolve_fee_tier(FeeTier::Economy, baseline.0, baseline.1);

        assert!(
            fastest.0 > half.0,
            "fastest max_fee ({}) must exceed half_hour ({})",
            fastest.0,
            half.0
        );
        assert!(
            half.0 > hour.0,
            "half_hour max_fee ({}) must exceed hour ({})",
            half.0,
            hour.0
        );
        assert!(
            hour.0 > eco.0,
            "hour max_fee ({}) must exceed economy ({})",
            hour.0,
            eco.0
        );
        // Priority fee moves in lockstep (conservative RBF).
        assert!(
            fastest.1 > half.1 && half.1 > hour.1 && hour.1 > eco.1,
            "priority_fee must descend fastest > half > hour > economy"
        );
    }

    /// S6f: HalfHour is identity (multiplier 1.0). Baseline passes
    /// through unchanged — sanity check that the default tier is a no-op.
    #[test]
    fn resolve_fee_tier_half_hour_is_identity() {
        let baseline = (50_000_000_000u128, 30_000_000_000u128);
        let (max_fee, prio) = resolve_fee_tier(FeeTier::HalfHour, baseline.0, baseline.1);
        assert_eq!(
            max_fee, baseline.0,
            "half_hour max_fee must equal baseline (1.0x multiplier)"
        );
        assert_eq!(
            prio, baseline.1,
            "half_hour priority_fee must equal baseline (1.0x multiplier)"
        );
    }

    /// S6g: zero baseline → zero result for any tier (no underflow,
    /// no negative result from f64::round as u128).
    #[test]
    fn resolve_fee_tier_zero_baseline_yields_zero() {
        assert_eq!(
            resolve_fee_tier(FeeTier::Fastest, 0, 0),
            (0u128, 0u128),
            "zero baseline must yield zero (no f64 rounding to negative)"
        );
        assert_eq!(resolve_fee_tier(FeeTier::Economy, 0, 0), (0u128, 0u128));
    }
}
