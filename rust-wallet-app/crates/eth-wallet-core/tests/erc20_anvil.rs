//! Task 9 (Issue #308) — Anvil regtest for the eth-wallet-core crate.
//!
//! V6 spike evidence: `rust-wallet-app/spikes/alloy-v1/tests/v6_erc20_anvil.rs`.
//!
//! Per L29 + Q8: live network smoke is operator-driven — this test is
//! `#[ignore]` (NEVER runs in CI) and requires `RUN_ANVIL_E2E=1` to
//! execute. The chain-id sanity check + bundled-registry parse tests
//! run ALWAYS (no Anvil spawn).

use alloy_node_bindings::Anvil;
use alloy_provider::Provider;

use eth_wallet_core::provider::new_http;
use eth_wallet_core::tokens::{load_chain, lookup_by_symbol};

/// Always-on smoke: confirm the bundled registry parses for known chain
/// ids + that `load_chain` returns the empty list for unknown chain
/// ids. No network I/O.
#[test]
fn bundled_registry_parses_for_all_supported_chain_ids() {
    assert!(
        load_chain(1).expect("mainnet JSON parses").len() >= 2,
        "mainnet should have at least USDC + USDT"
    );
    assert!(
        !load_chain(11155111)
            .expect("sepolia JSON parses")
            .is_empty(),
        "sepolia should have at least USDC"
    );
    assert!(
        load_chain(31337).expect("anvil JSON parses").is_empty(),
        "anvil stub is empty in v0.2"
    );
}

#[test]
fn lookup_by_symbol_resolves_lowercase_usdc_mainnet() {
    let found = lookup_by_symbol(1, "usdc")
        .expect("lookup ok")
        .expect("USDC should exist in mainnet registry");
    assert_eq!(found.symbol.to_uppercase(), "USDC");
    assert_eq!(found.decimals, 6);
    assert_eq!(found.chain_id, 1);
}

/// L29-gated Anvil smoke. Spawns a local Anvil via `alloy-node-bindings`,
/// opens a `RootProvider` against it, and verifies the chain-id sanity
/// + a single `get_block_number()` round-trip. No token round-trip here
/// (MockERC20 deploy lives in a follow-up to keep this PR compact; see
/// V6 spike for the full deploy/transfer round-trip).
///
/// Run with:
/// ```bash
/// RUN_ANVIL_E2E=1 cargo test --test erc20_anvil -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run"]
async fn anvil_chain_id_and_block_number() {
    if std::env::var("RUN_ANVIL_E2E").ok().as_deref() != Some("1") {
        eprintln!("[anvil regtest] SKIP — set RUN_ANVIL_E2E=1 to run");
        return;
    }

    let anvil = Anvil::new().spawn();
    let endpoint: reqwest::Url = anvil.endpoint().parse().expect("valid Anvil endpoint");
    eprintln!("[anvil regtest] spawned Anvil at {endpoint}");

    let provider = new_http(endpoint).expect("provider construction");
    let chain_id = provider.get_chain_id().await.expect("chain id");
    assert_eq!(chain_id, 31337, "anvil default chain id is 31337 (0x7a69)");

    let block_number = provider.get_block_number().await.expect("block number");
    eprintln!("[anvil regtest] get_block_number -> {block_number}");
}
