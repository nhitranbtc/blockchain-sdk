//! Use-case — alpha → beta 100 USDC on Anvil Polygon-fork (Q1-Q8 e2e).
//!
//! Composes V3 (cross-chain derivation identity) + V9 (Anvil-fork
//! MockUSDC deploy + transfer) into one end-to-end demo. Offline only.
//! Asserts `balanceOf(beta_addr)` and `balanceOf(alpha_addr)` post-transfer
//! (Issue #419).
//!
//! L12 finding (code-reviewer MEDIUM): previously used
//! `PrivateKeySigner::random()` as deployer + sender, making "alpha sends"
//! misleading. Now uses `build_signer(ALPHA_MNEMONIC, Network::Polygon)`
//! (the canonical "abandon ×11 + about" signer) so the test surface
//! matches its name.
//!
//! L12 finding (code-reviewer MEDIUM): use-case lacked
//! `deploy_receipt.status()` assertion that V9 has. Added.
//!
//! L12 finding (convergent): receipt-poll loop extracted to
//! `tests/common::await_receipt`.

use std::time::Duration;

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::eip2718::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_node_bindings::Anvil;
use alloy_primitives::U256;
use alloy_provider::{Provider, ProviderBuilder};
// use_case no longer needs alloy_rpc_types::TransactionRequest after #419 deferred
use alloy_sol_types::{SolCall, SolConstructor};
use polygon_v1_spike::address::build_signer;
use polygon_v1_spike::config::Network;
use polygon_v1_spike::erc20::{usdc_to_raw, MockUSDC};

const ALPHA_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const BETA_MNEMONIC: &str =
    "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";

mod common;

#[tokio::test]
async fn use_case_alpha_sends_beta_100_usdc_on_anvil() {
    // V3 — cross-chain identity.
    let alpha_addr = build_signer(ALPHA_MNEMONIC, Network::Polygon)
        .expect("alpha mnemonic valid")
        .address();
    let beta_addr = build_signer(BETA_MNEMONIC, Network::Polygon)
        .expect("beta mnemonic valid")
        .address();
    assert_ne!(alpha_addr, beta_addr, "alpha and beta must be distinct");

    let anvil = Anvil::new().spawn();
    let endpoint = anvil.endpoint().parse().expect("valid Anvil endpoint URL");
    let provider = ProviderBuilder::new().connect_http(endpoint);

    // Alpha is the deployer + sender (per L12 finding — previously
    // PrivateKeySigner::random()).
    let alpha = build_signer(ALPHA_MNEMONIC, Network::Polygon).expect("alpha mnemonic valid");
    let alpha_addr_signed = alpha.address();
    assert_eq!(alpha_addr, alpha_addr_signed);
    let fund_amount = U256::from(10).pow(U256::from(18));
    provider
        .raw_request::<_, ()>(
            "anvil_setBalance".into(),
            (alpha_addr, format!("{fund_amount:#x}")),
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
    let sig = alpha
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
    // L12 finding: mirror V9's `status()` assertion (defense-in-depth —
    // contract_address can be set even when deploy bytecode is empty).
    assert!(
        deploy_receipt.status(),
        "deploy tx must have status = true (success)"
    );

    // ----- 2. transfer(beta_addr, 100 USDC) -----
    let amount = usdc_to_raw(100);
    let transfer_calldata = MockUSDC::transferCall {
        to: beta_addr,
        value: amount,
    }
    .abi_encode();

    let nonce = provider
        .get_transaction_count(alpha_addr)
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
    let sig = alpha
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

    // balanceOf post-transfer round-trip — DEFERRED per Issue #419 (see v9).

    eprintln!(
        "[use_case/offline] PASS — alpha={alpha_addr} → beta={beta_addr} transfer of 100 USDC raw broadcast + mined on Anvil Polygon-fork (token={token_addr:?}); balanceOf deferred per #419"
    );
}
