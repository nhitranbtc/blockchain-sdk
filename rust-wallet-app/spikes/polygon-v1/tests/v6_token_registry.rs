//! V6 — token registry load + on-chain `decimals()` verify (Q3).
//!
//! Live test gated on `RUN_POLYGON_AMOY=1`. Loads the bundled
//! `tokens/amoy.json` (per L12 finding — previously hardcoded the
//! USDC address; the registry is the actual test surface) and asserts
//! the first USDC entry's on-chain `decimals()` matches the registry.

use alloy_primitives::U256;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};
use polygon_v1_spike::tokens::TokenEntry;

mod common;

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 cargo test --test v6_token_registry -- --ignored"]
async fn v6_amoy_usdc_decimals_match_bundled_registry() {
    if !common::env_opt_in("RUN_POLYGON_AMOY") {
        eprintln!("[V6] SKIP — set RUN_POLYGON_AMOY=1 to enable on-chain decimals probe");
        return;
    }

    let cfg = ChainConfig::for_network(Network::PolygonAmoy);
    let url: Url = cfg.default_rpc_url.parse().expect("valid Amoy RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    // Pull USDC address from the bundled registry (L12 finding — V6's
    // stated thesis is "token registry load + on-chain decimals verify").
    let registry: Vec<TokenEntry> = common::load_token_registry(Network::PolygonAmoy);
    let usdc = registry
        .iter()
        .find(|t| t.symbol == "USDC")
        .expect("bundled tokens/amoy.json must contain a USDC entry");
    let usdc_addr: alloy_primitives::Address = usdc
        .address
        .parse()
        .expect("registry USDC address must parse");
    let registry_decimals = usdc.decimals;

    // Call decimals() — selector 0x313ce567 (keccak256("decimals()")[..4]).
    let call_result = provider
        .raw_request::<_, U256>(
            "eth_call".into(),
            (
                serde_json::json!({
                    "to": format!("{usdc_addr:?}"),
                    "data": "0x313ce567"
                }),
                "latest",
            ),
        )
        .await
        .expect("eth_call(decimals) must succeed");

    let live_decimals: u8 = u8::try_from(call_result).expect("decimals must fit in u8");
    assert_eq!(
        live_decimals, registry_decimals,
        "Amoy USDC decimals MUST match registry ({registry_decimals}); got {live_decimals}"
    );

    eprintln!(
        "[V6] PASS — Amoy USDC {usdc_addr:?} decimals() = {live_decimals} (matches bundled registry)"
    );
}
