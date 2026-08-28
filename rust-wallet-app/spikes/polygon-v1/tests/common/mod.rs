//! Shared helpers for V1-V10 + use_case integration tests.
//!
//! L12 critical-tier review (commit `f2a7c3b` planned) extracted:
//! - `env_opt_in` — duplicated 4× across v2/v4/v5/v7 (type-design MEDIUM + code-reviewer MEDIUM)
//! - `await_receipt` — receipt-poll loop duplicated 4× across v8/v9/use_case
//! - `load_token_registry` — V6 was hardcoding the USDC address instead of
//!   reading `tokens/amoy.json` (type-design HIGH + code-reviewer MEDIUM)

use std::time::Duration;

use alloy_provider::Provider;
use polygon_v1_spike::config::Network;
use polygon_v1_spike::tokens::TokenEntry;

/// Returns true if the operator-driven env var is set ("1" or "true",
/// case-insensitive). Used as the gate for every `#[ignore]` live test.
#[allow(dead_code)] // used by #[ignore]'d live tests that don't compile in offline `cargo test`
pub fn env_opt_in(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Poll for a transaction receipt up to `attempts` times, sleeping
/// `interval` between tries. Returns `Some(receipt)` on first hit,
/// `None` if the deadline expires. Shared across v8/v9/use_case so the
/// timeout/retry knobs stay in one place.
#[allow(dead_code)] // used by v8 + use_case (#[tokio::test]'d tests that compile in offline `cargo test` but may be flagged depending on cfg)
pub async fn await_receipt<P>(
    provider: &P,
    tx_hash: alloy_primitives::B256,
    attempts: u32,
    interval: Duration,
) -> Option<alloy_rpc_types::TransactionReceipt>
where
    P: Provider,
{
    for _ in 0..attempts {
        if let Ok(Some(r)) = provider.get_transaction_receipt(tx_hash).await {
            return Some(r);
        }
        tokio::time::sleep(interval).await;
    }
    None
}

/// Load `tokens/amoy.json` (or `tokens/mainnet.json`) bundled with the
/// crate at compile time. Wired into V6 so the test asserts on-chain
/// decimals match the *bundled* registry (V6's stated thesis), not a
/// hardcoded address (L12 finding).
#[allow(dead_code)] // used by V6 (#[ignore]'d live test that doesn't compile offline)
pub fn load_token_registry(network: Network) -> Vec<TokenEntry> {
    let raw: &str = match network {
        Network::Polygon => include_str!("../../tokens/mainnet.json"),
        Network::PolygonAmoy => include_str!("../../tokens/amoy.json"),
        // No bundled Ethereum mainnet registry — token surface lives on
        // EVM chains; if Ethereum is needed, the caller can wire it.
        Network::Ethereum => "[]",
    };
    serde_json::from_str(raw).expect("bundled tokens/*.json must parse")
}
