//! V8 — native POL transfer (Q5 — POL value transfer + receipt poll).
//!
//! Live test gated on `RUN_POLYGON_AMOY=1`. Sends a native POL value
//! transfer on Amoy. Asserts the receipt poll returns `status = 0x1`.
//!
//! Per L29, operator-driven. Requires a real funded signer — env var
//! `POLYGON_AMOY_PRIVATE_KEY` carries the hex private key (64 chars).
//!
//! L12 finding (security-auditor MEDIUM): the env-var private key + the
//! 32-byte scalar derived from it MUST be wrapped in `Zeroizing<...>`
//! to avoid leaving the secret in process memory past the broadcast.
//! `zeroize` is already a workspace dep (`Cargo.toml:61`).

use std::time::Duration;

use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::eip2718::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use alloy_transport_http::reqwest::Url;
use polygon_v1_spike::config::{ChainConfig, Network};
use zeroize::{Zeroize, Zeroizing};

mod common;

#[tokio::test]
#[ignore = "operator-driven per L29 — run with: RUN_POLYGON_AMOY=1 POLYGON_AMOY_PRIVATE_KEY=<64-hex> cargo test --test v8_native_pol_transfer -- --ignored"]
async fn v8_amoy_native_pol_transfer_receipt_status_success() {
    if !common::env_opt_in("RUN_POLYGON_AMOY") {
        eprintln!("[V8] SKIP — set RUN_POLYGON_AMOY=1 to enable POL transfer");
        return;
    }
    // Zeroizing wrapper: scratch the scalar at function exit (zeroize
    // crate auto-zeros on `Drop`). The `String` and `[u8; 32]` cases
    // both implement `Zeroize`; for the 32-byte scalar we use
    // `Zeroizing<[u8; 32]>` directly. See L12 security-auditor + L56
    // lesson for the precedent (eth-wallet-core `unlock_signer()`).
    let mut pk_bytes: Zeroizing<[u8; 32]> = match std::env::var("POLYGON_AMOY_PRIVATE_KEY")
        .ok()
        .map(|s| {
            Zeroizing::new(
                hex::decode(s.trim_start_matches("0x"))
                    .expect("POLYGON_AMOY_PRIVATE_KEY must be hex"),
            )
        })
        .and_then(|v| {
            if v.len() == 32 {
                let mut out = Zeroizing::new([0u8; 32]);
                out.copy_from_slice(&v);
                Some(out)
            } else {
                None
            }
        }) {
        Some(p) => p,
        None => {
            eprintln!("[V8] SKIP — set POLYGON_AMOY_PRIVATE_KEY=<64-hex> to send live POL");
            return;
        }
    };

    // Build the signer directly from a fixed-size copy that the
    // PrivateKeySigner consumes (the `B256` value is the 32-byte scalar
    // interpreted as an alloy hash). After `from_bytes`, the secret
    // remains in `pk_bytes` (zeroized on Drop) and inside the signer's
    // internal `k256::SigningKey` (not Zeroize — known limitation per
    // L53 / L56).
    let pk_b256: alloy_primitives::B256 = (*pk_bytes).into();
    let sender = PrivateKeySigner::from_bytes(&pk_b256)
        .map_err(|_| "POLYGON_AMOY_PRIVATE_KEY rejected by signer")
        .expect("valid private key");

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

    let receipt = common::await_receipt(&provider, tx_hash, 40, Duration::from_millis(500))
        .await
        .expect("receipt must appear within 20s");

    assert!(
        receipt.status(),
        "POL transfer receipt MUST have status = true"
    );

    eprintln!(
        "[V8] PASS — POL 0.001 transfer {} -> {recipient}: tx_hash = {tx_hash}",
        sender.address()
    );

    // Explicit zeroize call before the implicit Drop (the implicit
    // Drop would also zeroize, but this documents intent at the use
    // site).
    pk_bytes.zeroize();
}
