//! Protobuf transaction encode/decode helpers (Q2).
//!
//! Plan §Q2: TRON transaction signing = `txID = SHA-256(protobuf-serialize(raw_data))`
//! then k256 ECDSA over that hash → 65-byte signature = `r‖s‖v` with `v ∈ {0, 1}`.
//! NOT Ethereum convention (`v + 27 ∈ {27, 28}`).

use prost::Message;
use sha2::Digest;

/// Re-export the generated TRON protobuf types so spike tests do not need to
/// know the exact codegen module path.
pub use crate::proto;

/// txID = SHA-256(protobuf-serialize(raw_data)). Per plan §Q2.
pub fn tx_id(raw_data: &impl Message) -> [u8; 32] {
    let bytes = raw_data.encode_to_vec();
    sha2::Sha256::digest(bytes).into()
}
