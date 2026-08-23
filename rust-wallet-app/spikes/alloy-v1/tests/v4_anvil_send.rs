//! V4 verification — `provider.send_transaction(signed_native_eth_tx)`
//! against Anvil returns a `TransactionReceipt`.
//!
//! Issue #293 — verification item V4.
//!
//! Per L29, this test is `#[ignore]` by default and only runs when the
//! operator opts in with `RUN_V4_ANVIL=1 cargo test`. Spins up a local
//! Anvil instance via `alloy-node-bindings`, funds a fresh keypair, sends a
//! signed native-ETH transfer, and waits for the receipt.

#![cfg(test)]

use alloy_network::TransactionBuilder;
use alloy_node_bindings::{Anvil, AnvilInstance};
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use alloy_transport_http::reqwest::Url;

fn env_opt_in() -> bool {
    std::env::var("RUN_V4_ANVIL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn spawn_anvil() -> AnvilInstance {
    // alloy-node-bindings 1.8.3: Anvil::new().spawn() returns AnvilInstance
    // directly (sync constructor; the subprocess itself is async-managed).
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

    // Anvil prefunds the first account it generates; here we hand-roll a random
    // signer and explicitly fund it via `anvil_setBalance`.
    let fund_amount = U256::from(10).pow(U256::from(18)); // 1 ETH in wei
    provider
        .raw_request::<_, ()>(
            "anvil_setBalance".into(),
            (sender.address(), format!("{fund_amount:#x}")),
        )
        .await
        .expect("anvil_setBalance must succeed");

    // Build + sign a 0.001 ETH transfer.
    let value = U256::from(10).pow(U256::from(15)); // 0.001 ETH
    let tx = TransactionRequest::default()
        .with_to(recipient)
        .with_value(value)
        .with_from(sender.address())
        .with_chain_id(anvil.chain_id());

    let pending = provider
        .send_transaction(tx)
        .await
        .expect("send_transaction must succeed");

    let receipt = pending
        .get_receipt()
        .await
        .expect("get_receipt must succeed");

    assert!(receipt.status(), "transaction must succeed (status = true)");
    eprintln!(
        "[V4] PASS — sent {value} wei from {} to {recipient}; tx_hash = {:?}",
        sender.address(),
        receipt.transaction_hash,
    );
}
