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
use alloy_sol_types::{SolCall, SolValue};
use polygon_v1_spike::address::build_signer;
use polygon_v1_spike::config::Network;
use polygon_v1_spike::erc20::{usdc_to_raw, MockUSDC};

const ALPHA_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const BETA_MNEMONIC: &str =
    "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";

mod common;

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_ANVIL_E2E=1 cargo test --test use_case_alpha_sends_beta_100_usdc -- --ignored"]
async fn use_case_alpha_sends_beta_100_usdc_on_anvil() {
    if !common::env_opt_in("RUN_POLYGON_ANVIL_E2E") {
        eprintln!("[use_case] SKIP — set RUN_POLYGON_ANVIL_E2E=1 to enable Anvil Polygon-fork e2e");
        return;
    }
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
    // Issue #419 fix: prepend the compiled runtime bytecode (from the
    // `#[sol(bytecode = "0x...")]` attribute on `MockUSDC`) to the
    // constructor calldata. Without the bytecode, Anvil receives only the
    // constructor-args payload and deploys an empty-code "contract" that
    // returns `0x` from any eth_call.
    let initial_supply = usdc_to_raw(1_000_000);
    let ctor_args = initial_supply.abi_encode();
    let mut deploy_input: Vec<u8> = MockUSDC::BYTECODE.to_vec();
    deploy_input.extend_from_slice(&ctor_args);
    let deploy_input: alloy_primitives::Bytes = deploy_input.into();
    let mut deploy_tx = TxEip1559 {
        chain_id: anvil.chain_id(),
        nonce: 0,
        gas_limit: 3_000_000,
        max_fee_per_gas: 1_000_000_000,
        max_priority_fee_per_gas: 1_000_000_000,
        to: alloy_primitives::TxKind::Create,
        value: U256::ZERO,
        input: deploy_input,
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

    // ----- 3. balanceOf round-trip — both sides (Issue #419 acceptance) -----
    use alloy_rpc_types::TransactionRequest;

    // beta (recipient) — should hold the transferred amount.
    let beta_calldata = MockUSDC::balanceOfCall { account: beta_addr }.abi_encode();
    let beta_req = TransactionRequest::default()
        .to(token_addr)
        .input(beta_calldata.into());
    let beta_bytes = provider
        .call(beta_req)
        .await
        .expect("eth_call balanceOf(beta) must succeed");
    assert_eq!(
        beta_bytes.len(),
        32,
        "balanceOf(beta) must return 32 bytes (got {} bytes = 0x{} = no bytecode — Issue #419)",
        beta_bytes.len(),
        hex::encode(&beta_bytes)
    );
    let beta_balance = U256::from_be_slice(&beta_bytes);
    assert_eq!(
        beta_balance, amount,
        "balanceOf(beta) should equal transfer amount (100 USDC raw)"
    );

    // alpha (deployer/sender) — should hold initialSupply - amount.
    let alpha_expected = usdc_to_raw(1_000_000) - amount;
    let alpha_calldata = MockUSDC::balanceOfCall {
        account: alpha_addr,
    }
    .abi_encode();
    let alpha_req = TransactionRequest::default()
        .to(token_addr)
        .input(alpha_calldata.into());
    let alpha_bytes = provider
        .call(alpha_req)
        .await
        .expect("eth_call balanceOf(alpha) must succeed");
    assert_eq!(
        alpha_bytes.len(),
        32,
        "balanceOf(alpha) must return 32 bytes (got {} bytes)",
        alpha_bytes.len()
    );
    let alpha_balance = U256::from_be_slice(&alpha_bytes);
    assert_eq!(
        alpha_balance, alpha_expected,
        "balanceOf(alpha) should equal initialSupply - transfer amount (1M - 100 USDC raw)"
    );

    eprintln!(
        "[use_case/offline] PASS — alpha={alpha_addr} → beta={beta_addr} transfer of 100 USDC raw mined (token={token_addr:?}); balanceOf(beta) = {beta_balance}, balanceOf(alpha) = {alpha_balance}"
    );
}
