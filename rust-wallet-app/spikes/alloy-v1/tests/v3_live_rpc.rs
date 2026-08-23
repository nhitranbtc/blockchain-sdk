//! V3 verification — `provider.get_block_number()` against
//! `https://ethereum.reth.rs/rpc` returns a sane value.
//!
//! Issue #293 — verification item V3.
//!
//! Per L29, this test is `#[ignore]` by default and only runs when the
//! operator opts in with `RUN_V3_LIVE_RPC=1 cargo test`. Network call goes to
//! a public Reth RPC endpoint; no credentials required.

#![cfg(test)]

use alloy_provider::{Provider, ProviderBuilder};

const RETH_RPC_URL: &str = "https://ethereum.reth.rs/rpc";

fn env_opt_in() -> bool {
    std::env::var("RUN_V3_LIVE_RPC")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_V3_LIVE_RPC=1 cargo test --test v3_live_rpc -- --ignored"]
async fn v3_get_block_number_against_reth_rpc() {
    if !env_opt_in() {
        eprintln!("[V3] SKIP — set RUN_V3_LIVE_RPC=1 to enable live RPC call");
        return;
    }

    let url = RETH_RPC_URL.parse().expect("valid Reth RPC URL");
    let provider = ProviderBuilder::new().connect_http(url);

    let block_number = provider
        .get_block_number()
        .await
        .expect("get_block_number should succeed against reth.rs");

    // Sanity: block number is non-zero and plausible (post-merge = > 15_537_393
    // on Sep 15 2022; today is well past that).
    assert!(
        block_number > 15_537_393,
        "block number {block_number} is below merge height",
    );

    eprintln!("[V3] PASS — reth.rs returned block_number = {block_number} (sane post-merge value)",);
}
