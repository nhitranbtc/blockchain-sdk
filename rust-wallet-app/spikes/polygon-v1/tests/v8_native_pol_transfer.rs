//! V8 — native POL transfer (Q5 — POL value transfer + receipt poll).
//!
//! Live test gated on `RUN_POLYGON_AMOY=1`. Sends a native POL value
//! transfer on Amoy. Asserts the receipt poll returns `status = 0x1`.
//!
//! Per L29, operator-driven. Requires a real funded signer — env var
//! `POLYGON_AMOY_PRIVATE_KEY` carries the hex private key (64 chars).

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};

fn env_opt_in() -> bool {
    std::env::var("RUN_POLYGON_AMOY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn load_private_key() -> Option<String> {
    std::env::var("POLYGON_AMOY_PRIVATE_KEY").ok()
}

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 POLYGON_AMOY_PRIVATE_KEY=<64-hex> cargo test --test v8_native_pol_transfer -- --ignored"]
async fn v8_amoy_native_pol_transfer_receipt_status_success() {
    if !env_opt_in() {
        eprintln!("[V8] SKIP — set RUN_POLYGON_AMOY=1 to enable POL transfer");
        return;
    }
    let pk_hex = match load_private_key() {
        Some(p) => p,
        None => {
            eprintln!("[V8] SKIP — set POLYGON_AMOY_PRIVATE_KEY=<64-hex> to send live POL");
            return;
        }
    };

    let pk_bytes: [u8; 32] = hex::decode(pk_hex.trim_start_matches("0x"))
        .expect("POLYGON_AMOY_PRIVATE_KEY must be hex")
        .try_into()
        .expect("POLYGON_AMOY_PRIVATE_KEY must be 32 bytes");
    let pk_b256: alloy_primitives::B256 = pk_bytes.into();
    let sender = PrivateKeySigner::from_bytes(&pk_b256).expect("valid private key");

    let cfg = ChainConfig::for_network(Network::PolygonAmoy);
    let endpoint: Url = cfg.default_rpc_url.parse().expect("valid Amoy RPC URL");
    let provider = ProviderBuilder::new().connect_http(endpoint);

    let recipient = Address::with_last_byte(0x42);

    let nonce_u256: U256 = provider
        .raw_request::<_, U256>(
            "eth_getTransactionCount".into(),
            (sender.address(), "latest"),
        )
        .await
        .expect("eth_getTransactionCount must succeed");
    let nonce: u64 = nonce_u256.try_into().expect("nonce must fit in u64");

    let value = U256::from(10).pow(U256::from(15)); // 0.001 POL

    let mut tx = TxEip1559 {
        chain_id: cfg.chain_id,
        nonce,
        gas_limit: 21_000,
        max_fee_per_gas: 50_000_000_000,
        max_priority_fee_per_gas: 30_000_000_000,
        to: alloy_primitives::TxKind::Call(recipient),
        value,
        access_list: Default::default(),
        input: Default::default(),
    };

    let sig = sender
        .sign_transaction_sync(&mut tx)
        .expect("sign must succeed");
    let signed = tx.into_signed(sig);
    let raw = signed.encoded_2718();

    let tx_hash = provider
        .raw_request::<_, alloy_primitives::B256>(
            "eth_sendRawTransaction".into(),
            (format!("0x{}", hex::encode(&raw)),),
        )
        .await
        .expect("eth_sendRawTransaction must succeed");

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
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    let receipt = receipt_opt.expect("receipt must appear within 20s");

    assert!(
        receipt.status(),
        "POL transfer receipt MUST have status = true"
    );

    eprintln!(
        "[V8] PASS — POL 0.001 transfer {sender_addr} → {recipient}: tx_hash = {tx_hash}",
        sender_addr = sender.address(),
    );
}
