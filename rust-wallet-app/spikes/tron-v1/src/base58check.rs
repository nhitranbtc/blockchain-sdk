//! base58check encode/decode — `bs58` + 4-byte double-SHA-256 checksum
//! (TRON/Ethereum/Bitcoin convention; `bs58::encode` is plain base58, no built-in check).
//! Plan §Q4.

use sha2::{Digest, Sha256};

/// Encode `payload` as a base58check string with a 4-byte double-SHA-256 checksum.
pub fn encode(payload: &[u8]) -> String {
    let cs = checksum(payload);
    let mut buf = Vec::with_capacity(payload.len() + 4);
    buf.extend_from_slice(payload);
    buf.extend_from_slice(&cs);
    bs58::encode(buf).into_string()
}

/// Decode a base58check string; verifies the trailing 4-byte checksum.
/// Returns the payload (excluding checksum) on success.
pub fn decode(s: &str) -> Result<Vec<u8>, DecodeError> {
    let raw = bs58::decode(s).into_vec().map_err(|_| DecodeError::Bs58)?;
    if raw.len() < 4 {
        return Err(DecodeError::TooShort);
    }
    let (payload, cs) = raw.split_at(raw.len() - 4);
    if cs != checksum(payload).as_slice() {
        return Err(DecodeError::ChecksumMismatch);
    }
    Ok(payload.to_vec())
}

/// 4-byte double-SHA-256 checksum (Bitcoin/TRON/Ethereum convention).
fn checksum(payload: &[u8]) -> [u8; 4] {
    let h1 = Sha256::digest(payload);
    let h2 = Sha256::digest(h1);
    let mut out = [0u8; 4];
    out.copy_from_slice(&h2[..4]);
    out
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    Bs58,
    TooShort,
    ChecksumMismatch,
}
