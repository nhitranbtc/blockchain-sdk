//! TRON transaction construction + broadcast helpers for issue #408.
//!
//! Cycle 3 covers `balance_of_trc20` (HTTP + pure-parser split). Cycles 4-6
//! add `build_signed_trc20_transfer`, `broadcast`, `poll_for_confirmation`.

use serde::Deserialize;

use crate::abi::{encode_balance_of, encode_transfer};
use crate::address::from_base58check;

/// Parse the hex-encoded uint256 balance returned by TronGrid's
/// `/wallet/triggerconstantcontract` response. TRC-20 balances fit in u128
/// in practice; we parse the lowest 128 bits and surface an error if the
/// high 128 bits are non-zero (would silently truncate).
///
/// Input format: `0x` prefix optional, then 1-64 hex chars (left-padded).
pub fn parse_balance_uint256(hex: &str) -> Result<u128, BalanceParseError> {
    let s = hex.strip_prefix("0x").unwrap_or(hex);
    if s.is_empty() {
        return Ok(0);
    }
    if s.len() > 64 {
        return Err(BalanceParseError::TooLong);
    }
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(BalanceParseError::NotHex);
    }
    // Left-pad to 64 chars, then split into hi/lo u128 halves.
    let padded = format!("{:0>64}", s);
    let hi_hex = &padded[..32];
    let lo_hex = &padded[32..];
    let hi = u128::from_str_radix(hi_hex, 16).map_err(|_| BalanceParseError::NotHex)?;
    let lo = u128::from_str_radix(lo_hex, 16).map_err(|_| BalanceParseError::NotHex)?;
    if hi != 0 {
        return Err(BalanceParseError::Overflow);
    }
    Ok(lo)
}

#[derive(Debug, PartialEq, Eq)]
pub enum BalanceParseError {
    /// Hex string was longer than 64 chars (truncated input).
    TooLong,
    /// Hex string contained non-hex characters.
    NotHex,
    /// Value exceeded u128 (high 128 bits non-zero).
    Overflow,
}

impl std::fmt::Display for BalanceParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::TooLong => "balance hex longer than 64 chars",
            Self::NotHex => "balance hex contains non-hex characters",
            Self::Overflow => "balance exceeds u128 range (high 128 bits non-zero)",
        };
        f.write_str(s)
    }
}

impl std::error::Error for BalanceParseError {}

/// JSON response shape for `/wallet/triggerconstantcontract`. The fullnode
/// returns the constant-call return data in **either** of two places
/// (verified live 2026-08-27 against `nile.trongrid.io`):
/// - `constant_result: ["<hex>"]` — top-level array (TronGrid)
/// - `result.result: "<hex>"` — nested object (older endpoints / some nodes)
///
/// Both carry the same data: the first element / nested field is the
/// hex-encoded uint256 big-endian balance.
#[derive(Debug, Deserialize)]
pub struct TriggerConstantResponse {
    #[serde(default)]
    pub result: Option<TriggerConstantResult>,
    #[serde(default)]
    pub constant_result: Option<Vec<String>>,
    #[serde(default)]
    pub energy_used: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct TriggerConstantResult {
    /// Either a hex-encoded return data string (older endpoints — for
    /// `balanceOf`, the uint256 big-endian) OR a boolean success flag
    /// (TronGrid sets this to `true`). We deserialize as `Value` so the
    /// caller can match on type — see `balance_of_trc20`.
    pub result: Option<serde_json::Value>,
}

/// Convert a T-base58check address to its 20-byte payload (strips `0x41`
/// prefix). Returns `None` on malformed input.
fn t_addr_to_20(t: &str) -> Option<[u8; 20]> {
    let raw21 = from_base58check(t).ok()?;
    if raw21.len() != 21 || raw21[0] != 0x41 {
        return None;
    }
    let mut out = [0u8; 20];
    out.copy_from_slice(&raw21[1..]);
    Some(out)
}

/// Query a TRC-20 token's `balanceOf(holder_t)` via the SPKI-pinned RPC
/// client. Returns the raw balance (token base units, 6-dec for USDT-TRC20).
///
/// `contract_t` and `holder_t` must be valid T-base58check addresses on the
/// same network as the RPC client.
pub async fn balance_of_trc20(
    rpc: &crate::rpc::JsonRpcClient,
    contract_t: &str,
    holder_t: &str,
) -> Result<u128, BalanceError> {
    let _contract_20 = t_addr_to_20(contract_t).ok_or(BalanceError::BadAddress)?;
    let holder_20 = t_addr_to_20(holder_t).ok_or(BalanceError::BadAddress)?;
    let calldata = encode_balance_of(&holder_20);
    // TronGrid's `/wallet/triggerconstantcontract` (mirrors the
    // `triggersmartcontract` fix in `build_signed_trc20_transfer`) prepends
    // the function selector itself — we send only the encoded args
    // (32-byte address slot, dropping the leading 4-byte selector).
    // Verified live 2026-08-27: with full 36-byte payload the server
    // returns zero; with the 32-byte arg-only payload it returns the
    // correct balance.
    let calldata_hex = hex::encode(&calldata[4..]);

    let body = serde_json::json!({
        "owner_address": holder_t,
        "contract_address": contract_t,
        "function_selector": "balanceOf(address)",
        "parameter": calldata_hex,
        "call_value": 0,
        "visible": true,
    });

    let resp: TriggerConstantResponse = rpc
        .post_trc20_constant(&body)
        .await
        .map_err(BalanceError::Rpc)?;

    // Prefer `constant_result[0]` (TronGrid) — fall back to nested
    // `result.result` only when it's a string (older endpoints). On
    // TronGrid the nested field is a `true` success flag, not the balance
    // hex, so we filter out non-strings explicitly.
    let raw_hex = resp
        .constant_result
        .as_ref()
        .and_then(|arr| arr.first().cloned())
        .or_else(|| {
            resp.result
                .as_ref()
                .and_then(|r| r.result.as_ref())
                .and_then(|v| v.as_str().map(String::from))
        })
        .unwrap_or_default();
    parse_balance_uint256(&raw_hex).map_err(BalanceError::Parse)
}

#[derive(Debug)]
pub enum BalanceError {
    /// T-address did not decode or had wrong prefix.
    BadAddress,
    /// HTTP / JSON-RPC transport error.
    Rpc(crate::rpc::JsonRpcError),
    /// Response hex failed to parse into u128.
    Parse(BalanceParseError),
}

impl std::fmt::Display for BalanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadAddress => f.write_str("bad T-address"),
            Self::Rpc(e) => write!(f, "rpc: {e}"),
            Self::Parse(e) => write!(f, "parse: {e}"),
        }
    }
}

impl std::error::Error for BalanceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_balance_zero_hex_with_prefix() {
        assert_eq!(parse_balance_uint256("0x0").unwrap(), 0);
    }

    #[test]
    fn parse_balance_zero_empty_string() {
        assert_eq!(parse_balance_uint256("").unwrap(), 0);
    }

    #[test]
    fn parse_balance_100_usdt_base_units() {
        // 100 USDT × 10^6 = 100_000_000 = 0x5f5e100
        assert_eq!(
            parse_balance_uint256(
                "0x000000000000000000000000000000000000000000000000000000005f5e100"
            )
            .unwrap(),
            100_000_000
        );
    }

    #[test]
    fn parse_balance_short_hex_pads_left() {
        // 0x5f5e100 unpadded → parsed as left-padded 64-char → lo == 0x5f5e100
        assert_eq!(parse_balance_uint256("0x5f5e100").unwrap(), 100_000_000);
    }

    #[test]
    fn parse_balance_rejects_non_hex() {
        assert_eq!(
            parse_balance_uint256("0xZZZZ"),
            Err(BalanceParseError::NotHex)
        );
    }

    #[test]
    fn parse_balance_rejects_too_long() {
        let s = "a".repeat(65);
        assert_eq!(parse_balance_uint256(&s), Err(BalanceParseError::TooLong));
    }

    #[test]
    fn parse_balance_rejects_u128_overflow() {
        // 64-char hex with high u128 = 1, low u128 = 0 → overflow
        let s = format!("1{}", "0".repeat(63));
        assert_eq!(parse_balance_uint256(&s), Err(BalanceParseError::Overflow));
    }

    #[test]
    fn parse_balance_round_trip_through_decode_response_shape() {
        // Simulate the wire format: full 32-byte uint256 left-padded.
        // Older endpoint shape — nested `result.result` is the balance hex.
        let wire = serde_json::json!({
            "result": { "result": "0000000000000000000000000000000000000000000000000000000005f5e100" }
        });
        let resp: TriggerConstantResponse = serde_json::from_value(wire).unwrap();
        let nested = resp.result.unwrap().result.unwrap();
        let hex = nested.as_str().unwrap();
        assert_eq!(parse_balance_uint256(hex).unwrap(), 100_000_000);
    }

    /// Live-captured shape from nile.trongrid.io 2026-08-27 — the balance
    /// hex lives at top-level `constant_result[0]`, NOT under `result`.
    #[test]
    fn parse_balance_round_trip_through_constant_result_shape() {
        let wire = serde_json::json!({
            "transaction": {"txID": "ignored"},
            "constant_result": ["0000000000000000000000000000000000000000000000000000000005f5e100"],
            "result": { "result": true },
            "energy_used": 935
        });
        let resp: TriggerConstantResponse = serde_json::from_value(wire).unwrap();
        // Verify the constant_result path is reachable (this is what the
        // bug-fix branch reads first).
        assert!(
            resp.constant_result.is_some(),
            "must deserialize constant_result"
        );
        let hex = resp
            .constant_result
            .as_ref()
            .and_then(|arr| arr.first().cloned())
            .expect("first element of constant_result");
        assert_eq!(parse_balance_uint256(&hex).unwrap(), 100_000_000);
    }

    // Keep abi::encode_transfer reachable from this module so cycles 4-6
    // don't have to re-import the path. No-op reference.
    #[allow(dead_code)]
    fn _abi_link() {
        let _ = encode_transfer(&[0u8; 20], &[0u8; 32]);
    }
}

/// JSON response shape for `/wallet/triggersmartcontract`. The envelope
/// (`raw_data_hex` + `txID`) lives **inside the `transaction` object**;
/// `result` is just a `{ "result": true }` success flag at the top level.
#[derive(Debug, Deserialize)]
pub struct TriggerSmartResponse {
    #[serde(default)]
    pub transaction: Option<TransactionEnvelope>,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionEnvelope {
    /// Structured raw transaction body (T-addresses when `visible=true`,
    /// hex addresses otherwise). REQUIRED by `/wallet/broadcasttransaction` —
    /// the node re-serializes this JSON object into protobuf bytes for SHA-256
    /// signature verification. Without it, the node's `Util.packTransaction`
    /// silently catches `JsonFormat$ParseException` and returns null,
    /// downstream `TransactionCapsule(null)` triggers NPE. See
    /// <https://github.com/tronprotocol/documentation-en/blob/master/docs/api/http/tx-build-and-broadcast/broadcasttransaction.md>.
    #[serde(default)]
    pub raw_data: serde_json::Value,
    /// Hex-encoded raw transaction body (the part that gets signed).
    /// Client-side display helper; node ignores when `visible=true`.
    #[serde(default)]
    pub raw_data_hex: Option<String>,
    /// Hex-encoded tx id (SHA-256 of `raw_data_hex`).
    #[serde(default, rename = "txID")]
    pub tx_id: Option<String>,
}

/// Fully-signed TRC-20 transfer transaction ready for broadcast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedTransferTx {
    /// tx_id = SHA-256 of the raw_data bytes (= `txID` returned by TronGrid).
    pub tx_id: String,
    /// Hex-encoded raw transaction body (already covered by `tx_id` SHA-256).
    pub raw_data_hex: String,
    /// 65-byte signature: `r (32) ‖ s (32) ‖ v (1)`, where `v ∈ {0, 1}` per plan §Q8.
    /// Hex-encoded for wire format.
    pub signature_hex: String,
    /// JSON body to POST to `/wallet/broadcasttransaction`.
    pub broadcast_body: serde_json::Value,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SignError {
    /// `raw_data_hex` field missing or not valid hex.
    BadEnvelope,
    /// k256 ECDSA signing failed.
    Signing(String),
    /// Computed tx_id did not match server-returned `txID` (sanity check).
    TxIdMismatch,
}

impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadEnvelope => f.write_str("envelope missing raw_data_hex or txID"),
            Self::Signing(e) => write!(f, "signing: {e}"),
            Self::TxIdMismatch => f.write_str("computed txID != server-returned txID"),
        }
    }
}

impl std::error::Error for SignError {}

/// Sign a server-built `triggersmartcontract` envelope using the sender's
/// secp256k1 key. Pure function — no HTTP. The signature byte is `v ∈ {0, 1}`
/// (TRON convention), NOT `v + 27` (Ethereum convention) per plan §Q8.
///
/// The signature is appended to `raw_data_hex` to form the full transaction
/// (the `signature` field in the protobuf transaction). Returns the full
/// `broadcast_body` JSON ready to POST to `/wallet/broadcasttransaction`.
pub fn sign_triggersmartcontract_response(
    envelope: &TriggerSmartResponse,
    sender_sk: &k256::ecdsa::SigningKey,
) -> Result<SignedTransferTx, SignError> {
    use k256::ecdsa::signature::hazmat::PrehashSigner;

    let txn = envelope
        .transaction
        .as_ref()
        .ok_or(SignError::BadEnvelope)?;
    let raw_data_hex = txn.raw_data_hex.as_ref().ok_or(SignError::BadEnvelope)?;
    let server_tx_id = txn.tx_id.as_ref().ok_or(SignError::BadEnvelope)?;

    // Decode raw_data for SHA-256 verification of txID.
    let raw_data_bytes = hex::decode(raw_data_hex).map_err(|_| SignError::BadEnvelope)?;

    // Verify tx_id consistency: SHA-256(raw_data) must equal server txID.
    use sha2::Digest;
    let computed_tx_id: [u8; 32] = sha2::Sha256::digest(&raw_data_bytes).into();
    let computed_tx_id_hex = hex::encode(computed_tx_id);
    if computed_tx_id_hex.as_str() != server_tx_id.as_str() {
        return Err(SignError::TxIdMismatch);
    }

    // Sign the 32-byte tx_id (stand-in for the production `txID =
    // SHA256(raw_data)` per plan §Q2 + §Q8).
    let sig: k256::ecdsa::Signature = sender_sk
        .sign_prehash(&computed_tx_id)
        .map_err(|e| SignError::Signing(e.to_string()))?;
    let (_rec_sig, rid) = sender_sk
        .sign_prehash_recoverable(&computed_tx_id)
        .map_err(|e| SignError::Signing(e.to_string()))?;
    let v = rid.to_byte();
    debug_assert!(v <= 1, "TRON v byte must be ∈ {{0, 1}}, got {v}");

    let mut sig65 = [0u8; 65];
    sig65[..64].copy_from_slice(&sig.to_bytes());
    sig65[64] = v;
    let signature_hex = hex::encode(sig65);

    let broadcast_body = serde_json::json!({
        "raw_data": txn.raw_data,
        "raw_data_hex": raw_data_hex,
        "txID": server_tx_id,
        "signature": [signature_hex],
        "visible": true,
    });

    Ok(SignedTransferTx {
        tx_id: server_tx_id.clone(),
        raw_data_hex: raw_data_hex.clone(),
        signature_hex,
        broadcast_body,
    })
}

/// Build + sign a TRC-20 transfer transaction for `amount` base units of
/// `contract_t` from `sender_t` to `recipient_t`. Calls the SPKI-pinned RPC
/// client to obtain the server-built envelope, then signs locally.
pub async fn build_signed_trc20_transfer(
    rpc: &crate::rpc::JsonRpcClient,
    sender_sk: &k256::ecdsa::SigningKey,
    sender_t: &str,
    contract_t: &str,
    recipient_t: &str,
    amount: u64,
) -> Result<SignedTransferTx, SignError> {
    let recipient_20 = t_addr_to_20(recipient_t).ok_or(SignError::BadEnvelope)?;
    let mut value32 = [0u8; 32];
    value32[24..].copy_from_slice(&amount.to_be_bytes());
    // TronGrid's `/wallet/triggersmartcontract` appends the function selector
    // to the `parameter` itself — we send only the encoded args.
    let calldata = encode_transfer(&recipient_20, &value32);
    let calldata_hex = hex::encode(&calldata[4..]);

    let body = serde_json::json!({
        "owner_address": sender_t,
        "contract_address": contract_t,
        "function_selector": "transfer(address,uint256)",
        "parameter": calldata_hex,
        "call_value": 0,
        "fee_limit": 100_000_000, // 100 TRX sun; per V5 sizing formula in production
        "visible": true,
    });

    let envelope: TriggerSmartResponse = rpc
        .post_triggersmartcontract(&body)
        .await
        .map_err(|_| SignError::BadEnvelope)?;
    sign_triggersmartcontract_response(&envelope, sender_sk)
}

#[cfg(test)]
mod sign_tests {
    use super::*;
    use k256::ecdsa::SigningKey;

    fn deterministic_sk() -> SigningKey {
        // 32-byte scalar = 1 (zero scalar is invalid for k256).
        let bytes: [u8; 32] = {
            let mut b = [0u8; 32];
            b[31] = 1;
            b
        };
        SigningKey::from_bytes(&bytes.into()).expect("valid scalar")
    }

    fn sample_envelope() -> TriggerSmartResponse {
        // Real envelope shape: raw_data_hex is 64+ hex chars, txID = SHA-256
        // of raw_data_hex decoded.
        let raw = b"tron-v1-spike raw_data placeholder for deterministic test";
        let raw_hex = hex::encode(raw);
        let tx_id = {
            use sha2::Digest;
            hex::encode(sha2::Sha256::digest(raw))
        };
        TriggerSmartResponse {
            transaction: Some(TransactionEnvelope {
                raw_data: serde_json::Value::Null,
                raw_data_hex: Some(raw_hex),
                tx_id: Some(tx_id),
            }),
            result: Some(serde_json::json!({"result": true})),
        }
    }

    #[test]
    fn sign_produces_65_byte_signature_with_v_in_0_1() {
        let env = sample_envelope();
        let sk = deterministic_sk();
        let signed = sign_triggersmartcontract_response(&env, &sk).expect("sign");
        let sig_bytes = hex::decode(&signed.signature_hex).expect("hex sig");
        assert_eq!(sig_bytes.len(), 65);
        assert!(
            sig_bytes[64] <= 1,
            "v byte ∈ {{0, 1}}, got {}",
            sig_bytes[64]
        );
    }

    #[test]
    fn sign_tx_id_matches_envelope() {
        let env = sample_envelope();
        let sk = deterministic_sk();
        let signed = sign_triggersmartcontract_response(&env, &sk).expect("sign");
        let expected = env
            .transaction
            .as_ref()
            .and_then(|t| t.tx_id.as_ref())
            .unwrap()
            .clone();
        assert_eq!(signed.tx_id, expected);
    }

    #[test]
    fn sign_broadcast_body_has_required_fields() {
        let env = sample_envelope();
        let sk = deterministic_sk();
        let signed = sign_triggersmartcontract_response(&env, &sk).expect("sign");
        let body = &signed.broadcast_body;
        assert!(
            body.get("raw_data").is_some(),
            "broadcast missing raw_data (issue #409 NPE fix)"
        );
        assert!(
            body.get("raw_data_hex").is_some(),
            "broadcast missing raw_data_hex"
        );
        assert!(
            body.get("signature").is_some(),
            "broadcast missing signature"
        );
        assert_eq!(body["visible"], serde_json::Value::Bool(true));
        let sigs = body["signature"].as_array().expect("signature array");
        assert_eq!(sigs.len(), 1);
        assert_eq!(
            sigs[0],
            serde_json::Value::String(signed.signature_hex.clone())
        );
    }

    #[test]
    fn sign_rejects_tx_id_mismatch() {
        let mut env = sample_envelope();
        // Mutate raw_data_hex WITHOUT updating txID — txID consistency check
        // must fire.
        let raw = b"different raw data";
        env.transaction.as_mut().unwrap().raw_data_hex = Some(hex::encode(raw));
        let sk = deterministic_sk();
        assert_eq!(
            sign_triggersmartcontract_response(&env, &sk),
            Err(SignError::TxIdMismatch)
        );
    }

    #[test]
    fn sign_rejects_missing_envelope() {
        let env = TriggerSmartResponse {
            transaction: None,
            result: None,
        };
        let sk = deterministic_sk();
        assert_eq!(
            sign_triggersmartcontract_response(&env, &sk),
            Err(SignError::BadEnvelope)
        );
    }
}

/// JSON response shape for `/wallet/broadcasttransaction` (only the
/// fields we read).
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct BroadcastReceipt {
    /// `true` when the network accepted the transaction into the mempool.
    /// `false` means the network rejected it (see `code` + `message`).
    #[serde(default)]
    pub result: Option<bool>,
    /// Hex-encoded tx id (echoed from broadcast body).
    #[serde(default)]
    pub txid: Option<String>,
    /// Error code when `result == false`.
    #[serde(default)]
    pub code: Option<String>,
    /// Error message when `result == false`.
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BroadcastError {
    /// Server rejected the broadcast (returned `result: false`).
    Rejected { code: String, message: String },
    /// HTTP / JSON-RPC transport error.
    Rpc(String),
    /// Response was missing the `result` flag.
    Malformed,
}

impl std::fmt::Display for BroadcastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected { code, message } => write!(f, "rejected ({code}): {message}"),
            Self::Rpc(e) => write!(f, "rpc: {e}"),
            Self::Malformed => f.write_str("broadcast response missing result flag"),
        }
    }
}

impl std::error::Error for BroadcastError {}

/// Pure parser — converts the raw `/wallet/broadcasttransaction` response
/// body into a structured receipt or surfaces rejection details.
pub fn parse_broadcast_response(
    body: serde_json::Value,
) -> Result<BroadcastReceipt, BroadcastError> {
    let receipt: BroadcastReceipt =
        serde_json::from_value(body).map_err(|_| BroadcastError::Malformed)?;
    match receipt.result {
        Some(true) => Ok(receipt),
        Some(false) => Err(BroadcastError::Rejected {
            code: receipt.code.unwrap_or_default(),
            message: receipt.message.unwrap_or_default(),
        }),
        None => Err(BroadcastError::Malformed),
    }
}

/// Broadcast a signed transaction via the SPKI-pinned RPC client.
pub async fn broadcast(
    rpc: &crate::rpc::JsonRpcClient,
    signed: &SignedTransferTx,
) -> Result<BroadcastReceipt, BroadcastError> {
    let resp: serde_json::Value = rpc
        .post_broadcasttransaction(&signed.broadcast_body)
        .await
        .map_err(|e| BroadcastError::Rpc(e.to_string()))?;
    parse_broadcast_response(resp)
}

// Old `/wallet/gettransactionbyid`-based poll logic removed — replaced by
// `tx_confirmed_in_receipt` + `/wallet/gettransactioninfobyid` below for
// response-to-request binding (the old path did not echo `txID`, breaking
// the security review's request-binding requirement).

#[derive(Debug, PartialEq, Eq)]
pub enum PollError {
    /// HTTP / JSON-RPC transport error.
    Rpc(String),
    /// Response body was not valid JSON.
    Decode,
    /// Loop exhausted `deadline` without seeing the tx.
    Timeout,
}

impl std::fmt::Display for PollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rpc(e) => write!(f, "rpc: {e}"),
            Self::Decode => f.write_str("response decode"),
            Self::Timeout => f.write_str("timeout waiting for confirmation"),
        }
    }
}

impl std::error::Error for PollError {}

/// JSON response shape for `/wallet/gettransactioninfobyid`. The fullnode
/// returns `{"id": "<tx_id>", ...}` for a seen tx, or `{}` / `{"id": null}`
/// when the tx is still pending. Unlike `gettransactionbyid`, this endpoint
/// echoes the submitted `id` — letting the caller bind response to request
/// without trusting the URL `value=<tx_id>` filter alone. Verified live
/// 2026-08-27 against `nile.trongrid.io`.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetTransactionInfoByIdResponse {
    /// Hex-encoded tx id — the only reliable response-to-request binding
    /// for the poll loop. Missing or `None` means the tx is not yet seen.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub receipt: Option<TransactionReceipt>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct TransactionReceipt {
    /// `"SUCCESS"` when the contract executed cleanly. Any other value
    /// (`"REVERT"`, `"OUT_OF_ENERGY"`, ...) or absent means failure.
    #[serde(default)]
    pub result: Option<String>,
}

/// Pure check: does the receipt response indicate the tx has been seen by
/// the network AND executed successfully? Two gates, both required:
///
/// 1. **ID binding** — the response's `id` field MUST equal `tx_id` (case-
///    insensitive hex). This prevents a malicious or buggy fullnode from
///    claiming success for a different transaction.
/// 2. **Execution status** — `receipt.result` MUST be exactly `"SUCCESS"`.
///
/// Either gate failing returns `false` (treat as "not yet confirmed").
pub fn tx_confirmed_in_receipt(body: serde_json::Value, tx_id: &str) -> Result<bool, PollError> {
    let parsed: GetTransactionInfoByIdResponse =
        serde_json::from_value(body).map_err(|_| PollError::Decode)?;
    let Some(recorded_id) = parsed.id else {
        return Ok(false);
    };
    if !recorded_id.eq_ignore_ascii_case(tx_id) {
        return Ok(false);
    }
    let receipt_result = parsed
        .receipt
        .as_ref()
        .and_then(|r| r.result.as_ref())
        .map(String::as_str);
    Ok(receipt_result == Some("SUCCESS"))
}

/// Poll `/wallet/gettransactioninfobyid` every `poll_interval` until `tx_id`
/// appears AND `receipt.result == "SUCCESS"`, OR `deadline` elapses.
/// Returns `Ok(())` on confirmation, `Err(PollError::Timeout)` on exhaustion.
///
/// Security: the poll uses the receipt endpoint (NOT `/gettransactionbyid`)
/// because the receipt echoes the submitted tx id as the top-level `id`
/// field — providing response-to-request binding that the bare-by-id
/// endpoint does not. See `tx_confirmed_in_receipt` for the gating logic.
pub async fn poll_for_confirmation(
    rpc: &crate::rpc::JsonRpcClient,
    tx_id: &str,
    deadline: std::time::Duration,
) -> Result<(), PollError> {
    let body = serde_json::json!({ "value": tx_id, "visible": true });
    let poll_interval = std::time::Duration::from_secs(3);
    let start = std::time::Instant::now();
    loop {
        let resp: serde_json::Value = rpc
            .post_gettransactioninfobyid(&body)
            .await
            .map_err(|e| PollError::Rpc(e.to_string()))?;
        if tx_confirmed_in_receipt(resp, tx_id)? {
            return Ok(());
        }
        if start.elapsed() >= deadline {
            return Err(PollError::Timeout);
        }
        std::thread::sleep(poll_interval);
    }
}

#[cfg(test)]
mod broadcast_tests {
    use super::*;

    #[test]
    fn parse_broadcast_accepts_true_result() {
        let body = serde_json::json!({
            "result": true,
            "txid": "deadbeef"
        });
        let receipt = parse_broadcast_response(body).expect("accept");
        assert_eq!(receipt.result, Some(true));
        assert_eq!(receipt.txid.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn parse_broadcast_rejects_false_result_with_code_and_message() {
        let body = serde_json::json!({
            "result": false,
            "code": "SIGERROR",
            "message": "signature verification failed"
        });
        match parse_broadcast_response(body) {
            Err(BroadcastError::Rejected { code, message }) => {
                assert_eq!(code, "SIGERROR");
                assert_eq!(message, "signature verification failed");
            }
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn parse_broadcast_rejects_missing_result_flag() {
        let body = serde_json::json!({ "txid": "deadbeef" });
        assert_eq!(
            parse_broadcast_response(body),
            Err(BroadcastError::Malformed)
        );
    }

    #[test]
    fn parse_broadcast_rejects_garbage_body() {
        // Valid JSON but wrong shape (result is a string).
        let body = serde_json::json!({ "result": "true" });
        assert_eq!(
            parse_broadcast_response(body),
            Err(BroadcastError::Malformed)
        );
    }
}

#[cfg(test)]
mod poll_tests {
    use super::*;

    const TX_ID: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn poll_confirmed_when_id_matches_and_receipt_success() {
        // Live-captured shape from
        // nile.trongrid.io/wallet/gettransactioninfobyid — `id` echoes the
        // submitted tx id; `receipt.result` is `"SUCCESS"`.
        let body = serde_json::json!({
            "id": TX_ID,
            "blockNumber": 70436867,
            "blockTimeStamp": 1787828070000_u64,
            "contractResult": ["0000000000000000000000000000000000000000000000000000000000000000"],
            "receipt": { "origin_energy_usage": 29650, "result": "SUCCESS" }
        });
        assert!(
            tx_confirmed_in_receipt(body, TX_ID).unwrap(),
            "id match + receipt.result=SUCCESS must confirm"
        );
    }

    #[test]
    fn poll_not_confirmed_when_id_field_missing() {
        let body = serde_json::json!({ "blockNumber": 1, "receipt": { "result": "SUCCESS" } });
        assert!(!tx_confirmed_in_receipt(body, TX_ID).unwrap());
    }

    #[test]
    fn poll_not_confirmed_when_id_mismatch() {
        // Defense against a malicious/buggy fullnode that returns a
        // SUCCESS receipt for a different tx — the security review finding.
        let body = serde_json::json!({
            "id": "different_tx_id_0000000000000000000000000000000000000000000000000000",
            "receipt": { "result": "SUCCESS" }
        });
        assert!(
            !tx_confirmed_in_receipt(body, TX_ID).unwrap(),
            "id mismatch must NOT confirm"
        );
    }

    #[test]
    fn poll_id_match_is_case_insensitive_hex() {
        // tx ids are hex — TronGrid sometimes returns uppercase hex.
        let body = serde_json::json!({
            "id": TX_ID.to_uppercase(),
            "receipt": { "result": "SUCCESS" }
        });
        assert!(tx_confirmed_in_receipt(body, TX_ID).unwrap());
    }

    #[test]
    fn poll_not_confirmed_when_receipt_result_revert() {
        let body = serde_json::json!({
            "id": TX_ID,
            "receipt": { "result": "REVERT" }
        });
        assert!(!tx_confirmed_in_receipt(body, TX_ID).unwrap());
    }

    #[test]
    fn poll_not_confirmed_when_receipt_missing() {
        let body = serde_json::json!({ "id": TX_ID });
        assert!(!tx_confirmed_in_receipt(body, TX_ID).unwrap());
    }

    #[test]
    fn poll_rejects_garbage_body() {
        let body = serde_json::json!("not an object");
        assert_eq!(tx_confirmed_in_receipt(body, TX_ID), Err(PollError::Decode));
    }
}
