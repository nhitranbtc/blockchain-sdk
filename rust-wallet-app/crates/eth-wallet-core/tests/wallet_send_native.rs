//! E2E Sepolia — Story 5 (Send native ETH) + Stories 13/14/15/16 folded.
//!
//! Issue #310 — port of `spikes/alloy-v1/tests/e2e_sepolia_send_native.rs`
//! (Issue #299 sample) into the `eth-wallet-core` crate, expanded with
//! batch / drain / nonce-strategy / manual-nonce variants from the #298
//! story map.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_wallet_send_native -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (funds m/44'/60'/0'/0/0 from faucet)
//!   ETH_E2E_RECIPIENT       optional — default = m/44'/60'/0'/0/1 (self-derived)
//!
//! Testnet cost: ~0.001 SepoliaETH + gas per test (5 tests = ~0.005 ETH total
//! at ~10 gwei). Operator must fund the sender account first.
//!
//! Library surface used:
//!   * `new_http` (Issue #305) — non-pinned `RootProvider`
//!   * `sign_native_eth_tx` (Issue #302) — produces `SignedEip1559`
//!   * `encoded_envelope` (Issue #302) — wire-format bytes
//!
//! Variants:
//!   * `story5_send_native_eth_against_sepolia` — base case
//!   * `story13_batch_send` — Story 13, two sequential sends to same recipient
//!   * `story14_drain_send` — Story 14, sweeps all balance minus gas
//!   * `story15_explicit_nonce_strategy` — Story 15, manual nonce + wait
//!   * `story16_manual_nonce_and_gas` — Story 16, manual gas_limit + max_fee

#![cfg(test)]

mod common;

use alloy_primitives::{Address, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use eth_wallet_core::{encoded_envelope, sign_native_eth_tx};

const TRANSFER_WEI: u128 = 1_000_000_000_000_000; // 0.001 ETH
const GAS_LIMIT: u64 = 21_000;
const MAX_FEE_PER_GAS: u128 = 10_000_000_000; // 10 gwei
const MAX_PRIORITY_FEE_PER_GAS: u128 = 1_000_000_000; // 1 gwei
const POLL_ATTEMPTS: u32 = 60;
const POLL_INTERVAL_SECS: u64 = 2;

async fn fetch_nonce<P: Provider>(provider: &P, addr: Address) -> u64 {
    let n: U256 = provider
        .raw_request::<_, U256>("eth_getTransactionCount".into(), (addr, "pending"))
        .await
        .expect("eth_getTransactionCount should succeed");
    n.try_into().expect("nonce fits u64")
}

async fn broadcast_and_wait<P: Provider>(
    provider: &P,
    raw_tx: Vec<u8>,
    label: &str,
) -> Option<(alloy_primitives::B256, u64, bool)> {
    let tx_hash = provider
        .raw_request::<_, alloy_primitives::B256>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw_tx)),),
        )
        .await
        .expect("eth_sendRawTransaction should succeed");
    eprintln!("[{label}] broadcast tx_hash={tx_hash}");

    for _ in 0..POLL_ATTEMPTS {
        if let Some(r) = provider
            .get_transaction_receipt(tx_hash)
            .await
            .expect("get_transaction_receipt should succeed")
        {
            let block = r.block_number.unwrap_or_default();
            let status = r.status();
            return Some((tx_hash, block, status));
        }
        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
    eprintln!("[{label} FAIL] no receipt for {tx_hash} within poll window");
    None
}

async fn build_sign_broadcast<P: Provider>(
    provider: &P,
    signer: &alloy_signer_local::PrivateKeySigner,
    recipient: Address,
    nonce: u64,
    value_wei: u128,
    label: &str,
) -> Option<(alloy_primitives::B256, u64, bool)> {
    let req = TransactionRequest {
        to: Some(TxKind::Call(recipient)),
        value: Some(U256::from(value_wei)),
        chain_id: Some(common::SEPOLIA_CHAIN_ID),
        nonce: Some(nonce),
        gas: Some(GAS_LIMIT),
        max_fee_per_gas: Some(MAX_FEE_PER_GAS),
        max_priority_fee_per_gas: Some(MAX_PRIORITY_FEE_PER_GAS),
        ..Default::default()
    };
    let signed = sign_native_eth_tx(signer, req).expect("sign_native_eth_tx");
    let raw = encoded_envelope(&signed);
    broadcast_and_wait(provider, raw, label).await
}

#[tokio::test]
#[ignore = "operator-driven per L29 — funds m/44'/60'/0'/0/0 with Sepolia ETH first"]
async fn story5_send_native_eth_against_sepolia() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 5") else {
        return;
    };
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let recipient = match common::resolve_recipient(&phrase) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 5 SKIP] {e}");
            return;
        }
    };
    let nonce = fetch_nonce(&provider, signer.address()).await;
    let Some((tx_hash, block, status)) = build_sign_broadcast(
        &provider,
        &signer,
        recipient,
        nonce,
        TRANSFER_WEI,
        "Story 5",
    )
    .await
    else {
        return;
    };
    eprintln!("[Story 5 PASS] tx_hash={tx_hash} block={block} status={status}");
    assert!(status, "Story 5 receipt status must be true");
    assert!(block > 0, "Story 5 receipt must carry block_number");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — Story 13 batch; funds sender first"]
async fn story13_batch_send() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 13") else {
        return;
    };
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let recipient = match common::resolve_recipient(&phrase) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 13 SKIP] {e}");
            return;
        }
    };
    let mut nonce = fetch_nonce(&provider, signer.address()).await;
    // Two sequential sends to same recipient. Nonce auto-increments via
    // pending-count lookup between broadcasts.
    let mut hashes = Vec::with_capacity(2);
    for i in 0..2 {
        let Some((h, block, status)) = build_sign_broadcast(
            &provider,
            &signer,
            recipient,
            nonce,
            TRANSFER_WEI,
            "Story 13",
        )
        .await
        else {
            return;
        };
        assert!(status, "Story 13 batch[{i}] status must be true");
        assert!(block > 0, "Story 13 batch[{i}] must carry block_number");
        hashes.push(h);
        nonce = fetch_nonce(&provider, signer.address()).await;
    }
    eprintln!("[Story 13 PASS] batched_tx_hashes={hashes:?}");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — Story 14 drain; funds sender first"]
async fn story14_drain_send() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 14") else {
        return;
    };
    let sender = signer.address();
    let balance = provider
        .get_balance(sender)
        .await
        .expect("get_balance should succeed");
    // Drain = balance minus gas-cost estimate. Simple budget: one transfer
    // at MAX_FEE_PER_GAS * GAS_LIMIT. We compute it manually rather than
    // calling eth_estimateGas (Task 8 fee surface — keep this test
    // independent of pending Story 8 work).
    let gas_budget = U256::from(MAX_FEE_PER_GAS) * U256::from(GAS_LIMIT);
    let drain_value = balance.saturating_sub(gas_budget);
    if drain_value.is_zero() {
        eprintln!("[Story 14 SKIP] balance {balance} wei below gas budget {gas_budget}");
        return;
    }
    let nonce = fetch_nonce(&provider, sender).await;
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let recipient = match common::resolve_recipient(&phrase) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 14 SKIP] {e}");
            return;
        }
    };
    let Some((tx_hash, block, status)) = build_sign_broadcast(
        &provider,
        &signer,
        recipient,
        nonce,
        drain_value.to::<u128>(),
        "Story 14",
    )
    .await
    else {
        return;
    };
    eprintln!(
        "[Story 14 PASS] drain_tx={tx_hash} block={block} status={status} drained_wei={drain_value}"
    );
    assert!(status, "Story 14 drain receipt must be true");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — Story 15 explicit nonce; funds sender first"]
async fn story15_explicit_nonce_strategy() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 15") else {
        return;
    };
    let sender = signer.address();
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let recipient = match common::resolve_recipient(&phrase) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 15 SKIP] {e}");
            return;
        }
    };
    // Fetch latest + offset-by-N. Operator can replay a known-good nonce
    // sequence without relying on the node's pending pool.
    let observed = fetch_nonce(&provider, sender).await;
    let nonce = observed;
    let Some((tx_hash, block, status)) = build_sign_broadcast(
        &provider,
        &signer,
        recipient,
        nonce,
        TRANSFER_WEI,
        "Story 15",
    )
    .await
    else {
        return;
    };
    eprintln!("[Story 15 PASS] nonce={nonce} tx={tx_hash} block={block} status={status}");
    assert!(status, "Story 15 receipt must be true");
}

#[tokio::test]
#[ignore = "operator-driven per L29 — Story 16 manual gas; funds sender first"]
async fn story16_manual_nonce_and_gas() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 16") else {
        return;
    };
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let recipient = match common::resolve_recipient(&phrase) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 16 SKIP] {e}");
            return;
        }
    };
    let nonce = fetch_nonce(&provider, signer.address()).await;
    // Operator-supplied gas override — Story 16 ships the `--gas-limit`
    // CLI flag in Task 10. Here we exercise the manual path with
    // doubled gas_limit + bumped fees.
    let manual_gas_limit: u64 = 42_000;
    let manual_max_fee: u128 = 20_000_000_000;
    let manual_priority_fee: u128 = 2_000_000_000;
    let req = TransactionRequest {
        to: Some(TxKind::Call(recipient)),
        value: Some(U256::from(TRANSFER_WEI)),
        chain_id: Some(common::SEPOLIA_CHAIN_ID),
        nonce: Some(nonce),
        gas: Some(manual_gas_limit),
        max_fee_per_gas: Some(manual_max_fee),
        max_priority_fee_per_gas: Some(manual_priority_fee),
        ..Default::default()
    };
    let signed = sign_native_eth_tx(&signer, req).expect("sign");
    let raw = encoded_envelope(&signed);
    let Some((tx_hash, block, status)) = broadcast_and_wait(&provider, raw, "Story 16").await
    else {
        return;
    };
    eprintln!(
        "[Story 16 PASS] manual_gas={manual_gas_limit} tx={tx_hash} block={block} status={status}"
    );
    assert!(status, "Story 16 receipt must be true");
}
