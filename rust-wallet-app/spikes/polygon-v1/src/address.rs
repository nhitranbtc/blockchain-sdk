//! EVM address derivation helpers (Q1 — cross-chain identity).
//!
//! Same `alloy_primitives::Address` works on Ethereum + Polygon because both
//! use secp256k1 + keccak256(last-20-bytes(pubkey)). The cross-chain identity
//! assertion (V3) is what locks this in empirically.

use alloy_primitives::Address;

/// Placeholder for EIP-55 checksum formatter — implemented in Phase 2.
pub fn _checksum_placeholder(addr: &Address) -> String {
    format!("{addr}")
}
