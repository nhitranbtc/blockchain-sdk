//! E2E Sepolia — Story 4 (Wallet sync).
//!
//! Issue #310 — Story 4 of the #298 story map. `wallet_sync` against
//! Sepolia reads block height + chain id as the minimal sync surface
//! that the library exposes today (Task 5/6 added `new_http`). Higher-
//! fidelity sync (account state, nonce tracking, head-watching) lands
//! in the `eth` CLI Task 10 / Story 4 implementation follow-up.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_wallet_sync -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (signer derives m/44'/60'/0'/0/0)

#![cfg(test)]

mod common;

use alloy_provider::Provider;

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC"]
async fn story4_wallet_sync_against_sepolia() {
    let Some((provider, _signer)) = common::preflight_or_skip("Story 4") else {
        return;
    };

    let chain_id = provider
        .get_chain_id()
        .await
        .expect("get_chain_id should succeed against Sepolia");
    let block_number = provider
        .get_block_number()
        .await
        .expect("get_block_number should succeed against Sepolia");

    eprintln!("[Story 4 PASS] chain_id={chain_id} block_number={block_number}");
    assert_eq!(
        chain_id,
        common::SEPOLIA_CHAIN_ID,
        "must connect to Sepolia"
    );
    assert!(block_number > 0, "Sepolia block number must be > 0");
}
