//! Nile/mainnet JSON-RPC client (Q6).
//!
//! Plan §Q6: `eth_chainId` JSON-RPC method via TronGrid `/jsonrpc` returns
//! `0xcd8690dc` for Nile. `wallet/getchainid` returns HTTP 405 on TronGrid's HTTP
//! front, so the `/jsonrpc` path is required.

use serde::{Deserialize, Serialize};

/// Nile chain-id per plan §Q6 (corrected 2026-08-27 — prior doc had Shasta's
/// chain-id `0x94a9059e`).
pub const NILE_CHAIN_ID_HEX: &str = "0xcd8690dc";

/// JSON-RPC request envelope.
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest<'a> {
    pub jsonrpc: &'a str,
    pub method: &'a str,
    pub params: serde_json::Value,
    pub id: u32,
}

/// JSON-RPC response envelope.
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u32,
    #[serde(default)]
    pub result: Option<T>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
}
