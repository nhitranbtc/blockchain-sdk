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

use crate::error::Result;

/// Response of `POST /wallet/broadcasttransaction`.
///
/// `code` is the per-TRON "transaction result code" string (e.g. `"SUCCESS"`).
/// A broadcast that returns HTTP 200 but a non-SUCCESS code is a *node-level*
/// rejection — the request reached the network but the chain refused the tx.
///
/// `error` captures the `{"Error": "..."}` envelope TronGrid returns when
/// the request never made it past the gateway (malformed protobuf, missing
/// fields, etc.) — distinct from a node-level `code != "SUCCESS"` rejection.
/// Both failure modes are non-success; both carry an explainable message.
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

    /// Gateway-level error envelope (`{"Error": "..."}`). Set when the
    /// request never reached the chain — typically a malformed body. Empty
    /// for node-level rejections, which surface via `code` / `message`.
    #[serde(rename = "Error")]
    pub error: Option<String>,
}

impl BroadcastReceipt {
    /// `true` iff the node reported success. Anything else (rejected, missing
    /// field, unrecognised code, gateway error) is treated as a non-success.
    pub fn is_success(&self) -> bool {
        self.code.as_deref() == Some("SUCCESS") && self.error.is_none() && self.txid.is_some()
    }
}

/// Response shape for `walletsolidity/getnowblock`.
///
/// `BlockHeader` is what the caller needs to feed
/// [`crate::tx::builder::set_ref_block`]; the network reports it under
/// `/walletsolidity/getnowblock`, not `/wallet/getnowblock`, because the
/// *SolidityNode* endpoint has stronger finality guarantees (per deep-dive Q7).
///
/// Fields `ref_block_bytes` and `ref_block_hash` are *derived* from
/// `blockID`, not read off the wire — the live `/getnowblock` response
/// places the canonical 32-byte `blockID` at the top level but does NOT
/// surface the two TAPOS slices anywhere reachable. They are computed here
/// so callers that want to inspect them still see the canonical values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockHeader {
    /// `ref_block_bytes` slot — the low two bytes of `blockID`.
    pub ref_block_bytes: Vec<u8>,

    /// `ref_block_hash` slot — bytes `[8..16]` of `blockID`.
    pub ref_block_hash: Vec<u8>,

    /// Block height, monotonic per chain. Used by the caller to recognise
    /// stale heads.
    pub block_number: u64,

    /// The 32-byte block id, hex-encoded (`blockID` in the TronGrid payload).
    /// Only the 32-byte hex form is accepted by `set_ref_block`.
    pub block_id: String,
}

/// Wire shape of `walletsolidity/getnowblock` — private adapter between
/// TronGrid's nested envelope and the flat [`BlockHeader`] our builder
/// consumes. The fields the network actually returns do not match the
/// field names [`BlockHeader`] documents (it places `blockID` at the top
/// level but puts `number` under `block_header.raw_data`, and `ref_block_*`
/// don't appear on `block_header` at all). Keeping the adapter private
/// means the wire shape can drift without leaking into the public type.
#[derive(Debug, Deserialize)]
struct RawBlockHeaderResponse {
    #[serde(rename = "blockID")]
    block_id: String,
    block_header: RawBlockHeaderRawData,
}

#[derive(Debug, Deserialize)]
struct RawBlockHeaderRawData {
    raw_data: RawBlockHeaderRawFields,
}

#[derive(Debug, Deserialize)]
struct RawBlockHeaderRawFields {
    number: u64,
}

impl TryFrom<RawBlockHeaderResponse> for BlockHeader {
    type Error = crate::error::Error;

    fn try_from(raw: RawBlockHeaderResponse) -> Result<Self> {
        let block_id_hex = raw.block_id;
        let block_id_bytes = hex::decode(&block_id_hex).map_err(|e| {
            crate::error::Error::NodeResponse(format!(
                "getnowblock blockID is not valid hex ({len} chars): {e}",
                len = block_id_hex.len()
            ))
        })?;
        if block_id_bytes.len() != 32 {
            return Err(crate::error::Error::NodeResponse(format!(
                "getnowblock blockID must be 32 bytes, got {}",
                block_id_bytes.len()
            )));
        }

        let ref_block_bytes = block_id_bytes[block_id_bytes.len() - 2..].to_vec();
        let ref_block_hash = block_id_bytes[8..16].to_vec();

        Ok(BlockHeader {
            ref_block_bytes,
            ref_block_hash,
            block_number: raw.block_header.raw_data.number,
            block_id: block_id_hex,
        })
    }
}

/// Parse a `walletsolidity/getnowblock` response body into a [`BlockHeader`].
///
/// Internal entry point used by [`crate::chain::TronGridClient::get_now_block`]
/// — kept here so the wire-shape adapter stays private to this module.
pub(crate) fn parse_block_header_response(bytes: &[u8]) -> Result<BlockHeader> {
    let raw: RawBlockHeaderResponse = serde_json::from_slice(bytes)
        .map_err(|e| crate::error::Error::NodeResponse(format!("getnowblock decode: {e}")))?;
    BlockHeader::try_from(raw)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured 2026-09-06 from `https://nile.trongrid.io/walletsolidity/getnowblock`.
    /// Truncated to the fields the wire-shape adapter touches. If TronGrid
    /// drifts the response layout, this test fails before `v10_broadcast`
    /// runs against the live network — saves one Nile faucet drip per drift.
    const NILE_GETNOWBLOCK_FIXTURE: &str = r#"{
      "blockID": "00000000043725ef3720be45f1e46cd7e7adb91ce947e01afdf04aa8b1b7dea9",
      "block_header": {
        "raw_data": {
            "number": 70723055,
            "txTrieRoot": "d1bbfae077b62cf0017bdfa1ef40ce5d9f03db5dc5d76d85de1c200a1fad7e69",
            "witness_address": "41d4492e1c6e4850c11d196d0826be66d1542c44ec",
            "parentHash": "00000000043725eea3f45bf7b9b5bee5da5a6a74b1b49bda3033dfdf190a2474",
            "version": 37,
            "timestamp": 1788689508000
        },
        "witness_signature": "00"
      }
    }"#;

    #[test]
    fn parses_nile_getnowblock_response() {
        let header = parse_block_header_response(NILE_GETNOWBLOCK_FIXTURE.as_bytes())
            .expect("captured Nile response must parse");

        assert_eq!(
            header.block_id,
            "00000000043725ef3720be45f1e46cd7e7adb91ce947e01afdf04aa8b1b7dea9"
        );
        assert_eq!(header.block_number, 70723055);

        // ref_block_bytes == low 2 bytes of blockID (last 2 of the 32-byte id)
        assert_eq!(header.ref_block_bytes, vec![0xde, 0xa9]);
        // ref_block_hash == bytes [8..16] of blockID
        assert_eq!(
            header.ref_block_hash,
            vec![0x37, 0x20, 0xbe, 0x45, 0xf1, 0xe4, 0x6c, 0xd7]
        );
    }

    #[test]
    fn rejects_block_id_wrong_length() {
        // 31 bytes hex-encoded — one short of the canonical 32.
        let bad = r#"{
            "blockID": "00000000043725ef3720be45f1e46cd7e7adb91ce947e01afdf04aa8b1b7de",
            "block_header": { "raw_data": { "number": 70723055 } }
        }"#;
        let err = parse_block_header_response(bad.as_bytes()).unwrap_err();
        assert!(matches!(err, crate::error::Error::NodeResponse(_)));
    }

    #[test]
    fn rejects_block_id_not_hex() {
        let bad = r#"{
            "blockID": "ZZZZ0000043725ef3720be45f1e46cd7e7adb91ce947e01afdf04aa8b1b7dea9",
            "block_header": { "raw_data": { "number": 70723055 } }
        }"#;
        let err = parse_block_header_response(bad.as_bytes()).unwrap_err();
        assert!(matches!(err, crate::error::Error::NodeResponse(_)));
    }
}
