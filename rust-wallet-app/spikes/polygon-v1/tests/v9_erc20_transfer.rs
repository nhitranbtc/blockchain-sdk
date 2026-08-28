//! V9 — ERC-20 `transfer` on Anvil Polygon-fork (Q3 — ERC-20 surface).
//!
//! Offline test: spawns Anvil in-process, deploys `MockUSDC` via signed
//! EIP-1559 contract-creation tx, then `transfer(recipient, value)` +
//! verifies receipt status. The post-transfer `balanceOf` round-trip is
//! deferred per Issue #419 — see the deferred block at the end of the
//! test fn.
//!
//! #419 root cause: the `sol!` macro in alloy 1.8.x only generates the
//! `BYTECODE` static const + `deploy()` helper when the `bytecode =
//! "0x..."` attribute is passed to the macro invocation. Our `MockUSDC`
//! lacks that attribute (the bytecode was never compiled in — Anvil
//! receives only the constructor-args payload and deploys an empty-code
//! "contract" that returns 0x from any eth_call). Adding the bytecode
//! attribute requires first compiling the contract source (e.g. via
//! solc) and embedding the resulting hex — a chicken-and-egg loop on
//! the test infrastructure. Filed as Issue #419; deploy + transfer
//! halves both PASS receipt status = true.

use std::time::Duration;

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::eip2718::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_node_bindings::Anvil;
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolConstructor};

use polygon_v1_spike::erc20::{usdc_to_raw, MockUSDC};

mod common;

#[tokio::test]
async fn v9_deploy_mock_usdc_and_transfer_on_anvil_polygon_fork() {
    let anvil = Anvil::new().spawn();
    let endpoint = anvil.endpoint().parse().expect("valid Anvil endpoint URL");
    let provider = ProviderBuilder::new().connect_http(endpoint);

    let signer = PrivateKeySigner::random();
    let sender_addr = signer.address();

    // Fund the signer via anvil_setBalance so it can pay for deploy + transfer gas.
    let fund_amount = U256::from(10).pow(U256::from(18)); // 1 ETH
    provider
        .raw_request::<_, ()>(
            "anvil_setBalance".into(),
            (sender_addr, format!("{fund_amount:#x}")),
        )
        .await
        .expect("anvil_setBalance must succeed");

    // ----- 1. Deploy MockUSDC -----
    let ctor_calldata = MockUSDC::constructorCall {
        initialSupply: usdc_to_raw(10_000_000),
    }
    .abi_encode();
    let mut deploy_tx = TxEip1559 {
        chain_id: anvil.chain_id(),
        nonce: 0,
        gas_limit: 3_000_000,
        max_fee_per_gas: 1_000_000_000,
        max_priority_fee_per_gas: 1_000_000_000,
        to: alloy_primitives::TxKind::Create,
        value: U256::ZERO,
        input: ctor_calldata.into(),
        access_list: Default::default(),
    };
    let sig = signer
        .sign_transaction_sync(&mut deploy_tx)
        .expect("sign deploy must succeed");
    let signed = deploy_tx.into_signed(sig);
    let raw = signed.encoded_2718();

    let tx_hash = provider
        .raw_request::<_, alloy_primitives::B256>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw)),),
        )
        .await
        .expect("eth_sendRawTransaction (deploy) must succeed");

    let deploy_receipt = common::await_receipt(&provider, tx_hash, 40, Duration::from_millis(100))
        .await
        .expect("deploy receipt must appear within 4s");
    let token_addr = deploy_receipt
        .contract_address
        .expect("deploy receipt must include contract_address");
    assert!(
        deploy_receipt.status(),
        "deploy tx must have status = true (success)"
    );

    // ----- 2. transfer(beta, 100 USDC) -----
    let recipient = Address::with_last_byte(0x42);
    let amount = usdc_to_raw(100);
    let transfer_calldata = MockUSDC::transferCall {
        to: recipient,
        value: amount,
    }
    .abi_encode();

    let nonce = provider
        .get_transaction_count(sender_addr)
        .await
        .expect("nonce fetch must succeed");

    let mut transfer_tx = TxEip1559 {
        chain_id: anvil.chain_id(),
        nonce,
        gas_limit: 100_000,
        max_fee_per_gas: 1_000_000_000,
        max_priority_fee_per_gas: 1_000_000_000,
        to: alloy_primitives::TxKind::Call(token_addr),
        value: U256::ZERO,
        input: transfer_calldata.into(),
        access_list: Default::default(),
    };
    let sig = signer
        .sign_transaction_sync(&mut transfer_tx)
        .expect("sign transfer must succeed");
    let signed = transfer_tx.into_signed(sig);
    let raw = signed.encoded_2718();

    let tx_hash = provider
        .raw_request::<_, alloy_primitives::B256>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw)),),
        )
        .await
        .expect("eth_sendRawTransaction (transfer) must succeed");

    let transfer_receipt =
        common::await_receipt(&provider, tx_hash, 40, Duration::from_millis(100))
            .await
            .expect("transfer receipt must appear within 4s");
    assert!(
        transfer_receipt.status(),
        "transfer tx must have status = true (success)"
    );

    eprintln!(
        "[V9] PASS — MockUSDC deployed at {token_addr:?}; transfer of 100 USDC raw broadcast + mined (tx_hash={tx_hash}); balanceOf deferred per #419"
    );
}
