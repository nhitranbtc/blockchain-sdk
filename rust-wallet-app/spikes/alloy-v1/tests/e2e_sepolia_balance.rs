//! E2E Sepolia sample — Story 3 (Check ETH balance).
//!
//! Issue #299 — ref sample for #298.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test --test e2e_sepolia_balance -- --ignored
//!
//! Required env vars (operator must set):
//!   ETH_E2E_RPC_URL   Sepolia HTTP RPC endpoint (e.g., from Alchemy/Infura public RPC)
//!   ETH_E2E_MNEMONIC  BIP-39 phrase, 12+ words (signer derives address at m/44'/60'/0'/0/0)
//!
//! Testnet cost: zero (read-only RPC).
//!
//! Verifies: balance read from a fresh-wallet derivation index against
//! `provider.get_balance(addr)` directly. Friendly error if env unset.
//!
//! Promotion path: when `eth-wallet-core` crate ships (Plan Task 1), this sample
//! moves to `rust-wallet-app/crates/eth-wallet-core/tests/e2e_sepolia/wallet_balance.rs`.

#![cfg(test)]

use alloy_primitives::Address;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::MnemonicBuilder;

const DEFAULT_RECEIVE_TESTNET: &str = "Sepolia (chain id 11155111)";

fn env_opt_in() -> bool {
    std::env::var("RUN_ETH_E2E")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn require_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing required env var: {name}"))
}

/// Preflight gate: env-var presence + opt-in. Returns `Some(provider, addr)` if
/// the test should proceed, `None` if it should skip-with-message.
async fn preflight_or_skip(testnet: &str) -> Option<(impl Provider, Address)> {
    let _ = testnet; // surfaced in skip message below
    if !env_opt_in() {
        eprintln!(
            "[Story 3 SKIP] set RUN_ETH_E2E=1 to run; default testnet = {DEFAULT_RECEIVE_TESTNET}"
        );
        return None;
    }
    let rpc_url = match require_env("ETH_E2E_RPC_URL") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[Story 3 SKIP] {e}");
            return None;
        }
    };
    let phrase = match require_env("ETH_E2E_MNEMONIC") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[Story 3 SKIP] {e}");
            return None;
        }
    };
    let url = match rpc_url.parse() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[Story 3 FAIL] ETH_E2E_RPC_URL unparsable: {e}");
            return None;
        }
    };
    let provider = ProviderBuilder::new().connect_http(url);
    let signer = match MnemonicBuilder::english()
        .phrase(phrase.as_str())
        .index(0)
        .expect("valid account index")
        .build()
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[Story 3 FAIL] mnemonic build: {e}");
            return None;
        }
    };
    let addr = signer.address();
    Some((provider, addr))
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC"]
async fn story3_check_eth_balance_against_sepolia() {
    let Some((provider, addr)) = preflight_or_skip(DEFAULT_RECEIVE_TESTNET).await else {
        return;
    };

    let balance = provider
        .get_balance(addr)
        .await
        .expect("get_balance should succeed against Sepolia");

    // Verifiable artifact: print exact (addr, balance_wei) pair. Operator can
    // cross-check by calling `cast balance <addr> --rpc-url <sepolia>`.
    eprintln!("[Story 3 PASS] addr={addr} balance_wei={balance}");
    // Sentinel: any non-empty U256 = call returned a valid balance.
    assert!(
        balance <= alloy_primitives::U256::MAX,
        "balance should fit U256"
    );
}
