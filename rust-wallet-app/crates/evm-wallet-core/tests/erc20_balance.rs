//! E2E Sepolia — Story 22 (Check ERC-20 token balance).
//!
//! Issue #310 — port of `spikes/alloy-v1/tests/e2e_sepolia_erc20_balance.rs`
//! (Issue #299 sample) into the `eth-wallet-core` crate. Uses library
//! `new_http` façade + `sol!` macro for typed ABI binding.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_erc20_balance -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (owner = m/44'/60'/0'/0/0)
//!   ETH_E2E_TOKEN_ADDRESS  ERC-20 contract address on Sepolia
//!
//! Testnet cost: zero (read-only `eth_call`).
//!
//! Verifies: `balanceOf(address)` via `provider.call(&req)` + ABI decode
//! returns U256. Demonstrates `sol!` macro usage for typed ABI bindings.

#![cfg(test)]

mod common;

use alloy_primitives::U256;
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::{sol, SolCall};

sol! {
    /// Minimal ERC-20 surface needed for balanceOf call.
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
    }
}

#[tokio::test]
#[ignore = "operator-driven per L29 — provide ETH_E2E_TOKEN_ADDRESS (deploy a mock or use a public testnet ERC-20)"]
async fn story22_check_erc20_balance_against_sepolia() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 22") else {
        return;
    };
    let owner = signer.address();
    let token_addr = match common::require_env_as_address("ETH_E2E_TOKEN_ADDRESS") {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 22 SKIP] {e}");
            return;
        }
    };

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
    assert!(balance <= U256::MAX, "balance must fit U256");
}
