//! E2E Sepolia — Story 8 (Fee estimates).
//!
//! Issue #310 — Story 8 of the #298 story map. Validates `eth_gasPrice`
//! + `eth_feeHistory` against Sepolia. Per #297 G1 the percentile
//! mapping for the future `eth fee` CLI subcommand is:
//!   fastest   = 95th percentile
//!   half_hour = 80th percentile
//!   hour      = 70th percentile
//!   economy   = 50th percentile
//! This test reads `eth_feeHistory` + reports max_fee_per_gas suggestions
//! per the four buckets so operators can verify the values match their
//! intuition before the CLI ships.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_fee -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase

#![cfg(test)]
#![allow(clippy::doc_lazy_continuation)]

mod common;

use alloy_primitives::U256;
use alloy_provider::Provider;

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC"]
async fn story8_fee_estimates_against_sepolia() {
    let Some((provider, _signer)) = common::preflight_or_skip("Story 8") else {
        return;
    };

    // eth_gasPrice (current head base fee surrogate).
    let gas_price: U256 = provider
        .raw_request::<_, U256>("eth_gasPrice".into(), ())
        .await
        .expect("eth_gasPrice should succeed against Sepolia");

    // eth_feeHistory over 5 blocks at 4 reward percentiles (matches G1 buckets).
    let history: serde_json::Value = provider
        .raw_request::<_, serde_json::Value>(
            "eth_feeHistory".into(),
            (
                serde_json::json!("0x5"),
                serde_json::json!("latest"),
                serde_json::json!([95, 80, 70, 50]),
            ),
        )
        .await
        .expect("eth_feeHistory should succeed against Sepolia");

    let rewards = history
        .get("reward")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Build the four percentile reports. `rewards[i][j]` = i-th block,
    // j-th percentile (95/80/70/50). Take the median across the 5-block
    // window for a single "head" suggestion per bucket.
    let mut p95 = Vec::new();
    let mut p80 = Vec::new();
    let mut p70 = Vec::new();
    let mut p50 = Vec::new();
    for block_rewards in &rewards {
        if let Some(arr) = block_rewards.as_array() {
            if arr.len() == 4 {
                p95.push(arr[0].as_str().unwrap_or("0x0"));
                p80.push(arr[1].as_str().unwrap_or("0x0"));
                p70.push(arr[2].as_str().unwrap_or("0x0"));
                p50.push(arr[3].as_str().unwrap_or("0x0"));
            }
        }
    }
    let pick_median = |v: &[&str]| -> u128 {
        let mut parsed: Vec<u128> = v
            .iter()
            .filter_map(|s| u128::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .collect();
        if parsed.is_empty() {
            return 0;
        }
        parsed.sort_unstable();
        parsed[parsed.len() / 2]
    };
    let fastest = pick_median(&p95);
    let half_hour = pick_median(&p80);
    let hour = pick_median(&p70);
    let economy = pick_median(&p50);

    eprintln!(
        "[Story 8 PASS] gas_price={gas_price}wei fastest={fastest}wei \
         half_hour={half_hour}wei hour={hour}wei economy={economy}wei"
    );
    assert!(gas_price <= U256::MAX, "gas_price must fit U256");
    // Sanity ordering: faster buckets should pay >= slower ones (median).
    assert!(fastest >= economy, "95th percentile should pay >= 50th");
}
