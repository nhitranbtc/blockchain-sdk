//! V2 — chain-id (Q4 — RPC connectivity).
//!
//! Live test gated on `RUN_POLYGON_AMOY=1`. Hits `eth_chainId` against
//! `https://rpc-amoy.polygon.technology` and asserts the returned value
//! equals `0x13882` (80002 decimal).

use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};

fn env_opt_in() -> bool {
    std::env::var("RUN_POLYGON_AMOY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 cargo test --test v2_chain_id -- --ignored"]
async fn v2_amoy_chain_id_returns_80002() {
    if !env_opt_in() {
        eprintln!("[V2] SKIP — set RUN_POLYGON_AMOY=1 to enable Amoy RPC probe");
        return;
    }

    let cfg = ChainConfig::for_network(Network::PolygonAmoy);
    let url: Url = cfg.default_rpc_url.parse().expect("valid Amoy RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("eth_chainId must succeed against Amoy RPC");

    assert_eq!(
        chain_id, 80_002,
        "Amoy chain-id MUST be 80002 (0x13882); got {chain_id}"
    );

    eprintln!("[V2] PASS — Amoy chain_id = {chain_id} (0x{chain_id:x})");
}
