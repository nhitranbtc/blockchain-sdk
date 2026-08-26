//! E2E Sepolia — Story 25 (ERC-20 approve).
//!
//! Issue #310 — Story 25 of the #298 story map. Builds ERC-20
//! `approve(spender, amount)` calldata + signs + broadcasts. Then
//! queries `allowance(owner, spender)` via `sol!` typed call to
//! verify the broadcast landed (Story 24-style user-token registry
//! remains out-of-scope until Task 10 CLI ships).
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_erc20_approve -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (sender = owner)
//!   ETH_E2E_TOKEN_ADDRESS  ERC-20 contract address on Sepolia
//!   ETH_E2E_SPENDER         spender address (defaults to m/44'/60'/0'/0/2 if unset)
//!   ETH_E2E_TOKEN_DECIMALS optional — default 18
//!   ETH_E2E_TOKEN_AMOUNT   optional — human-readable amount (default "1")

#![cfg(test)]

mod common;

use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_signer_local::MnemonicBuilder;
use alloy_sol_types::{sol, SolCall};
use eth_wallet_core::{encoded_envelope, sign_erc20_tx_bytes};

sol! {
    interface IERC20 {
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

const ERC20_GAS_LIMIT: u64 = 65_000;
const MAX_FEE_PER_GAS: u128 = 10_000_000_000;
const MAX_PRIORITY_FEE_PER_GAS: u128 = 1_000_000_000;
const POLL_ATTEMPTS: u32 = 60;
const POLL_INTERVAL_SECS: u64 = 2;

#[tokio::test]
#[ignore = "operator-driven per L29 — funds sender with gas ETH; approve amount must fit token balance"]
async fn story25_erc20_approve_against_sepolia() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 25") else {
        return;
    };
    let token_addr = match common::require_env_as_address("ETH_E2E_TOKEN_ADDRESS") {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 25 SKIP] {e}");
            return;
        }
    };
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let spender = match std::env::var("ETH_E2E_SPENDER") {
        Ok(v) => match v.parse::<Address>() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("[Story 25 SKIP] ETH_E2E_SPENDER invalid: {e}");
                return;
            }
        },
        Err(_) => match MnemonicBuilder::english()
            .phrase(phrase.as_str())
            .index(2)
            .expect("valid index")
            .build()
        {
            Ok(s) => s.address(),
            Err(e) => {
                eprintln!("[Story 25 SKIP] mnemonic build index 2: {e}");
                return;
            }
        },
    };

    let decimals: u8 = std::env::var("ETH_E2E_TOKEN_DECIMALS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(18u8);
    let human_amount: u128 = std::env::var("ETH_E2E_TOKEN_AMOUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1u128);
    let base_units = human_amount
        .checked_mul(10u128.pow(decimals as u32))
        .expect("amount fits u128");
    let amount = U256::from(base_units);

    // approve(spender, amount). Selector = 0x095ea7b3.
    let call = IERC20::approveCall { spender, amount };
    let calldata: alloy_primitives::Bytes = call.abi_encode().into();
    assert_eq!(
        &calldata[..4],
        &[0x09, 0x5e, 0xa7, 0xb3],
        "selector must be keccak256('approve(address,uint256)')"
    );

    let sender_addr = signer.address();
    let nonce: u64 = {
        let n: U256 = provider
            .raw_request::<_, U256>("eth_getTransactionCount".into(), (sender_addr, "pending"))
            .await
            .expect("eth_getTransactionCount");
        n.try_into().expect("nonce fits u64")
    };

    let signed = sign_erc20_tx_bytes(
        &signer,
        token_addr,
        calldata,
        U256::ZERO,
        nonce,
        common::SEPOLIA_CHAIN_ID,
        MAX_FEE_PER_GAS,
        MAX_PRIORITY_FEE_PER_GAS,
        ERC20_GAS_LIMIT,
    )
    .expect("sign_erc20_tx_bytes");
    let raw = encoded_envelope(&signed);
    let tx_hash: alloy_primitives::B256 = provider
        .raw_request::<_, _>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw)),),
        )
        .await
        .expect("eth_sendRawTransaction");
    eprintln!("[Story 25] broadcast tx_hash={tx_hash} token={token_addr} spender={spender} amount={amount}");

    // Poll for confirmation.
    let mut receipt_opt = None;
    for _ in 0..POLL_ATTEMPTS {
        if let Some(r) = provider
            .get_transaction_receipt(tx_hash)
            .await
            .expect("get_transaction_receipt")
        {
            receipt_opt = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
    let receipt = match receipt_opt {
        Some(r) => r,
        None => {
            eprintln!("[Story 25 FAIL] no receipt for {tx_hash} within poll window");
            return;
        }
    };
    let block = receipt.block_number.unwrap_or_default();
    assert!(receipt.status(), "Story 25 receipt status must be true");
    assert!(block > 0, "Story 25 receipt must carry block_number");

    // Verify allowance changed to `amount` via typed allowance query.
    let allow_call = IERC20::allowanceCall {
        owner: sender_addr,
        spender,
    };
    let allow_calldata: alloy_primitives::Bytes = allow_call.abi_encode().into();
    let allow_req = TransactionRequest::default()
        .to(token_addr)
        .input(allow_calldata.into());
    let raw_allow = provider
        .call(allow_req)
        .await
        .expect("provider.call should succeed for allowance");
    let mut word = [0u8; 32];
    word.copy_from_slice(&raw_allow[..32]);
    let onchain_allowance = U256::from_be_bytes(word);

    eprintln!(
        "[Story 25 PASS] tx_hash={tx_hash} block={block} onchain_allowance={onchain_allowance}"
    );
    assert_eq!(
        onchain_allowance, amount,
        "on-chain allowance must equal broadcast amount"
    );
}
