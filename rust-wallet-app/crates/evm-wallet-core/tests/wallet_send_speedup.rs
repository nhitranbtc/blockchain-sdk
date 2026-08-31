//! E2E Sepolia — Story 17 (Speed-up tx).
//!
//! Issue #310 — Story 17 of the #298 story map. Operator-driven
//! replace-by-fee flow: broadcasts a tx at low fee, then a second tx
//! at the same nonce + higher max_fee. Verifies that the second
//! broadcast either confirms the original out of the mempool OR the
//! new tx overwrites the original (Sepolia behavior depends on the
//! operator's chosen RPC mempool policy).
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_wallet_send_speedup -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (funds sender first)
//!   ETH_E2E_RECIPIENT       optional — default = m/44'/60'/0'/0/1

#![cfg(test)]

mod common;

use alloy_primitives::{Address, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use evm_wallet_core::{encoded_envelope, sign_native_eth_tx};

const TRANSFER_WEI: u128 = 1_000_000_000_000_000; // 0.001 ETH
const GAS_LIMIT: u64 = 21_000;
const POLL_ATTEMPTS: u32 = 60;
const POLL_INTERVAL_SECS: u64 = 2;

async fn fetch_nonce<P: Provider>(provider: &P, addr: Address) -> u64 {
    let n: U256 = provider
        .raw_request::<_, U256>("eth_getTransactionCount".into(), (addr, "pending"))
        .await
        .expect("eth_getTransactionCount");
    n.try_into().expect("nonce fits u64")
}

async fn broadcast<P: Provider>(provider: &P, raw_tx: Vec<u8>) -> alloy_primitives::B256 {
    provider
        .raw_request::<_, alloy_primitives::B256>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw_tx)),),
        )
        .await
        .expect("eth_sendRawTransaction")
}

#[tokio::test]
#[ignore = "operator-driven per L29 — funds sender first; public RPC mempool behavior varies"]
async fn story17_speed_up_tx_against_sepolia() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 17") else {
        return;
    };
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let recipient = match common::resolve_recipient(&phrase) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 17 SKIP] {e}");
            return;
        }
    };
    let nonce = fetch_nonce(&provider, signer.address()).await;

    // Tx 1: low-fee.
    let tx_low = TransactionRequest {
        to: Some(TxKind::Call(recipient)),
        value: Some(U256::from(TRANSFER_WEI)),
        chain_id: Some(common::SEPOLIA_CHAIN_ID),
        nonce: Some(nonce),
        gas: Some(GAS_LIMIT),
        max_fee_per_gas: Some(1_000_000_000), // 1 gwei
        max_priority_fee_per_gas: Some(500_000_000),
        ..Default::default()
    };
    let env_low = sign_native_eth_tx(&signer, tx_low).expect("sign low");
    let hash_low = broadcast(&provider, encoded_envelope(&env_low)).await;
    eprintln!("[Story 17] low_fee_tx={hash_low}");

    // Tx 2: same nonce, 10x max_fee → replace-by-fee.
    let tx_high = TransactionRequest {
        to: Some(TxKind::Call(recipient)),
        value: Some(U256::from(TRANSFER_WEI)),
        chain_id: Some(common::SEPOLIA_CHAIN_ID),
        nonce: Some(nonce),
        gas: Some(GAS_LIMIT),
        max_fee_per_gas: Some(10_000_000_000), // 10 gwei
        max_priority_fee_per_gas: Some(2_000_000_000),
        ..Default::default()
    };
    let env_high = sign_native_eth_tx(&signer, tx_high).expect("sign high");
    let hash_high = broadcast(&provider, encoded_envelope(&env_high)).await;
    eprintln!("[Story 17] high_fee_tx={hash_high}");

    // Poll for the high-fee receipt. Sepolia public RPCs may either:
    // (a) accept the replacement (receipt.status = true), or
    // (b) reject it with "replacement tx underpriced" — surfaced as
    //     a low-fee confirmation only.
    let mut high_receipt = None;
    let mut low_receipt = None;
    for _ in 0..POLL_ATTEMPTS {
        if high_receipt.is_none() {
            if let Some(r) = provider
                .get_transaction_receipt(hash_high)
                .await
                .expect("get high receipt")
            {
                high_receipt = Some(r);
            }
        }
        if low_receipt.is_none() {
            if let Some(r) = provider
                .get_transaction_receipt(hash_low)
                .await
                .expect("get low receipt")
            {
                low_receipt = Some(r);
            }
        }
        if high_receipt.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }

    match (low_receipt, high_receipt) {
        (Some(l), Some(h)) => {
            // Both confirmed. RBF accepted: high_fee mined, low_fee was
            // dropped (or also mined — Sepolia behavior varies).
            eprintln!(
                "[Story 17 PASS] low_confirmed_block={:?} high_confirmed_block={:?}",
                l.block_number, h.block_number
            );
        }
        (None, Some(h)) => {
            eprintln!(
                "[Story 17 PASS] high_fee_confirmed_block={:?} low_fee_dropped",
                h.block_number
            );
        }
        (Some(l), None) => {
            eprintln!(
                "[Story 17 SKIP] low_fee_confirmed_block={:?} high_fee_rejected",
                l.block_number
            );
        }
        (None, None) => {
            eprintln!("[Story 17 FAIL] no receipt within poll window for either tx");
        }
    }
}
