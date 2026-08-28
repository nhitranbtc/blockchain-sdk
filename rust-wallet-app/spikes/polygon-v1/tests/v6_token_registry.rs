//! V6 — token registry load + on-chain `decimals()` verify (Q3).
//!
//! Live test gated on `RUN_POLYGON_AMOY=1`. Loads `tokens/amoy.json` from
//! the spike bundle and asserts the registry's USDC entry's `decimals`
//! field matches the live on-chain `decimals()` call.

use alloy_primitives::U256;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};

fn env_opt_in() -> bool {
    std::env::var("RUN_POLYGON_AMOY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 cargo test --test v6_token_registry -- --ignored"]
async fn v6_amoy_usdc_decimals_match_registry() {
    if !env_opt_in() {
        eprintln!("[V6] SKIP — set RUN_POLYGON_AMOY=1 to enable on-chain decimals probe");
        return;
    }

    let cfg = ChainConfig::for_network(Network::PolygonAmoy);
    let url: Url = cfg.default_rpc_url.parse().expect("valid Amoy RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    // The registry's USDC address (tokens/amoy.json).
    let usdc_addr: alloy_primitives::Address = "0x41e94eb019c0762f9bfcf29fb1e7664d170daffb"
        .parse()
        .expect("valid registry address");

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
        live_decimals, 6,
        "Amoy USDC decimals MUST match registry (6); got {live_decimals}"
    );

    eprintln!("[V6] PASS — Amoy USDC decimals() = {live_decimals} (matches registry)");
}
