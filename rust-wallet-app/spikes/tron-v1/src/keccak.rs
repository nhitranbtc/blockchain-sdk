//! Keccak-256 wrapper. TRON/Ethereum convention: original Keccak padding (0x01),
//! NOT NIST SHA3-256 (0x06). Never use the `sha3` crate for TRON addresses — see
//! plan §B tiny-keccak rationale.

use tiny_keccak::{Hasher, Keccak};

/// Compute Keccak-256 digest of `input`.
pub fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(input);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}
