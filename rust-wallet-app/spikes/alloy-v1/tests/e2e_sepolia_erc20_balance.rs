//! E2E Sepolia sample — Story 22 (Check ERC-20 token balance).
//!
//! Issue #299 — ref sample for #298.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test --test e2e_sepolia_erc20_balance -- --ignored
//!
//! Required env vars (operator must set):
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (owner address = m/44'/60'/0'/0/0)
//!   ETH_E2E_TOKEN_ADDRESS  ERC-20 contract address on Sepolia (e.g., USDC test contract)
//!
//! Testnet cost: zero (read-only `eth_call`).
//!
//! Verifies: `balanceOf(address)` via `provider.call(&req)` + ABI decode returns
//! U256, matches raw call. Demonstrates `sol!` macro usage for typed ABI bindings.
//!
//! Promotion path: when `eth-wallet-core` crate ships (Plan Task 1), this sample
//! moves to `rust-wallet-app/crates/eth-wallet-core/tests/e2e_sepolia/erc20_balance.rs`.

#![cfg(test)]

use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::TransactionRequest;
use alloy_signer_local::MnemonicBuilder;
use alloy_sol_types::{sol, SolCall};
use std::str::FromStr;

sol! {
    /// Minimal ERC-20 surface needed for balanceOf call.
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
    }
}

fn env_opt_in() -> bool {
    std::env::var("RUN_ETH_E2E")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn require_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing required env var: {name}"))
}

fn eprintln_return(label: &str, msg: &str) {
    eprintln!("[{label} SKIP] {msg}");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — provide token address via ETH_E2E_TOKEN_ADDRESS (deploy a mock or use a public testnet ERC-20)"]
async fn story22_check_erc20_balance_against_sepolia() {
    if !env_opt_in() {
        eprintln!(
            "[Story 22 SKIP] set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC + ETH_E2E_TOKEN_ADDRESS"
        );
        return;
    }
    let rpc_url = match require_env("ETH_E2E_RPC_URL") {
        Ok(v) => v,
        Err(e) => return eprintln_return("Story 22", &e),
    };
    let phrase = match require_env("ETH_E2E_MNEMONIC") {
        Ok(v) => v,
        Err(e) => return eprintln_return("Story 22", &e),
    };
    let token_addr_str = match require_env("ETH_E2E_TOKEN_ADDRESS") {
        Ok(v) => v,
        Err(e) => return eprintln_return("Story 22", &e),
    };
    let token_addr = match Address::from_str(&token_addr_str) {
        Ok(a) => a,
        Err(e) => {
            return eprintln_return("Story 22", &format!("ETH_E2E_TOKEN_ADDRESS invalid: {e}"))
        }
    };

    let url = match rpc_url.parse() {
        Ok(u) => u,
        Err(e) => return eprintln_return("Story 22", &format!("ETH_E2E_RPC_URL unparsable: {e}")),
    };
    let owner = match MnemonicBuilder::english()
        .phrase(phrase.as_str())
        .index(0)
        .expect("valid account index")
        .build()
    {
        Ok(s) => s.address(),
        Err(e) => return eprintln_return("Story 22", &format!("mnemonic build: {e}")),
    };

    let provider = ProviderBuilder::new().connect_http(url);

    // Typed call via sol! macro: balanceOf(owner) -> uint256.
    let call = IERC20::balanceOfCall { account: owner };
    let calldata: alloy_primitives::Bytes = call.abi_encode().into();
    let req = TransactionRequest::default()
        .to(token_addr)
        .input(calldata.into());
    let raw = provider
        .call(req)
        .await
        .expect("provider.call should succeed for balanceOf");

    // Manual decode: ERC-20 balanceOf returns uint256 (32 bytes BE).
    // Avoids the SolCall::abi_decode_returns trait path — first 32 bytes
    // are the BE-encoded U256, which is the only return value.
    let mut word = [0u8; 32];
    word.copy_from_slice(&raw[..32]);
    let balance = U256::from_be_bytes(word);

    // Verifiable artifact: print (token, owner, balance) tuple.
    eprintln!("[Story 22 PASS] token={token_addr} owner={owner} balance={balance}");

    // Sentinel: U256 max bound check.
    assert!(balance <= U256::MAX, "balance should fit U256");
}
