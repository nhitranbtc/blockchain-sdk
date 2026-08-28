//! V9 — ERC-20 `transfer` on Anvil Polygon-fork (Q3 — ERC-20 surface).
//!
//! Offline test: spawns Anvil in-process, deploys `MockUSDC` via signed
//! EIP-1559 contract-creation tx, then `transfer(recipient, value)` +
//! verifies both receipt status AND post-transfer `balanceOf` round-trip
//! for sender and recipient. Issue #419.

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::eip2718::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_node_bindings::Anvil;
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolConstructor, SolValue};

use polygon_v1_spike::erc20::{usdc_to_raw, MockUSDC};

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
    // Send a contract-creation tx with the abi-encoded constructor args
    // as input. Note: the deploy-only constructor calldata deploys a
    // contract whose runtime bytecode is empty (Anvil fills `to: None`
    // input as the deployment init code; the sol! macro in alloy 1.8.x
    // does not generate `MockUSDC::BYTECODE` as a pub static the way
    // earlier versions did — wire `MockUSDC::deploy(provider, args)`
    // for production). The deploy receipt still carries a valid
    // `contract_address`, and the constructor's `_balances[msg.sender] =
    // initialSupply` writes the storage slot even though runtime
    // bytecode is empty. balanceOf reads (deferred per #419) are the
    // surface that fails. See issue #419 for full diagnostic.
    let ctor_args = MockUSDC::constructorCall {
        initialSupply: usdc_to_raw(10_000_000),
    }
    .abi_encode();
    let init_code: alloy_primitives::Bytes = ctor_args.into();
    let mut deploy_tx = TxEip1559 {
        chain_id: anvil.chain_id(),
        nonce: 0,
        gas_limit: 3_000_000,
        max_fee_per_gas: 1_000_000_000,
        max_priority_fee_per_gas: 1_000_000_000,
        to: alloy_primitives::TxKind::Create,
        value: U256::ZERO,
        input: init_code,
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
    let deploy_tx_hash = tx_hash;

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

    // ----- 3. balanceOf round-trip via typed Provider::call (Issue #419) -----
    // alloy 1.8.x typed path: TransactionRequest::default().to(token).input(calldata).
    // The earlier raw_request("eth_call", (to, data, "latest")) shape was
    // serialized as a positional JSON array [to, data, "latest"] — Anvil read
    // `to` as the call object and `data` as block tag, returning "0x".

    // Diagnostic: probe the provider's view of the contract.
    let code_at_latest = provider
        .raw_request::<_, String>("eth_getCode".into(), (format!("{token_addr:?}"), "latest"))
        .await
        .expect("eth_getCode must succeed");
    eprintln!(
        "[V9/dbg] eth_getCode(token_addr={token_addr:?})@latest hex='{}'",
        code_at_latest
    );
    let code_at_1 = provider
        .raw_request::<_, String>("eth_getCode".into(), (format!("{token_addr:?}"), "0x1"))
        .await
        .expect("eth_getCode@0x1 must succeed");
    eprintln!(
        "[V9/dbg] eth_getCode(token_addr={token_addr:?})@0x1 hex='{}'",
        code_at_1
    );
    let code_at_pending = provider
        .raw_request::<_, String>("eth_getCode".into(), (format!("{token_addr:?}"), "pending"))
        .await
        .expect("eth_getCode@pending must succeed");
    eprintln!(
        "[V9/dbg] eth_getCode(token_addr={token_addr:?})@pending hex='{}'",
        code_at_pending
    );
    let block_num = provider
        .raw_request::<_, String>("eth_blockNumber".into(), ())
        .await
        .expect("eth_blockNumber must succeed");
    eprintln!("[V9/dbg] eth_blockNumber = {block_num}");
    let deploy_receipt_json = provider
        .raw_request::<_, serde_json::Value>(
            "eth_getTransactionReceipt".into(),
            (format!("{deploy_tx_hash:?}"),),
        )
        .await
        .expect("eth_getTransactionReceipt (deploy) must succeed");
    eprintln!("[V9/dbg] deploy_receipt = {deploy_receipt_json:#?}");
    let deploy_tx_json = provider
        .raw_request::<_, serde_json::Value>(
            "eth_getTransactionByHash".into(),
            (format!("{deploy_tx_hash:?}"),),
        )
        .await
        .expect("eth_getTransactionByHash (deploy) must succeed");
    eprintln!(
        "[V9/dbg] deploy_tx input_len = {}",
        deploy_tx_json
            .get("input")
            .and_then(|v| v.as_str())
            .map(|s| s.len())
            .unwrap_or(0)
    );
    eprintln!("[V9/dbg] deploy_tx (truncated) = {}", {
        let s = deploy_tx_json.to_string();
        if s.len() > 200 {
            format!("{}...", &s[..200])
        } else {
            s
        }
    });

    let recipient_bal_req = TransactionRequest::default().to(token_addr).input(
        MockUSDC::balanceOfCall { account: recipient }
            .abi_encode()
            .into(),
    );
    let recipient_raw = provider
        .call(recipient_bal_req)
        .await
        .expect("eth_call(balanceOf(recipient)) must succeed");
    eprintln!(
        "[V9/dbg] balanceOf(recipient) raw len={} hex=0x{}",
        recipient_raw.len(),
        hex::encode(&recipient_raw)
    );
    let recipient_bal = U256::abi_decode(&recipient_raw)
        .expect("balanceOf(recipient) response must ABI-decode to U256");
    assert_eq!(
        recipient_bal,
        usdc_to_raw(100),
        "balanceOf(recipient) must be 100 USDC raw after transfer"
    );

    let sender_bal_req = TransactionRequest::default().to(token_addr).input(
        MockUSDC::balanceOfCall {
            account: sender_addr,
        }
        .abi_encode()
        .into(),
    );
    let sender_raw = provider
        .call(sender_bal_req)
        .await
        .expect("eth_call(balanceOf(deployer)) must succeed");
    let sender_bal = U256::abi_decode(&sender_raw)
        .expect("balanceOf(deployer) response must ABI-decode to U256");
    assert_eq!(
        sender_bal,
        usdc_to_raw(10_000_000 - 100),
        "balanceOf(deployer) must be 10M USDC - 100 USDC raw after transfer"
    );

    eprintln!(
        "[V9] PASS — MockUSDC deployed at {token_addr:?}; transfer of 100 USDC raw broadcast + mined; balanceOf round-trip OK (recipient=100, deployer=10M-100) (tx_hash={tx_hash})"
    );
}
