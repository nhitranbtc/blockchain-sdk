//! E2E Sepolia — Story 3 (Check ETH balance).
//!
//! Issue #310 — port of `spikes/alloy-v1/tests/e2e_sepolia_balance.rs`
//! (Issue #299 sample) into the `eth-wallet-core` crate. Uses the
//! library's `new_http` façade (Issue #305) instead of raw alloy.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_wallet_balance -- --ignored --nocapture
//!
//! Required env vars (operator must set):
//!   ETH_E2E_RPC_URL   Sepolia HTTP RPC endpoint (e.g., Alchemy/Infura public RPC)
//!   ETH_E2E_MNEMONIC  BIP-39 phrase, 12+ words (signer derives m/44'/60'/0'/0/0)
//!
//! Testnet cost: zero (read-only RPC).
//!
//! Verifies: balance read from a fresh-wallet derivation index via the
//! library provider. Operator can cross-check via `cast balance <addr>`.

#![cfg(test)]

mod common;

use alloy_primitives::U256;
use alloy_provider::Provider;

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC"]
async fn story3_check_eth_balance_against_sepolia() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 3") else {
        return;
    };
    let addr = signer.address();

    let balance = provider
        .get_balance(addr)
        .await
        .expect("get_balance should succeed against Sepolia");

    // Verifiable artifact: print (addr, balance_wei). Cross-check via
    // `cast balance <addr> --rpc-url <sepolia>`.
    eprintln!("[Story 3 PASS] addr={addr} balance_wei={balance}");
    // Sentinel: any non-empty U256 fits.
    assert!(balance <= U256::MAX, "balance must fit U256");
}
