//! `tests/chain_rpc.rs` — Tier 1 + Tier 2 hardening tests for `chain::client::RpcClient`.
//!
//! Covers URL allowlist (Tier 1 finding #1), JSON-RPC envelope
//! parsing (Tier 2 finding #7), bincode wire round-trip (Tier 2
//! finding #8), custom Debug impl (Tier 2 finding #11), and rate
//! limiter behavior.
//!
//! Uses `wiremock` for the mock HTTP server (returns canned JSON-RPC
//! envelopes matching the Anza RPC server wire format).
//!
//! Does NOT use live `https://api.devnet.solana.com` — that's for
//! `tests/native.rs` + `tests/spl.rs` integration tests gated on
//! `RUN_SOL_DEVNET=1`.

use serial_test::serial;
use sol_wallet_core::chain::client::{RateLimiter, RpcClient};
use sol_wallet_core::chain::{
    get_account_info, get_balance, get_epoch_info, get_health, get_latest_blockhash,
    get_minimum_balance_for_rent_exemption, get_multiple_accounts, get_recent_prioritization_fees,
    get_signature_status, get_token_account_balance, get_token_accounts_by_owner, get_token_supply,
    get_transaction, get_version, request_airdrop, ConfirmationStatus, TransactionResponse,
    TransactionStatus,
};
use sol_wallet_core::disambig::TokenProgram;
use sol_wallet_core::error::Error;
use sol_wallet_core::tx::broadcast::{send_and_confirm, wait_for_confirm, DEFAULT_CONFIRM_TIMEOUT};
use sol_wallet_core::tx::native::prepare_sol_transfer_message;
use sol_wallet_core::tx::spl::prepare_spl_transfer_message;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature, Signer};
use solana_sdk::transaction::Transaction;
use solana_system_interface::instruction as system_instruction;
use std::time::Duration;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// =============================================================================
// Tier 1 — URL allowlist
// =============================================================================

#[test]
fn url_allowlist_accepts_https() {
    assert!(RpcClient::new("https://api.devnet.solana.com").is_ok());
}

#[test]
fn url_allowlist_accepts_http_localhost() {
    assert!(RpcClient::new("http://localhost:8899").is_ok());
}

#[test]
fn url_allowlist_accepts_http_127() {
    assert!(RpcClient::new("http://127.0.0.1:8899").is_ok());
}

#[test]
fn url_allowlist_rejects_http_attacker() {
    // Tier 1 finding #1 — cleartext HTTP to non-loopback host is
    // a MITM / signed-tx exfiltration vector. Rejected at constructor.
    let err = RpcClient::new("http://attacker.com").unwrap_err();
    assert!(matches!(err, Error::Transport(ref s) if s.contains("RPC URL must be https")));
}

#[test]
fn url_allowlist_rejects_ftp_scheme() {
    let err = RpcClient::new("ftp://example.com").unwrap_err();
    assert!(matches!(err, Error::Transport(_)));
}

#[test]
fn url_allowlist_rejects_malformed_url() {
    let err = RpcClient::new("not a url").unwrap_err();
    assert!(matches!(err, Error::Transport(_)));
}

// =============================================================================
// Tier 2 — JSON-RPC envelope parsing
// =============================================================================

#[tokio::test]
#[serial]
async fn jsonrpc_envelope_parses_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"value": 12345}
        })))
        .mount(&server)
        .await;
    let rpc = RpcClient::new(&server.uri()).unwrap();
    let raw: serde_json::Value = rpc
        .post("getBalance", serde_json::json!(["dummy"]))
        .await
        .unwrap();
    assert_eq!(raw["value"], serde_json::json!(12345));
}

#[tokio::test]
#[serial]
async fn http_5xx_returns_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let rpc = RpcClient::new(&server.uri()).unwrap();
    let raw: Result<serde_json::Value, Error> = rpc.post("getBalance", serde_json::json!([])).await;
    assert!(matches!(raw, Err(Error::Transport(_))));
}

#[tokio::test]
#[serial]
async fn malformed_json_returns_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
        .mount(&server)
        .await;
    let rpc = RpcClient::new(&server.uri()).unwrap();
    let raw: Result<serde_json::Value, Error> = rpc.post("getBalance", serde_json::json!([])).await;
    assert!(matches!(raw, Err(Error::Transport(_))));
}

#[tokio::test]
#[serial]
async fn connection_refused_returns_transport_error() {
    let rpc = RpcClient::new("http://127.0.0.1:1").unwrap();
    let raw: Result<serde_json::Value, Error> = rpc.post("getBalance", serde_json::json!([])).await;
    assert!(matches!(raw, Err(Error::Transport(_))));
}

// =============================================================================
// Tier 2 — bincode wire round-trip (Tier 2 finding #8)
// =============================================================================

#[test]
fn bincode_roundtrip_serialization() {
    // Tier 2 finding #8: bincode wire format with `legacy` config
    // must round-trip through deserialize. Anza RPC servers expect
    // this exact format; if the bincode version drifts, broadcasts
    // silently fail.
    let keypair = Keypair::new();
    let recipient = Pubkey::new_unique();
    let ix = system_instruction::transfer(&keypair.pubkey(), &recipient, 1_000_000);
    let recent_blockhash = Hash::new_from_array([1u8; 32]);
    let msg = solana_sdk::message::Message::new(&[ix], Some(&keypair.pubkey()));
    let tx = Transaction::new(&[&keypair], msg, recent_blockhash);
    let wire = bincode::serialize(&tx).expect("bincode serialize");
    let restored: Transaction = bincode::deserialize(&wire).expect("bincode deserialize");
    assert_eq!(tx.signatures, restored.signatures);
    assert_eq!(tx.message, restored.message);
}

// =============================================================================
// Tier 2 — custom Debug impl strips URL query string (Tier 2 finding #11)
// =============================================================================

#[test]
fn debug_impl_strips_url_query_string() {
    let url = "https://api.mainnet-beta.solana.com/?api_key=sk-secret-1234";
    let rpc = RpcClient::new(url).unwrap();
    let dbg = format!("{rpc:?}");
    assert!(
        !dbg.contains("sk-secret-1234"),
        "Debug leaked API key: {dbg}"
    );
    assert!(dbg.contains("api.mainnet-beta.solana.com"));
    assert!(!dbg.contains("?api_key"));
}

// =============================================================================
// Rate limiter
// =============================================================================

#[tokio::test]
#[serial]
async fn rate_limiter_disabled_sentinel_passes() {
    let limiter = RateLimiter::new(0, 0);
    for _ in 0..1000 {
        limiter.acquire("test-host").await.unwrap();
    }
}

#[tokio::test]
#[serial]
async fn rate_limiter_burst_exhaustion_returns_error_after_sleep() {
    let limiter = RateLimiter::new(5, 1);
    limiter.acquire("test-host").await.unwrap();
    let start = std::time::Instant::now();
    let result = limiter.acquire("test-host").await;
    let elapsed = start.elapsed();
    assert!(result.is_err(), "second immediate acquire should fail");
    assert!(
        elapsed >= Duration::from_millis(900),
        "should wait ~1s, was {elapsed:?}"
    );
}

// =============================================================================
// Smoke tests for the 15 RPC methods (one mock per method)
// =============================================================================

async fn mock_rpc_with_body(body: serde_json::Value) -> RpcClient {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    RpcClient::new(&server.uri()).unwrap()
}

#[tokio::test]
#[serial]
async fn smoke_get_latest_blockhash() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {"context": {"slot": 2792},
                   "value": {"blockhash": "11111111111111111111111111111111", "lastValidSlot": 12345}}
    }))
    .await;
    let (_bh, _slot) = get_latest_blockhash(&rpc).await.unwrap();
}

#[tokio::test]
#[serial]
async fn smoke_get_balance() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": {"value": 1000000}
    }))
    .await;
    let lamports = get_balance(&rpc, &Pubkey::new_unique()).await.unwrap();
    assert_eq!(lamports, 1_000_000);
}

#[tokio::test]
#[serial]
async fn smoke_get_account_info_returns_none_when_null() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": null
    }))
    .await;
    let acct = get_account_info(&rpc, &Pubkey::new_unique()).await.unwrap();
    assert!(acct.is_none());
}

#[tokio::test]
#[serial]
async fn smoke_get_signature_status_returns_none_when_null() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": {"value": [null]}
    }))
    .await;
    let keypair = Keypair::new();
    let status = get_signature_status(&rpc, &{
        let mut b = [0u8; 64];
        b.copy_from_slice(&keypair.to_bytes()[..64]);
        Signature::from(b)
    })
    .await
    .unwrap();
    assert!(status.is_none());
}

#[tokio::test]
#[serial]
async fn smoke_get_signature_status_returns_some_when_confirmed() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {"value": [{
            "slot": 100, "confirmations": 1,
            "confirmationStatus": "confirmed",
            "err": null, "blockTime": 1234567890
        }]}
    }))
    .await;
    let keypair = Keypair::new();
    let status: Option<TransactionStatus> = get_signature_status(&rpc, &{
        let mut b = [0u8; 64];
        b.copy_from_slice(&keypair.to_bytes()[..64]);
        Signature::from(b)
    })
    .await
    .unwrap();
    let s = status.expect("status should be Some");
    assert_eq!(s.slot, 100);
    assert_eq!(s.confirmation_status, Some(ConfirmationStatus::Confirmed));
}

#[tokio::test]
#[serial]
async fn smoke_get_minimum_balance_for_rent_exemption() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": 890880
    }))
    .await;
    let lamports = get_minimum_balance_for_rent_exemption(&rpc, 165)
        .await
        .unwrap();
    assert_eq!(lamports, 890880);
}

#[tokio::test]
#[serial]
async fn smoke_get_recent_prioritization_fees() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": [{"slot": 100, "prioritizationFee": 5000}]
    }))
    .await;
    let fees = get_recent_prioritization_fees(&rpc, &[]).await.unwrap();
    assert_eq!(fees.len(), 1);
    assert_eq!(fees[0].prioritization_fee, 5000);
}

#[tokio::test]
#[serial]
async fn smoke_get_version() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {"context": {"slot": 2792},
                   "value": {"solana-core": "1.18.26", "feature-set": 12345}}
    }))
    .await;
    let v = get_version(&rpc).await.unwrap();
    assert_eq!(v.solana_core, "1.18.26");
    assert_eq!(v.feature_set, Some(12345));
}

#[tokio::test]
#[serial]
async fn smoke_get_health_ok() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": "ok"
    }))
    .await;
    get_health(&rpc).await.unwrap();
}

#[tokio::test]
#[serial]
async fn smoke_get_epoch_info() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {
            "context": {"slot": 2792},
            "value": {
                "absoluteSlot": 100, "blockHeight": 50, "epoch": 1,
                "slotIndex": 100, "slotsInEpoch": 8192, "transactionCount": 1000
            }
        }
    }))
    .await;
    let info = get_epoch_info(&rpc).await.unwrap();
    assert_eq!(info.epoch, 1);
    assert_eq!(info.absolute_slot, 100);
}

#[tokio::test]
#[serial]
async fn smoke_get_token_account_balance() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {
            "value": {
                "amount": "1000000", "decimals": 6,
                "uiAmount": 1.0, "uiAmountString": "1"
            }
        }
    }))
    .await;
    let bal = get_token_account_balance(&rpc, &Pubkey::new_unique())
        .await
        .unwrap();
    assert_eq!(bal.amount, "1000000");
    assert_eq!(bal.decimals, 6);
}

#[tokio::test]
#[serial]
async fn smoke_get_token_supply() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {
            "context": {"slot": 2792},
            "value": {
                "amount": "1000000000", "decimals": 6,
                "uiAmount": 1000.0, "uiAmountString": "1000"
            }
        }
    }))
    .await;
    let sup = get_token_supply(&rpc, &Pubkey::new_unique()).await.unwrap();
    assert_eq!(sup.decimals, 6);
}

#[tokio::test]
#[serial]
async fn smoke_get_multiple_accounts_empty() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": {"value": []}
    }))
    .await;
    let accts = get_multiple_accounts(&rpc, &[Pubkey::new_unique()])
        .await
        .unwrap();
    assert_eq!(accts.len(), 0);
}

#[tokio::test]
#[serial]
async fn smoke_get_token_accounts_by_owner_empty() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": {"value": []}
    }))
    .await;
    let owner = Pubkey::new_unique();
    let program = Pubkey::new_unique();
    let accts = get_token_accounts_by_owner(&rpc, &owner, &program)
        .await
        .unwrap();
    assert_eq!(accts.len(), 0);
}

// =============================================================================
// Preflight smoke
// =============================================================================

#[tokio::test]
#[serial]
async fn preflight_check_native_balance_ok() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": {"value": 10_000_000}
    }))
    .await;
    sol_wallet_core::chain::preflight::check_native_balance(
        &rpc,
        &Pubkey::new_unique(),
        1_000_000,
        5_000,
    )
    .await
    .unwrap();
}

#[tokio::test]
#[serial]
async fn preflight_check_native_balance_insufficient_returns_err() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": {"value": 100_000}
    }))
    .await;
    let err = sol_wallet_core::chain::preflight::check_native_balance(
        &rpc,
        &Pubkey::new_unique(),
        1_000_000,
        5_000,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, Error::InsufficientFunds { .. }));
}

#[tokio::test]
#[serial]
async fn preflight_check_ata_exists_false() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": null
    }))
    .await;
    let exists = sol_wallet_core::chain::preflight::check_ata_exists(&rpc, &Pubkey::new_unique())
        .await
        .unwrap();
    assert!(!exists);
}

// =============================================================================
// Native + SPL builder + broadcast smoke (in-process, no real RPC)
// =============================================================================

#[test]
fn native_builder_produces_3_ixs() {
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();
    let bh = Hash::new_from_array([1u8; 32]);
    let msg = prepare_sol_transfer_message(&from, &to, 1_000_000, 150_000, 0, bh);
    assert_eq!(msg.instructions.len(), 3);
}

#[test]
fn spl_builder_produces_3_ixs_without_ata_create() {
    let wallet = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let src = Pubkey::new_unique();
    let dst = Pubkey::new_unique();
    let bh = Hash::new_from_array([1u8; 32]);
    let msg = prepare_spl_transfer_message(
        &wallet,
        &src,
        &dst,
        &mint,
        TokenProgram::Classic,
        1_000_000,
        6,
        150_000,
        0,
        bh,
        false,
    );
    assert_eq!(msg.instructions.len(), 3);
}

#[test]
fn spl_builder_produces_4_ixs_with_ata_create() {
    let wallet = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let src = Pubkey::new_unique();
    let dst = Pubkey::new_unique();
    let bh = Hash::new_from_array([1u8; 32]);
    let msg = prepare_spl_transfer_message(
        &wallet,
        &src,
        &dst,
        &mint,
        TokenProgram::Classic,
        1_000_000,
        6,
        150_000,
        0,
        bh,
        true,
    );
    assert_eq!(msg.instructions.len(), 4);
}

#[tokio::test]
#[serial]
async fn broadcast_send_and_confirm_returns_signature_on_success() {
    let server = MockServer::start().await;
    let sig_base58 = "5".to_string() + &"1".repeat(86);
    Mock::given(method("POST"))
        .and(body_partial_json(
            serde_json::json!({"method": "getLatestBlockhash"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"context": {"slot": 2792},
                       "value": {"blockhash": "11111111111111111111111111111111", "lastValidSlot": 100}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            serde_json::json!({"method": "sendTransaction"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": sig_base58
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            serde_json::json!({"method": "getSignatureStatuses"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"value": [{
                "slot": 100, "confirmations": 1,
                "confirmationStatus": "confirmed", "err": null, "blockTime": 1234567890
            }]}
        })))
        .mount(&server)
        .await;
    let rpc = RpcClient::new(&server.uri()).unwrap();
    let keypair = Keypair::new();
    let recipient = Pubkey::new_unique();
    let _msg = prepare_sol_transfer_message(
        &keypair.pubkey(),
        &recipient,
        1_000_000,
        150_000,
        0,
        Hash::new_from_array([1u8; 32]),
    );
    let sys_ix = solana_system_interface::instruction::transfer(
        &keypair.pubkey(),
        &Pubkey::new_unique(),
        1_000_000,
    );
    let msg_for_tx = solana_sdk::message::Message::new(&[sys_ix], Some(&keypair.pubkey()));
    let tx = Transaction::new(&[&keypair], msg_for_tx, Hash::new_from_array([1u8; 32]));
    let sig = send_and_confirm(
        &rpc,
        &tx,
        CommitmentConfig::confirmed(),
        DEFAULT_CONFIRM_TIMEOUT,
    )
    .await
    .unwrap();
    assert!(
        sig.to_string().len() >= 87,
        "signature base58 length {} < 87",
        sig.to_string().len()
    );
}

#[tokio::test]
#[serial]
async fn wait_for_confirm_returns_confirm_pending_at_half_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            serde_json::json!({"method": "getSignatureStatuses"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"value": [{
                "slot": 100, "confirmations": 10,
                "confirmationStatus": "processed", "err": null
            }]}
        })))
        .mount(&server)
        .await;
    let rpc = RpcClient::new(&server.uri()).unwrap();
    let keypair = Keypair::new();
    let start = std::time::Instant::now();
    let err = wait_for_confirm(
        &rpc,
        &{
            let mut b = [0u8; 64];
            b.copy_from_slice(&keypair.to_bytes()[..64]);
            Signature::from(b)
        },
        CommitmentConfig::confirmed(),
        Duration::from_secs(2),
    )
    .await
    .unwrap_err();
    let elapsed = start.elapsed();
    assert!(
        matches!(err, Error::ConfirmPending { .. }),
        "expected ConfirmPending, got {err:?}"
    );
    assert!(
        elapsed < Duration::from_millis(1500),
        "should fire at ~1s, was {elapsed:?}"
    );
}

// =============================================================================
// Task 5.2 — request_airdrop with devnet host allowlist (Tier 3 finding #6)
// =============================================================================

#[tokio::test]
#[serial]
async fn request_airdrop_rejects_mainnet_host() {
    // Q8 grilled decision: requestAirdrop is a CLUSTER-LEVEL restriction
    // (mainnet rejects it). The RpcClient doesn't carry a mode flag —
    // the method enforces its own host policy.
    let rpc = RpcClient::new("https://api.mainnet-beta.solana.com").unwrap();
    let err = request_airdrop(&rpc, &Pubkey::new_unique(), 1_000_000_000)
        .await
        .unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("mainnet-beta.solana.com"),
        "error should mention offending host, got: {msg}"
    );
    assert!(
        msg.contains("devnet allowlist"),
        "error should mention devnet allowlist, got: {msg}"
    );
}

#[tokio::test]
#[serial]
async fn request_airdrop_rejects_unknown_host() {
    let rpc = RpcClient::new("https://attacker.com").unwrap();
    let err = request_airdrop(&rpc, &Pubkey::new_unique(), 1_000_000_000)
        .await
        .unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("attacker.com"),
        "error should mention offending host, got: {msg}"
    );
}

#[tokio::test]
#[serial]
async fn request_airdrop_accepts_localhost() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": "5".to_string() + &"1".repeat(86)
    }))
    .await;
    let sig = request_airdrop(&rpc, &Pubkey::new_unique(), 1_000_000_000)
        .await
        .unwrap();
    assert!(sig.to_string().len() >= 87);
}

#[tokio::test]
#[serial]
async fn request_airdrop_rejects_non_string_result() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": 42
    }))
    .await;
    let err = request_airdrop(&rpc, &Pubkey::new_unique(), 1_000_000_000)
        .await
        .unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("not a string"),
        "error should mention type mismatch, got: {msg}"
    );
}

// =============================================================================
// Task 5.3 — get_transaction (full log decode for `sol tx`)
// =============================================================================

#[tokio::test]
#[serial]
async fn get_transaction_returns_some_on_confirmed_tx() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {
            "slot": 1234,
            "blockTime": 1700000000,
            "meta": {"err": null, "fee": 5000}
        }
    }))
    .await;
    let keypair = Keypair::new();
    let sig = {
        let mut b = [0u8; 64];
        b.copy_from_slice(&keypair.to_bytes()[..64]);
        Signature::from(b)
    };
    let resp: Option<TransactionResponse> = get_transaction(&rpc, &sig).await.unwrap();
    let r = resp.expect("transaction should be Some");
    assert_eq!(r.slot, 1234);
    assert_eq!(r.block_time, Some(1700000000));
    assert_eq!(r.meta.get("fee").and_then(|v| v.as_u64()), Some(5000));
}

#[tokio::test]
#[serial]
async fn get_transaction_returns_none_on_null() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "result": null
    }))
    .await;
    let keypair = Keypair::new();
    let sig = {
        let mut b = [0u8; 64];
        b.copy_from_slice(&keypair.to_bytes()[..64]);
        Signature::from(b)
    };
    let resp: Option<TransactionResponse> = get_transaction(&rpc, &sig).await.unwrap();
    assert!(resp.is_none());
}

#[tokio::test]
#[serial]
async fn get_transaction_rejects_missing_slot() {
    let rpc = mock_rpc_with_body(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": {"blockTime": 1700000000, "meta": {"err": null}}
    }))
    .await;
    let keypair = Keypair::new();
    let sig = {
        let mut b = [0u8; 64];
        b.copy_from_slice(&keypair.to_bytes()[..64]);
        Signature::from(b)
    };
    let err = get_transaction(&rpc, &sig).await.unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("missing field `slot`") || msg.contains("missing slot"),
        "expected missing-slot error, got: {msg}"
    );
}

// =============================================================================
// Task 5.5 — SPKI pin escape hatch (Tier 3 finding #2)
// =============================================================================

#[test]
fn spki_pin_empty_bytes_rejected() {
    // V0.1 refuses to construct an unpinned client — caller must supply
    // real SPKI bytes (DER encoding of the cluster's leaf cert pubkey).
    let err =
        RpcClient::new_with_pinned_spki("https://api.mainnet-beta.solana.com", vec![]).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("empty DER bytes"),
        "expected empty-bytes rejection, got: {msg}"
    );
    assert!(
        msg.contains("refusing to construct an unpinned client"),
        "expected security message, got: {msg}"
    );
}

#[test]
fn spki_pin_accepts_valid_bytes() {
    // 91 bytes is a typical RSA-2048 SPKI envelope; the V0.1 constructor
    // stores them verbatim (live TLS-level verification deferred to V0.1.5).
    let spki = vec![0x30u8; 91];
    let rpc = RpcClient::new_with_pinned_spki("https://api.mainnet-beta.solana.com", spki.clone())
        .expect("pinned client should construct");
    assert_eq!(rpc.pinned_spki(), Some(&spki));
}

#[test]
fn spki_pin_default_new_returns_none() {
    // `RpcClient::new` (no pin) returns `None` for `pinned_spki()` —
    // distinguishes pinned vs unpinned clients at runtime.
    let rpc = RpcClient::new("https://api.devnet.solana.com").unwrap();
    assert_eq!(rpc.pinned_spki(), None);
}

#[test]
fn spki_pin_rejects_non_allowlisted_url() {
    // The allowlist runs FIRST (Tier 1 #1). A pinned mainnet URL is fine
    // (https + any host); a pinned non-loopback http URL is rejected.
    let spki = vec![0x30u8; 91];
    let err = RpcClient::new_with_pinned_spki("http://attacker.com", spki).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("RPC URL must be https"),
        "expected allowlist rejection, got: {msg}"
    );
}
