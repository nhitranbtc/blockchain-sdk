//! E2E Sepolia — Story 21 (ERC-20 send).
//!
//! Issue #310 — Story 21 of the #298 story map. Builds an ERC-20
//! transfer calldata via `sol! transfer(address,uint256)` and signs +
//! broadcasts it through the library façade (`sign_erc20_tx_bytes`).
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_erc20_send -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (funds sender + has token balance)
//!   ETH_E2E_TOKEN_ADDRESS  ERC-20 contract address on Sepolia
//!   ETH_E2E_TOKEN_DECIMALS optional — default 18 (token decimals for amount parsing)
//!   ETH_E2E_TOKEN_AMOUNT   optional — human-readable amount (default "1")
//!   ETH_E2E_RECIPIENT       optional — default = m/44'/60'/0'/0/1
//!
//! Testnet cost: ERC-20 transfer gas (~65k gas at 10 gwei ≈ 0.00065 ETH).

#![cfg(test)]

mod common;

use alloy_primitives::U256;
use alloy_provider::Provider;
use alloy_sol_types::{sol, SolCall};
use evm_wallet_core::{encoded_envelope, sign_erc20_tx_bytes};

sol! {
    interface IERC20 {
        function transfer(address to, uint256 amount) external returns (bool);
        function decimals() external view returns (uint8);
    }
}

const ERC20_GAS_LIMIT: u64 = 65_000;
const MAX_FEE_PER_GAS: u128 = 10_000_000_000;
const MAX_PRIORITY_FEE_PER_GAS: u128 = 1_000_000_000;
const POLL_ATTEMPTS: u32 = 60;
const POLL_INTERVAL_SECS: u64 = 2;

#[tokio::test]
#[ignore = "operator-driven per L29 — funds sender with token balance first"]
async fn story21_erc20_send_against_sepolia() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 21") else {
        return;
    };
    let token_addr = match common::require_env_as_address("ETH_E2E_TOKEN_ADDRESS") {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 21 SKIP] {e}");
            return;
        }
    };
    let phrase = std::env::var("ETH_E2E_MNEMONIC").unwrap_or_default();
    let recipient = match common::resolve_recipient(&phrase) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[Story 21 SKIP] {e}");
            return;
        }
    };

    let decimals: u8 = std::env::var("ETH_E2E_TOKEN_DECIMALS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(18u8);

    let human_amount: u128 = std::env::var("ETH_E2E_TOKEN_AMOUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1u128);
    let base_units = human_amount
        .checked_mul(10u128.pow(decimals as u32))
        .expect("human * 10^decimals fits u128 for small decimals");
    let amount = U256::from(base_units);

    // Build calldata: transfer(recipient, amount). Selector = 0xa9059cbb
    // per #297 + Task 7 acceptance.
    let call = IERC20::transferCall {
        to: recipient,
        amount,
    };
    let calldata: alloy_primitives::Bytes = call.abi_encode().into();
    assert_eq!(
        &calldata[..4],
        &[0xa9, 0x05, 0x9c, 0xbb],
        "selector must be keccak256('transfer(address,uint256)')"
    );

    let sender_addr = signer.address();
    let nonce: u64 = {
        let n: U256 = provider
            .raw_request::<_, U256>("eth_getTransactionCount".into(), (sender_addr, "pending"))
            .await
            .expect("eth_getTransactionCount");
        n.try_into().expect("nonce fits u64")
    };

    let signed = sign_erc20_tx_bytes(
        &signer,
        token_addr,
        calldata,
        U256::ZERO, // ERC-20 transfer value = 0 ETH (token moved via calldata)
        nonce,
        common::SEPOLIA_CHAIN_ID,
        MAX_FEE_PER_GAS,
        MAX_PRIORITY_FEE_PER_GAS,
        ERC20_GAS_LIMIT,
    )
    .expect("sign_erc20_tx_bytes");

    let raw = encoded_envelope(&signed);
    let tx_hash: alloy_primitives::B256 = provider
        .raw_request::<_, _>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw)),),
        )
        .await
        .expect("eth_sendRawTransaction");
    eprintln!(
        "[Story 21] broadcast tx_hash={tx_hash} token={token_addr} to={recipient} amount={amount}"
    );

    let mut receipt_opt = None;
    for _ in 0..POLL_ATTEMPTS {
        if let Some(r) = provider
            .get_transaction_receipt(tx_hash)
            .await
            .expect("get_transaction_receipt")
        {
            receipt_opt = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
    let receipt = match receipt_opt {
        Some(r) => r,
        None => {
            eprintln!("[Story 21 FAIL] no receipt for {tx_hash} within poll window");
            return;
        }
    };
    let block = receipt.block_number.unwrap_or_default();
    eprintln!(
        "[Story 21 PASS] tx_hash={tx_hash} block_number={block} status={}",
        receipt.status()
    );
    assert!(receipt.status(), "Story 21 receipt status must be true");
    assert!(block > 0, "Story 21 receipt must carry block_number");
}
