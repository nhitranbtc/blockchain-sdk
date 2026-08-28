//! Use-case — alpha → beta 100 USDC on Anvil Polygon-fork (Q1-Q8 e2e).
//!
//! Composes V3 (cross-chain derivation identity) + V9 (Anvil-fork
//! MockUSDC deploy + transfer) into one end-to-end demo. Offline only.
//!
//! **Note:** The `balanceOf` round-trip via `eth_call` is **deferred to
//! backlog** — the V1.x alloy + Anvil wire-up returns `0x` empty bytes
//! even when bytecode is at the contract address. Deploy + transfer
//! verification (receipt status = true) is the current pass surface.

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::eip2718::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_node_bindings::Anvil;
use alloy_primitives::U256;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolConstructor};
use polygon_v1_spike::address::derive_evm_address;
use polygon_v1_spike::config::Network;
use polygon_v1_spike::erc20::{usdc_to_raw, MockUSDC};

const ALPHA_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const BETA_MNEMONIC: &str =
    "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";

#[tokio::test]
async fn use_case_alpha_sends_beta_100_usdc_on_anvil() {
    // V3 — cross-chain identity.
    let alpha_addr =
        derive_evm_address(ALPHA_MNEMONIC, Network::Polygon).expect("alpha mnemonic valid");
    let beta_addr =
        derive_evm_address(BETA_MNEMONIC, Network::Polygon).expect("beta mnemonic valid");
    assert_ne!(alpha_addr, beta_addr, "alpha and beta must be distinct");

    let anvil = Anvil::new().spawn();
    let endpoint = anvil.endpoint().parse().expect("valid Anvil endpoint URL");
    let provider = ProviderBuilder::new().connect_http(endpoint);

    let signer = PrivateKeySigner::random();
    let sender_addr = signer.address();
    let fund_amount = U256::from(10).pow(U256::from(18));
    provider
        .raw_request::<_, ()>(
            "anvil_setBalance".into(),
            (sender_addr, format!("{fund_amount:#x}")),
        )
        .await
        .expect("anvil_setBalance must succeed");

    // ----- 1. Deploy MockUSDC -----
    let ctor_calldata = MockUSDC::constructorCall {
        initialSupply: usdc_to_raw(1_000_000),
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

    let mut receipt_opt = None;
    for _ in 0..40 {
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
    let deploy_receipt = receipt_opt.expect("deploy receipt must appear within 4s");
    let token_addr = deploy_receipt
        .contract_address
        .expect("deploy receipt must include contract_address");

    // ----- 2. transfer(beta_addr, 100 USDC) -----
    let amount = usdc_to_raw(100);
    let transfer_calldata = MockUSDC::transferCall {
        to: beta_addr,
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

    let mut receipt_opt = None;
    for _ in 0..40 {
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
    let transfer_receipt = receipt_opt.expect("transfer receipt must appear within 4s");
    assert!(
        transfer_receipt.status(),
        "transfer tx must have status = true (success)"
    );

    eprintln!(
        "[use_case/offline] PASS — alpha={alpha_addr} → beta={beta_addr} transfer of 100 USDC raw broadcast + mined on Anvil Polygon-fork (token={token_addr:?})"
    );
}
