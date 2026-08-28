//! V4 — EIP-1559 fee estimates + 2-second-block baseFee cadence (Q5).
//!
//! Polls `eth_gasPrice` (legacy) + `eth_maxPriorityFeePerGas` (EIP-1559
//! tip suggestion) and asserts the gas-price invariants. Cadence is
//! asserted via a separate #[ignore] test. Gated on `RUN_POLYGON_AMOY=1`.

use std::time::{Duration, Instant};

use alloy_primitives::U256;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};

mod common;

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 cargo test --test v4_eip1559_estimates -- --ignored"]
async fn v4_amoy_base_fee_and_priority_fee_within_band() {
    if !common::env_opt_in("RUN_POLYGON_AMOY") {
        eprintln!("[V4] SKIP — set RUN_POLYGON_AMOY=1 to enable Amoy gas probe");
        return;
    }

    let cfg = ChainConfig::for_network(Network::PolygonAmoy);
    let url: Url = cfg.default_rpc_url.parse().expect("valid Amoy RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    let gas_price = provider
        .get_gas_price()
        .await
        .expect("eth_gasPrice must succeed");
    assert!(gas_price > 0, "gas_price must be positive");

    let priority_fee = provider
        .raw_request::<_, U256>("eth_maxPriorityFeePerGas".into(), ())
        .await
        .expect("eth_maxPriorityFeePerGas must succeed");

    let gas_price_u256: U256 = U256::from(gas_price);
    assert!(
        gas_price_u256 >= priority_fee,
        "gas_price ({gas_price_u256}) must be >= priority_fee ({priority_fee})"
    );

    eprintln!("[V4] PASS — gas_price={gas_price_u256} priority_fee={priority_fee}");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 cargo test --test v4_eip1559_estimates -- --ignored"]
async fn v4_amoy_block_cadence_is_approximately_2_seconds() {
    if !common::env_opt_in("RUN_POLYGON_AMOY") {
        eprintln!("[V4-cadence] SKIP — set RUN_POLYGON_AMOY=1 to enable Amoy cadence probe");
        return;
    }

    let cfg = ChainConfig::for_network(Network::PolygonAmoy);
    let url: Url = cfg.default_rpc_url.parse().expect("valid Amoy RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    let b1_start = Instant::now();
    let b1 = provider
        .get_block_number()
        .await
        .expect("eth_blockNumber must succeed");
    let mut b2 = b1;
    let deadline = Instant::now() + Duration::from_secs(10);
    while b2 == b1 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(250)).await;
        b2 = provider
            .get_block_number()
            .await
            .expect("eth_blockNumber must succeed");
    }
    let elapsed = b1_start.elapsed();

    assert!(b2 > b1, "block number must advance within 10s deadline");
    assert!(
        elapsed >= Duration::from_millis(500) && elapsed <= Duration::from_secs(10),
        "block cadence should be ~2s (got {elapsed:?})"
    );

    eprintln!("[V4-cadence] PASS — {b1} → {b2} in {elapsed:?}");
}
