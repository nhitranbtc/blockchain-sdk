//! Sign-only primitives — no Provider, no broadcast.
//!
//! Per Issue #302 Task 3 acceptance:
//! - `sign_native_eth_tx(signer, tx) -> TxEnvelope<alloy_consensus::TxEip1559>`
//! - `sign_erc20_tx_bytes(signer, token_addr, calldata, value, nonce, ...)`
//!   — accept pre-built calldata (Task 7 ships the calldata builders)
//! - `sign_message(signer, msg) -> Signature` (EIP-191 personal_sign)
//! - `sign_typed_data(signer, typed) -> Signature` (EIP-712 subset, v0.2)
//!
//! All signatures are deterministic for a given (mnemonic, index, payload).
//! Manual sign pattern matches Lesson 43 + spike V4 (`tests/v4_anvil_send.rs`).
//!
//! Failure mode: `SignError::Unsupported` for legacy / EIP-2930 /
//! EIP-4844 — Tasks 5+ decide whether to widen. v0.2 ships EIP-1559 (type 2)
//! writes only, per #297 G6.

use alloy_consensus::{EthereumTxEnvelope, SignableTransaction, TxEip1559};
use alloy_eips::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_primitives::{Address, Signature, TxKind, U256};
use alloy_rpc_types::TransactionRequest;
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use thiserror::Error;

/// Type alias matching the canonical alloy return shape for a signed EIP-1559
/// tx ready for `Provider::send_raw_transaction`. Equal to
/// `EthereumTxEnvelope<Signed<TxEip1559>>` — alloy default.
pub type SignedEip1559 = EthereumTxEnvelope<TxEip1559>;

/// Sign errors for Task 3. Task 4 widens to the 17-variant crate-wide enum;
/// these 4 variants are the minimum sign-only surface.
#[derive(Debug, Error)]
pub enum SignError {
    #[error("signing failed: {0}")]
    Sign(String),
    #[error("unsupported tx shape: {0}")]
    Unsupported(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("invalid request field: {0}")]
    InvalidRequest(String),
}

pub type Result<T> = std::result::Result<T, SignError>;

/// Convert a `TransactionRequest` into the canonical `TxEip1559` shape that
/// `PrivateKeySigner::sign_transaction_sync` knows how to sign.
///
/// v0.2 only — refuses any other envelope flavor with `SignError::Unsupported`.
/// Task 4 path: `From<TransactionRequest> for TxEip1559` may exist on alloy's
/// side; we use a hand-rolled projection here so callers can audit the field
/// mapping.
pub(crate) fn project_into_eip1559(tx: TransactionRequest) -> Result<TxEip1559> {
    let to = tx
        .to
        .ok_or_else(|| SignError::InvalidRequest("missing `to` address".into()))?;
    let to = match to {
        TxKind::Call(addr) => addr,
        TxKind::Create => {
            return Err(SignError::Unsupported(
                "contract creation unsupported in v0.2".into(),
            ));
        }
    };
    let chain_id = tx
        .chain_id
        .ok_or_else(|| SignError::InvalidRequest("missing chain_id".into()))?;
    let nonce = tx
        .nonce
        .ok_or_else(|| SignError::InvalidRequest("missing nonce".into()))?;
    let max_fee_per_gas = tx
        .max_fee_per_gas
        .ok_or_else(|| SignError::InvalidRequest("missing max_fee_per_gas".into()))?;
    let max_priority_fee_per_gas = tx
        .max_priority_fee_per_gas
        .ok_or_else(|| SignError::InvalidRequest("missing max_priority_fee_per_gas".into()))?;
    let gas_limit = tx
        .gas
        .ok_or_else(|| SignError::InvalidRequest("missing gas".into()))?;
    let value = tx.value.unwrap_or(U256::ZERO);
    let input = tx.input.input().cloned().unwrap_or_default();

    Ok(TxEip1559 {
        chain_id,
        nonce,
        gas_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        to: alloy_primitives::TxKind::Call(to),
        value,
        access_list: tx.access_list.unwrap_or_default(),
        input,
    })
}

/// Sign an EIP-1559 native-ETH transfer. Returns the signed envelope
/// ready for broadcast via `Provider::send_raw_transaction`.
pub fn sign_native_eth_tx(
    signer: &PrivateKeySigner,
    tx: TransactionRequest,
) -> Result<SignedEip1559> {
    let mut tx_eip1559 = project_into_eip1559(tx)?;
    let sig = signer
        .sign_transaction_sync(&mut tx_eip1559)
        .map_err(|e| SignError::Sign(format!("native: {e}")))?;
    Ok(EthereumTxEnvelope::Eip1559(tx_eip1559.into_signed(sig)))
}

/// Sign an ERC-20 transfer (or any contract call) given a pre-built calldata
/// payload. The caller provides all tx params explicitly; calldata builders
/// ship in Task 7 (`sol! transfer(address,uint256)`).
///
/// Note: the resulting `to` field IS the token contract (NOT the recipient).
/// The recipient lives inside the calldata; this is the canonical ERC-20
/// transfer shape (tx.value = 0; recipient encoded in calldata).
#[allow(clippy::too_many_arguments)]
pub fn sign_erc20_tx_bytes(
    signer: &PrivateKeySigner,
    token_contract: Address,
    calldata: alloy_primitives::Bytes,
    amount_wei: U256, // ETH value sent with the tx (= 0 for ERC-20 transfers)
    nonce: u64,
    chain_id: u64,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
    gas_limit: u64,
) -> Result<SignedEip1559> {
    let mut tx_eip1559 = TxEip1559 {
        chain_id,
        nonce,
        gas_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        to: alloy_primitives::TxKind::Call(token_contract),
        value: amount_wei,
        access_list: Default::default(),
        input: calldata,
    };
    let sig = signer
        .sign_transaction_sync(&mut tx_eip1559)
        .map_err(|e| SignError::Sign(format!("erc20: {e}")))?;
    Ok(EthereumTxEnvelope::Eip1559(tx_eip1559.into_signed(sig)))
}

/// EIP-191 `personal_sign` over the raw `msg` bytes. The alloy signer
/// prepends the `"\x19Ethereum Signed Message:\n" + len(varint)` prefix
/// internally — do NOT pre-prefix the bytes here.
pub fn sign_message(signer: &PrivateKeySigner, msg: &[u8]) -> Result<Signature> {
    signer
        .sign_message_sync(msg)
        .map_err(|e| SignError::Sign(format!("eip191: {e}")))
}

/// EIP-712 typed-data sign. v0.2 subset per #297 D1: single-domain
/// payloads only (no nested structs). Returns 65-byte `r||s||v` signature.
///
/// **Deferred — follow-up issue needed.** alloy 1.8.x's `TypedData` struct
/// sits behind a feature flag (`alloy_primitives/eip712` or `alloy_signer/eip712`)
/// not enabled in this crate's minimal feature set. v0.2 ships EIP-191
/// (`sign_message`) only; full EIP-712 lands in a follow-up PR once we
/// pick the right cargo feature gate. The function signature stays put
/// so callers don't need to track the deferral.
pub fn sign_typed_data(
    _signer: &PrivateKeySigner,
    _typed: &[u8], // opaque blob — final type once feature is picked
) -> Result<Signature> {
    Err(SignError::Unsupported(
        "EIP-712 sign deferred — needs alloy eip712 feature gate; tracked in follow-up issue"
            .into(),
    ))
}

/// Helper: encode a signed EIP-1559 envelope for the wire (used by Tasks 9
/// / 10 to broadcast via `Provider::send_raw_transaction`).
///
/// Returns the canonical 2718-encoded `Vec<u8>` bytes (RLP-equivalent).
pub fn encoded_envelope(envelope: &SignedEip1559) -> Vec<u8> {
    envelope.encoded_2718()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signer() -> PrivateKeySigner {
        let phrase =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        alloy_signer_local::MnemonicBuilder::english()
            .phrase(phrase)
            .index(0)
            .expect("valid index")
            .build()
            .expect("build")
    }

    fn test_request() -> TransactionRequest {
        TransactionRequest {
            to: Some(TxKind::Call(
                "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
                    .parse()
                    .unwrap(),
            )),
            value: Some(U256::from(1_000_000_000_000_000u128)), // 0.001 ETH
            chain_id: Some(11155111),
            nonce: Some(7),
            gas: Some(21_000),
            max_fee_per_gas: Some(10_000_000_000),
            max_priority_fee_per_gas: Some(1_000_000_000),
            ..Default::default()
        }
    }

    #[test]
    fn sign_native_is_deterministic_for_same_inputs() {
        let s = test_signer();
        let env1 = sign_native_eth_tx(&s, test_request()).expect("env1");
        let env2 = sign_native_eth_tx(&s, test_request()).expect("env2");
        assert_eq!(
            env1, env2,
            "EIP-1559 must produce identical envelopes (deterministic signing)"
        );
    }

    #[test]
    fn encoded_envelope_round_trip_decodes_2718() {
        let s = test_signer();
        let env = sign_native_eth_tx(&s, test_request()).expect("env");
        let bytes = encoded_envelope(&env);
        assert!(!bytes.is_empty());
        assert!(
            bytes[0] == 0x02,
            "EIP-1559 envelope must start with type-2 (0x02) prefix"
        );
    }

    #[test]
    fn sign_message_is_deterministic() {
        let s = test_signer();
        let msg = b"hello, world";
        let sig1 = sign_message(&s, msg).expect("sig1");
        let sig2 = sign_message(&s, msg).expect("sig2");
        assert_eq!(
            sig1, sig2,
            "EIP-191 sign must be deterministic for given (key, message)"
        );
        assert_eq!(
            sig1.as_bytes().len(),
            65,
            "signature must be 65 bytes r||s||v"
        );
    }

    #[test]
    fn missing_required_field_yields_invalid_request() {
        let s = test_signer();
        let mut req = test_request();
        req.max_fee_per_gas = None;
        let err = sign_native_eth_tx(&s, req).expect_err("missing max_fee must reject");
        assert!(matches!(err, SignError::InvalidRequest(_)), "got: {err:?}");
    }

    #[test]
    fn sign_erc20_tx_bytes_produces_distinct_envelope_from_native() {
        let s = test_signer();
        let calldata = alloy_primitives::Bytes::from_static(&[0xa9, 0x05, 0x9c, 0xbb]);
        let erc20 = sign_erc20_tx_bytes(
            &s,
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                .parse()
                .unwrap(),
            calldata,
            U256::ZERO,
            7,
            11155111,
            10_000_000_000,
            1_000_000_000,
            65_000,
        )
        .expect("erc20");

        let native = sign_native_eth_tx(&s, test_request()).expect("native");
        assert_ne!(
            erc20, native,
            "ERC-20 envelope must differ from native envelope (different `to` + `input`)"
        );
    }
}
