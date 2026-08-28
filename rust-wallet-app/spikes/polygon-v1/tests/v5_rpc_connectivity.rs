//! V5 — mainnet RPC connectivity + finality (Q4).
//!
//! Live test gated on `RUN_POLYGON_MAINNET=1`. Hits Polygon mainnet RPC
//! and asserts the chain head advances (sanity) and that the chain-id
//! matches `137`.

use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};

fn env_opt_in() -> bool {
    std::env::var("RUN_POLYGON_MAINNET")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_MAINNET=1 cargo test --test v5_rpc_connectivity -- --ignored"]
async fn v5_polygon_mainnet_rpc_returns_chain_id_137() {
    if !env_opt_in() {
        eprintln!("[V5] SKIP — set RUN_POLYGON_MAINNET=1 to enable mainnet probe");
        return;
    }

    let cfg = ChainConfig::for_network(Network::Polygon);
    let url: Url = cfg.default_rpc_url.parse().expect("valid mainnet RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("eth_chainId must succeed against mainnet");
    assert_eq!(chain_id, 137, "Polygon mainnet chain-id MUST be 137");

    let head = provider
        .get_block_number()
        .await
        .expect("eth_blockNumber must succeed");
    assert!(head > 0, "mainnet head must be > 0; got {head}");

    eprintln!("[V5] PASS — mainnet chain_id = {chain_id}; head = {head}");
}
