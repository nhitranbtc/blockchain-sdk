//! Fee-tier resolution + `polygon fee` handler — Issue #426 / T6d-1.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.7: maps the four CLI-facing tier names
//! (`fastest | half_hour | hour | economy`) to EIP-1559
//! `(max_fee_per_gas, max_priority_fee_per_gas)` multipliers over the
//! per-call `provider.estimate_eip1559_fees()` baseline.
//!
//! `fetch_fee_estimate` (T6d-1 / Story 8 — `polygon fee`) calls
//! `provider.estimate_eip1559_fees()` once per invocation — no cache
//! because Polygon's 2-second block time (plan §Q5) makes cached
//! values stale in <3s.
//!
//! Tier helpers (parse_fee_tier, resolve_fee_tier) are pure RPC-free;
//! `fetch_fee_estimate` + `build_provider` are the only RPC-touching
//! fns. Lives in its own module so the per-tier table stays
//! testable in isolation.

use alloy_network::Ethereum;
use alloy_provider::{Provider, RootProvider};
use serde::Serialize;

use polygon_wallet_core::{
    new_http, new_http_polygon_amoy, new_http_polygon_mainnet, Error, Network, PolygonChain, Result,
};

use crate::handlers::validate_rpc_scheme;

/// EIP-1559 fee estimate plus the chain_id of the responding RPC.
///
/// `wei` fields carry the raw u128 from `provider.estimate_eip1559_fees()`;
/// `gwei` fields are the same value as `f64 / 1e9` for human-readable
/// display (Polygon mainnet max_fee_per_gas is typically 30-300 gwei).
/// Both representations travel together in `--json` output so consumers
/// don't have to redo the conversion.
#[derive(Debug, Clone, Serialize)]
pub struct FeeEstimate {
    pub network: String,
    pub chain_id: u64,
    pub max_fee_per_gas_wei: u128,
    pub max_priority_fee_per_gas_wei: u128,
    pub max_fee_per_gas_gwei: f64,
    pub max_priority_fee_per_gas_gwei: f64,
}

/// Build an `RootProvider<Ethereum>` from the per-network default RPC
/// (Amoy / mainnet) or an operator-supplied `--rpc-url`. Reuses the
/// same scheme guard as the wallet handlers (moved to
/// `super::validate_rpc_scheme` so future handlers don't duplicate
/// the policy). Returns `Error::InvalidInput` for non-https / non-
/// loopback-http URLs so the operator sees the fix at exit 2.
pub(crate) fn build_provider(
    rpc_url: Option<&str>,
    network: Network,
) -> Result<RootProvider<Ethereum>> {
    if let Some(url_str) = rpc_url {
        let url = url::Url::parse(url_str)
            .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
        validate_rpc_scheme(&url)?;
        return new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")));
    }
    match network {
        Network::Polygon(PolygonChain::Amoy) => {
            new_http_polygon_amoy().map_err(|e| Error::Rpc(format!("provider amoy: {e}")))
        }
        Network::Polygon(PolygonChain::Mainnet) => {
            new_http_polygon_mainnet().map_err(|e| Error::Rpc(format!("provider mainnet: {e}")))
        }
        // Polygon CLI never imports an Ethereum-network wallet; an
        // Ethereum-flavored Network enum passed here is an upstream
        // routing bug, not an operator error — surface it loudly.
        Network::Ethereum(_) => Err(Error::Rpc(format!(
            "polygon fee: unsupported network {network:?} (Polygon CLI)"
        ))),
    }
}

/// Fetch the live EIP-1559 fee estimate for `network`.
///
/// `rpc_url` overrides the per-network default when `Some`. The chain_id
/// returned by the RPC must equal `network.chain_id()` — a hostile or
/// misconfigured RPC that reports a different chain is rejected as
/// `Error::InvalidInput` (exit 2) so operators see the mismatch at
/// the boundary, not as a silently-signed transaction.
/// RPC timeout for `fetch_fee_estimate` (10s). Without this, a slow /
/// hostile RPC at the `fee` boundary can hang the CLI indefinitely —
/// `fee` is typically the first non-write RPC call a user hits, so a
/// silent hang there blocks every downstream `wallet send`.
pub(super) const RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub async fn fetch_fee_estimate(rpc_url: Option<&str>, network: Network) -> Result<FeeEstimate> {
    let provider = build_provider(rpc_url, network)?;
    let provider_chain_id = tokio::time::timeout(RPC_TIMEOUT, provider.get_chain_id())
        .await
        .map_err(|_| Error::Rpc(format!("get_chain_id timed out after {:?}", RPC_TIMEOUT)))?
        .map_err(|e| Error::Rpc(format!("get_chain_id: {e}")))?;
    let expected_chain_id = network.chain_id();
    if provider_chain_id != expected_chain_id {
        return Err(Error::InvalidInput(format!(
            "rpc chain_id {provider_chain_id} does not match wallet network {network:?} (expected {expected_chain_id})"
        )));
    }
    let est = tokio::time::timeout(RPC_TIMEOUT, provider.estimate_eip1559_fees())
        .await
        .map_err(|_| {
            Error::Rpc(format!(
                "estimate_eip1559_fees timed out after {:?}",
                RPC_TIMEOUT
            ))
        })?
        .map_err(|e| Error::Rpc(format!("estimate_eip1559_fees: {e}")))?;
    let max_fee_per_gas_wei = est.max_fee_per_gas;
    let max_priority_fee_per_gas_wei = est.max_priority_fee_per_gas;
    Ok(FeeEstimate {
        network: network_label(network),
        chain_id: provider_chain_id,
        max_fee_per_gas_wei,
        max_priority_fee_per_gas_wei,
        max_fee_per_gas_gwei: max_fee_per_gas_wei as f64 / 1e9,
        max_priority_fee_per_gas_gwei: max_priority_fee_per_gas_wei as f64 / 1e9,
    })
}

/// Stable string mapping for the `FeeEstimate.network` JSON field.
///
/// `format!("{network:?}")` leaks Rust's Debug repr (e.g. `"Polygon(Amoy)"`)
/// into the wire contract — any future `Network` enum variant reorder
/// silently shifts the JSON. Use a fixed vocabulary instead.
fn network_label(network: Network) -> String {
    match network {
        Network::Polygon(PolygonChain::Amoy) => "polygon-amoy".into(),
        Network::Polygon(PolygonChain::Mainnet) => "polygon-mainnet".into(),
        Network::Ethereum(_) => "ethereum-unsupported".into(),
    }
}

/// Format a `FeeEstimate` for the human-readable (non-`--json`) path.
pub fn format_fee_human(est: &FeeEstimate) -> String {
    format!(
        "network: {}\nchain_id: {}\nmax_fee_per_gas: {:.3} gwei ({} wei)\nmax_priority_fee_per_gas: {:.3} gwei ({} wei)",
        est.network,
        est.chain_id,
        est.max_fee_per_gas_gwei,
        est.max_fee_per_gas_wei,
        est.max_priority_fee_per_gas_gwei,
        est.max_priority_fee_per_gas_wei,
    )
}

#[cfg(test)]
mod handler_tests {
    //! T6d-1 tests for the `polygon fee` handler. Pure (no live RPC):
    //! the RPC-touching `fetch_fee_estimate` path is exercised by the
    //! L29 operator-driven smoke in T7.

    use super::*;

    /// S7a (failing seed): FeeEstimate JSON output carries both wei and
    /// gwei representations so `--json` consumers don't have to redo
    /// the conversion. Field order matches serde's derive order.
    #[test]
    fn fee_estimate_serializes_with_wei_and_gwei_fields() {
        let est = FeeEstimate {
            network: "Polygon(Amoy)".into(),
            chain_id: 80_002,
            max_fee_per_gas_wei: 30_000_000_000,
            max_priority_fee_per_gas_wei: 30_000_000_000,
            max_fee_per_gas_gwei: 30.0,
            max_priority_fee_per_gas_gwei: 30.0,
        };
        let json = serde_json::to_string(&est).expect("serialize");
        assert!(json.contains("\"max_fee_per_gas_wei\":30000000000"));
        assert!(json.contains("\"max_fee_per_gas_gwei\":30"));
        assert!(json.contains("\"chain_id\":80002"));
    }

    /// S7b: format_fee_human surfaces both gwei and wei so operators
    /// can copy-paste either representation. chain_id present so
    /// operators spot mismatches at the boundary.
    #[test]
    fn format_fee_human_includes_both_units_and_chain_id() {
        let est = FeeEstimate {
            network: "Polygon(Mainnet)".into(),
            chain_id: 137,
            max_fee_per_gas_wei: 50_000_000_000,
            max_priority_fee_per_gas_wei: 30_000_000_000,
            max_fee_per_gas_gwei: 50.0,
            max_priority_fee_per_gas_gwei: 30.0,
        };
        let s = format_fee_human(&est);
        assert!(s.contains("chain_id: 137"));
        assert!(s.contains("50.000 gwei"));
        assert!(s.contains("50000000000 wei"));
        assert!(s.contains("30.000 gwei"));
        assert!(s.contains("30000000000 wei"));
    }

    /// S7c: build_provider rejects `http://evil.example` (cleartext
    /// RPC must not cross the wire — L12 transport-security mirror).
    #[test]
    fn build_provider_rejects_non_https_non_loopback_url() {
        let r = build_provider(
            Some("http://evil.example/rpc"),
            Network::Polygon(PolygonChain::Amoy),
        );
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("not allowed"),
                    "InvalidInput must name the rejected scheme; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// S7d: build_provider accepts `https://...` URLs (happy path) —
    /// fails only at the RPC connect, which we don't reach without a
    /// real server, so we expect a `new_http` Ok result (provider
    /// construction is lazy; no network call yet).
    #[test]
    fn build_provider_accepts_https_url() {
        let r = build_provider(
            Some("https://polygon-rpc.example/"),
            Network::Polygon(PolygonChain::Mainnet),
        );
        assert!(
            r.is_ok(),
            "https URL must build a provider (lazy connection); got {r:?}"
        );
    }

    /// S7e: build_provider accepts `http://localhost` (regtest case)
    /// — L12 transport-security carve-out for local development.
    #[test]
    fn build_provider_accepts_loopback_http() {
        let r = build_provider(
            Some("http://127.0.0.1:8545/"),
            Network::Polygon(PolygonChain::Amoy),
        );
        assert!(r.is_ok(), "loopback http must build a provider");
    }
}

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
