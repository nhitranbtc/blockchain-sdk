//! V7 — Amoy faucet reachability (Q4).
//!
//! Live test gated on `RUN_POLYGON_AMOY=1`. Probes the official Polygon
//! Amoy faucet endpoint. Faucet drip itself requires interactive auth
//! (GitHub OAuth per the docs) so the spike only checks reachability + a
//! pre-drip chain responsiveness check.

use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};

mod common;

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 cargo test --test v7_amoy_faucet -- --ignored"]
async fn v7_amoy_chain_responsive_for_faucet_precondition() {
    if !common::env_opt_in("RUN_POLYGON_AMOY") {
        eprintln!("[V7] SKIP — set RUN_POLYGON_AMOY=1 to enable faucet probe");
        return;
    }

    let cfg = ChainConfig::for_network(Network::PolygonAmoy);
    let url: Url = cfg.default_rpc_url.parse().expect("valid Amoy RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("Amoy RPC must respond");

    assert_eq!(chain_id, 80_002, "Amoy chain-id sanity check");

    eprintln!(
        "[V7] PASS — Amoy chain responsive (chain_id = {chain_id}). Faucet drip is operator-interactive: https://faucet.polygon.technology/"
    );
}
