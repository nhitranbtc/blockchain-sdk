//! V6 — Nile JSON-RPC ping (Q6) — GATED behind RUN_TRON_NILE=1 (L29).
//!
//! Plan §Q6: `POST /jsonrpc eth_chainId` returns `0xcd8690dc` (Nile). `wallet/getchainid`
//! returns HTTP 405 on TronGrid's HTTP front, so `/jsonrpc` path is required.
//! Address generation uses prefix `0x41` (same code path for mainnet/Shasta/Nile).
//! For TAPOS reference: `walletsolidity/getnowblock` (not `wallet/getnowblock`).

use tron_v1_spike::config::nile_config;
use tron_v1_spike::rpc::{JsonRpcRequest, JsonRpcResponse, NILE_CHAIN_ID_HEX};

#[test]
fn v6_nile_chain_id_via_eth_chainid() {
    if std::env::var("RUN_TRON_NILE").ok().as_deref() != Some("1") {
        eprintln!("[SKIP — RUN_TRON_NILE=1 required for V6 live Nile RPC]");
        return;
    }

    let rpc_url = nile_config().rpc_url;

    let body = serde_json::to_value(JsonRpcRequest {
        jsonrpc: "2.0",
        method: "eth_chainId",
        params: serde_json::json!([]),
        id: 1,
    })
    .unwrap();

    let resp: JsonRpcResponse<String> = reqwest::blocking::Client::new()
        .post(format!("{rpc_url}/jsonrpc"))
        .json(&body)
        .send()
        .expect("Nile RPC unreachable")
        .json()
        .expect("non-JSON response");

    assert_eq!(
        resp.result.as_deref(),
        Some(NILE_CHAIN_ID_HEX),
        "expected Nile chain-id {NILE_CHAIN_ID_HEX}"
    );
    eprintln!("[PASS] V6 Nile chain-id = {NILE_CHAIN_ID_HEX}");
}

#[test]
fn v6_nile_getnowblock_for_tapos() {
    if std::env::var("RUN_TRON_NILE").ok().as_deref() != Some("1") {
        eprintln!("[SKIP — RUN_TRON_NILE=1 required for V6 live Nile RPC]");
        return;
    }

    let rpc_url = nile_config().rpc_url;

    // walletsolidity/getnowblock (NOT wallet/getnowblock) for TAPOS per plan §Q6.
    let resp: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("{rpc_url}/walletsolidity/getnowblock"))
        .json(&serde_json::json!({}))
        .send()
        .expect("Nile RPC unreachable")
        .json()
        .expect("non-JSON response");

    let block_id = resp
        .get("blockID")
        .and_then(|v| v.as_str())
        .expect("missing blockID");
    let block_header = resp.get("block_header").expect("missing block_header");

    assert!(!block_id.is_empty());
    let _raw_data = block_header
        .get("raw_data")
        .expect("missing block_header.raw_data");
    eprintln!("[PASS] V6 Nile block_id = {block_id}");
}
