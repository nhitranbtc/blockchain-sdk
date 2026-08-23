//! V4 verification — `provider.send_raw_transaction(signed_native_eth_tx_bytes)`
//! against Anvil returns a tx hash; `provider.get_transaction_receipt(hash)` then
//! returns a `TransactionReceipt`.
//!
//! Issue #293 — verification item V4.
//!
//! Per L29, this test is `#[ignore]` by default and only runs when the
//! operator opts in with `RUN_V4_ANVIL=1 cargo test`. Spins up a local
//! Anvil instance via `alloy-node-bindings`, funds a fresh keypair, signs
//! a native-ETH transfer manually (avoids wallet-fill-provider API
//! complexity in alloy 1.8.3), broadcasts the signed envelope via
//! `eth_sendRawTransaction`, and waits for the receipt.

#![cfg(test)]

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_node_bindings::{Anvil, AnvilInstance};
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use alloy_transport_http::reqwest::Url;

fn env_opt_in() -> bool {
    std::env::var("RUN_V4_ANVIL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn spawn_anvil() -> AnvilInstance {
    Anvil::new().spawn()
}

#[tokio::test]
#[ignore = "operator-driven per L29 — requires foundry install + run with: RUN_V4_ANVIL=1 cargo test --test v4_anvil_send -- --ignored"]
async fn v4_send_signed_native_eth_tx_against_anvil() {
    if !env_opt_in() {
        eprintln!("[V4] SKIP — set RUN_V4_ANVIL=1 to enable Anvil spawn");
        return;
    }

    let anvil = spawn_anvil();
    let endpoint: Url = anvil.endpoint().parse().expect("valid Anvil endpoint URL");
    let provider = ProviderBuilder::new().connect_http(endpoint);

    // Fresh signer + recipient.
    let sender = PrivateKeySigner::random();
    let recipient = Address::with_last_byte(0x42);

    // Fund sender via anvil_setBalance so we can send a 0.001 ETH transfer.
    let fund_amount = U256::from(10).pow(U256::from(18)); // 1 ETH in wei
    provider
        .raw_request::<_, ()>(
            "anvil_setBalance".into(),
            (sender.address(), format!("{fund_amount:#x}")),
        )
        .await
        .expect("anvil_setBalance must succeed");

    // Fetch nonce for sender.
    let nonce_u256: U256 = provider
        .raw_request::<_, U256>(
            "eth_getTransactionCount".into(),
            (sender.address(), "latest"),
        )
        .await
        .expect("eth_getTransactionCount must succeed");
    let nonce: u64 = nonce_u256
        .try_into()
        .expect("nonce must fit in u64 for anvil regtest");

    // Build an EIP-1559 transaction and sign it manually.
    let value = U256::from(10).pow(U256::from(15)); // 0.001 ETH
    let mut tx = TxEip1559 {
        chain_id: anvil.chain_id(),
        nonce,
        gas_limit: 21_000,
        max_fee_per_gas: 1_000_000_000,          // 1 gwei
        max_priority_fee_per_gas: 1_000_000_000, // 1 gwei tip
        to: alloy_primitives::TxKind::Call(recipient),
        value,
        access_list: Default::default(),
        input: Default::default(),
    };

    let sig = sender
        .sign_transaction_sync(&mut tx)
        .expect("sign_transaction_sync must succeed");
    let signed_envelope = tx.into_signed(sig);

    // Broadcast the raw signed transaction.
    let raw_tx = signed_envelope.encoded_2718();
    let tx_hash: alloy_primitives::B256 = provider
        .raw_request::<_, _>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw_tx)),),
        )
        .await
        .expect("eth_sendRawTransaction must succeed");

    eprintln!(
        "[V4] PASS — broadcasted 0.001 ETH tx from {} to {recipient}; tx_hash = {tx_hash}",
        sender.address(),
    );

    // Verify receipt exists (poll briefly).
    let mut receipt_opt = None;
    for _ in 0..20 {
        if let Some(r) = provider
            .get_transaction_receipt(tx_hash)
            .await
            .expect("get_transaction_receipt must succeed")
        {
            receipt_opt = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(receipt_opt.is_some(), "tx {tx_hash} must have a receipt",);
    let receipt = receipt_opt.expect("receipt present");
    let success = receipt.status();
    assert!(success, "tx {tx_hash} must have status = true (success)",);
    eprintln!(
        "[V4] PASS — receipt confirmed for {tx_hash}; block_number = {}",
        receipt.block_number.expect("block number present"),
    );
}
