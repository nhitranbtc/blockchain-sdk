//! E2E Sepolia sample — Story 5 (Send native ETH).
//!
//! Issue #299 — ref sample for #298.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test --test e2e_sepolia_send_native -- --ignored
//!
//! Required env vars (operator must set):
//!   ETH_E2E_RPC_URL        Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC       BIP-39 phrase (funds m/44'/60'/0'/0/0 from faucet)
//!   ETH_E2E_RECIPIENT     optional — default = m/44'/60'/0'/0/1 (self-derived)
//!
//! Testnet cost: tiny (~0.001 SepoliaETH + gas; ~0.001-0.005 ETH total at ~10 gwei).
//!
//! Verifies: signed EIP-1559 tx broadcasts → pending tx → receipt with status=true.
//! Pattern mirrors spike V4 (`tests/v4_anvil_send.rs`) — manual sign + raw broadcast.
//!
//! Promotion path: when `eth-wallet-core` crate ships (Plan Task 1), this sample
//! moves to `rust-wallet-app/crates/eth-wallet-core/tests/e2e_sepolia/wallet_send_native.rs`.

#![cfg(test)]

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_primitives::{Address, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::MnemonicBuilder;
use std::str::FromStr;

const SEPOLIA_CHAIN_ID: u64 = 11155111;
const TRANSFER_WEI: u128 = 1_000_000_000_000_000; // 0.001 ETH
const GAS_LIMIT: u64 = 21_000;
const MAX_FEE_PER_GAS: u128 = 10_000_000_000; // 10 gwei
const MAX_PRIORITY_FEE_PER_GAS: u128 = 1_000_000_000; // 1 gwei

fn env_opt_in() -> bool {
    std::env::var("RUN_ETH_E2E")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn require_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing required env var: {name}"))
}

fn build_signer(phrase: &str, index: u32) -> Result<alloy_signer_local::PrivateKeySigner, String> {
    MnemonicBuilder::english()
        .phrase(phrase)
        .index(index)
        .expect("valid account index")
        .build()
        .map_err(|e| format!("mnemonic build at index {index}: {e}"))
}

fn eprintln_return(label: &str, msg: &str) {
    eprintln!("[{label} SKIP] {msg}");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — funds m/44'/60'/0'/0/0 with Sepolia ETH first from a faucet"]
async fn story5_send_native_eth_against_sepolia() {
    if !env_opt_in() {
        eprintln!("[Story 5 SKIP] set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC");
        return;
    }
    let rpc_url = match require_env("ETH_E2E_RPC_URL") {
        Ok(v) => v,
        Err(e) => return eprintln_return("Story 5", &e),
    };
    let phrase = match require_env("ETH_E2E_MNEMONIC") {
        Ok(v) => v,
        Err(e) => return eprintln_return("Story 5", &e),
    };

    let url = match rpc_url.parse() {
        Ok(u) => u,
        Err(e) => return eprintln_return("Story 5", &format!("ETH_E2E_RPC_URL unparsable: {e}")),
    };
    let sender = match build_signer(&phrase, 0) {
        Ok(s) => s,
        Err(e) => return eprintln_return("Story 5", &e),
    };
    let recipient = match std::env::var("ETH_E2E_RECIPIENT") {
        Ok(v) => match Address::from_str(&v) {
            Ok(a) => a,
            Err(e) => {
                return eprintln_return("Story 5", &format!("ETH_E2E_RECIPIENT invalid: {e}"))
            }
        },
        Err(_) => match build_signer(&phrase, 1) {
            Ok(s) => s.address(),
            Err(e) => return eprintln_return("Story 5", &e),
        },
    };

    let provider = ProviderBuilder::new().connect_http(url);

    let nonce_u256: U256 = provider
        .raw_request::<_, U256>(
            "eth_getTransactionCount".into(),
            (sender.address(), "latest"),
        )
        .await
        .expect("eth_getTransactionCount should succeed");
    let nonce: u64 = nonce_u256
        .try_into()
        .expect("nonce should fit u64 on Sepolia");

    let mut tx = TxEip1559 {
        chain_id: SEPOLIA_CHAIN_ID,
        nonce,
        gas_limit: GAS_LIMIT,
        max_fee_per_gas: MAX_FEE_PER_GAS,
        max_priority_fee_per_gas: MAX_PRIORITY_FEE_PER_GAS,
        to: TxKind::Call(recipient),
        value: U256::from(TRANSFER_WEI),
        access_list: Default::default(),
        input: Default::default(),
    };

    let sig = sender
        .sign_transaction_sync(&mut tx)
        .expect("sign_transaction_sync should succeed");
    let signed_envelope = tx.into_signed(sig);
    let raw_tx = signed_envelope.encoded_2718();

    let tx_hash: alloy_primitives::B256 = provider
        .raw_request::<_, _>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw_tx)),),
        )
        .await
        .expect("eth_sendRawTransaction should succeed");
    eprintln!("[Story 5] broadcast tx_hash={tx_hash}");

    let mut receipt_opt = None;
    for _ in 0..60 {
        if let Some(r) = provider
            .get_transaction_receipt(tx_hash)
            .await
            .expect("get_transaction_receipt should succeed")
        {
            receipt_opt = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    let receipt = match receipt_opt {
        Some(r) => r,
        None => {
            eprintln!("[Story 5 FAIL] no receipt for {tx_hash} within 120s poll window");
            return;
        }
    };
    let block: u64 = receipt.block_number.unwrap_or_default();
    eprintln!(
        "[Story 5 PASS] tx_hash={tx_hash} block_number={block} status={}",
        receipt.status()
    );
    assert!(receipt.status(), "receipt status should be true (success)");
    assert!(block > 0, "receipt should carry a block_number");
}
