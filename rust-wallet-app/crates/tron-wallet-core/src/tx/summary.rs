//! Human-readable transfer log.
//!
//! Pure data; the format layer that stringifies this lives in the CLI crate,
//! not here. This module exists so `tx::sign::sign_tx` and friends have a
//! shape to hand back that callers can `println!` or ship over the FFI
//! boundary without each side picking their own field order.

use serde::Serialize;

/// A single receipt summary suitable for either stdout or JSON serialisation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TxSummary {
    /// Hex-encoded txid (the *true* id, derived via double SHA-256 over the
    /// raw wire bytes — see [`crate::tx::sign::txid`]).
    pub txid: String,

    /// Hex-encoded raw-data portion of the signed transaction (the bytes
    /// TRON expects alongside the signature at `wallet/broadcasttransaction`).
    pub raw_data_hex: String,

    /// Hex-encoded 65-byte signature (`r ‖ s ‖ v`, with `v ∈ {0, 1}`).
    pub signature_hex: String,
}
