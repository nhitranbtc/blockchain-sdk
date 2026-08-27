//! TRON address derivation + display encoding.
//!
//! Plan §Q4: prefix byte `0x41` universal across mainnet/Shasta/Nile.
//! Internal API calls use 21-byte raw form (`[0x41] ++ last 20 bytes of keccak256(pubkey)`);
//! user-facing display = T-base58check (34 chars, starts with `T`).

use crate::base58check;
use crate::keccak::keccak256;

/// Universal TRON address prefix byte (Q4 — same for all networks).
pub const PREFIX_MAINNET: u8 = 0x41;

/// Derive the 21-byte raw TRON address from an uncompressed secp256k1 public key
/// (65 bytes, leading `0x04`). Returns `[0x41] ++ keccak256(pubkey)[12..32]`.
pub fn raw_21_from_uncompressed_pubkey(pubkey_uncompressed: &[u8; 65]) -> [u8; 21] {
    assert_eq!(
        pubkey_uncompressed[0], 0x04,
        "expected uncompressed secp256k1 pubkey (leading 0x04)"
    );
    let h = keccak256(&pubkey_uncompressed[1..]);
    let mut out = [0u8; 21];
    out[0] = PREFIX_MAINNET;
    out[1..].copy_from_slice(&h[12..]);
    out
}

/// Encode a 21-byte raw address as a T-base58check string (34 chars, starts with `T`).
pub fn to_base58check(raw_21: &[u8; 21]) -> String {
    base58check::encode(raw_21)
}

/// Decode a T-base58check string back to its 21-byte raw form. Validates the
/// 21-byte length and `0x41` prefix.
pub fn from_base58check(s: &str) -> Result<[u8; 21], AddressError> {
    let raw = base58check::decode(s).map_err(|_| AddressError::InvalidChecksum)?;
    if raw.len() != 21 {
        return Err(AddressError::WrongLength);
    }
    if raw[0] != PREFIX_MAINNET {
        return Err(AddressError::WrongPrefix);
    }
    let mut out = [0u8; 21];
    out.copy_from_slice(&raw);
    Ok(out)
}

#[derive(Debug, PartialEq, Eq)]
pub enum AddressError {
    InvalidChecksum,
    WrongLength,
    WrongPrefix,
}
