//! Wire-shape envelopes for TronGrid HTTP RPC.
//!
//! These exist so the rest of the crate has a `serde::Serialize/Deserialize`
//! shape it can reason about, not so we reimplement the TronGrid schema.
//! Anything we don't need we leave out — fewer fields means fewer
//! accidental drift surfaces.
//!
//! The actual HTTP client lives in `crate::chain::TronGridClient`; this
//! module only defines the types it sends and decodes.

use serde::{Deserialize, Serialize};

/// Response of `POST /wallet/broadcasttransaction`.
///
/// `code` is the per-TRON "transaction result code" string (e.g. `"SUCCESS"`).
/// A broadcast that returns HTTP 200 but a non-SUCCESS code is a *node-level*
/// rejection — the request reached the network but the chain refused the tx.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BroadcastReceipt {
    /// Per-TRON result code string, e.g. `"SUCCESS"`. `None` means the
    /// response did not include the field at all (TronGrid variants differ
    /// across endpoints).
    #[serde(rename = "code")]
    pub code: Option<String>,

    /// Best-effort identifier for the accepted transaction. May be absent on
    /// rejection, or differ from the locally-computed txid if the node
    /// re-derived it.
    #[serde(rename = "txid")]
    pub txid: Option<String>,

    /// Optional human-readable message, present on rejection. Never relied
    /// upon for branching logic — codes are the contract.
    #[serde(rename = "message")]
    pub message: Option<String>,
}

impl BroadcastReceipt {
    /// `true` iff the node reported success. Anything else (rejected, missing
    /// field, unrecognised code) is treated as a non-success.
    pub fn is_success(&self) -> bool {
        matches!(self.code.as_deref(), Some("SUCCESS"))
    }
}

/// Response shape for `walletsolidity/getnowblock`.
///
/// `BlockHeader` is what the caller needs to feed
/// [`crate::tx::builder::set_ref_block`]; the network reports it under
/// `/walletsolidity/getnowblock`, not `/wallet/getnowblock`, because the
/// *SolidityNode* endpoint has stronger finality guarantees (per deep-dive Q7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockHeader {
    /// `ref_block_bytes` slot — the low two bytes of `blockID`.
    #[serde(rename = "ref_block_bytes")]
    pub ref_block_bytes: Vec<u8>,

    /// `ref_block_hash` slot — bytes `[8..16]` of `blockID`.
    #[serde(rename = "ref_block_hash")]
    pub ref_block_hash: Vec<u8>,

    /// Block height, monotonic per chain. Used by the caller to recognise
    /// stale heads.
    #[serde(rename = "block_number")]
    pub block_number: u64,

    /// The 32-byte block id, hex-encoded (`blockID` in the TronGrid payload).
    /// Only the 32-byte hex form is accepted by `set_ref_block`.
    #[serde(rename = "blockID")]
    pub block_id: String,
}

/// Response of `wallet/gettransactioninfobyid`.
///
/// `fee` is in SUN. `contract_result` is a per-contract return-code string —
/// an empty array means the call succeeded at the contract level (signature
/// + execution accept), but for native TRX transfers there is no contract
///   result, so empty is the success indicator.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TransactionInfo {
    /// Network-side transaction id, in hex. Should match the local
    /// `crate::tx::sign::txid` derivation.
    #[serde(rename = "id")]
    pub id: Option<String>,

    /// Block number the transaction was included in, if it has been included.
    /// `None` is the legitimate state for a pending transaction.
    #[serde(rename = "blockNumber")]
    pub block_number: Option<u64>,

    /// Per-contract execution result. May be empty for native TRX.
    #[serde(rename = "contract_result", default)]
    pub contract_result: Vec<String>,

    /// Fee in SUN (broadly: bandwidth burned + energy × price).
    #[serde(rename = "fee")]
    pub fee: Option<u64>,
}
