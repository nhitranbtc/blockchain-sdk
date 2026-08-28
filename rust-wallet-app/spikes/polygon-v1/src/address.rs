//! EVM address derivation (Q1 — cross-chain identity).
//!
//! Same BIP-39 mnemonic → same EVM address on Ethereum + Polygon because
//! both chains use secp256k1 + keccak256(last-20-bytes(pubkey)). The
//! `derive_evm_address` helper takes a mnemonic + a `Network` variant and
//! returns the canonical address at `m/44'/60'/0'/0/0` (SLIP-44 coin type
//! 60 covers both chains per EVM convention).

use alloy_primitives::Address;
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner};

use crate::config::Network;

/// BIP-44 derivation path for EVM accounts (m/44'/60'/0'/0/0).
/// Same for Ethereum + Polygon + any EVM chain under SLIP-44 coin type 60.
const EVM_DERIVATION_INDEX: u32 = 0;

/// Build a `PrivateKeySigner` from a BIP-39 mnemonic at the canonical
/// EVM derivation path. The `Network` argument is accepted to keep the
/// V3 cross-chain-identity assertion ergonomic — the derivation is
/// network-agnostic (always coin type 60 + index 0), but the API
/// signature mirrors what `evm-wallet-core` will expose in Phase 0.
pub fn build_signer(mnemonic: &str, _network: Network) -> Result<PrivateKeySigner, DeriveError> {
    MnemonicBuilder::english()
        .phrase(mnemonic)
        .index(EVM_DERIVATION_INDEX)
        .expect("hard-coded index 0 is valid")
        .build()
        .map_err(|e| DeriveError(e.to_string()))
}

/// Derive the EVM address from a mnemonic. Convenience wrapper around
/// `build_signer` that returns just the address — what V3 asserts.
pub fn derive_evm_address(mnemonic: &str, network: Network) -> Result<Address, DeriveError> {
    Ok(build_signer(mnemonic, network)?.address())
}

#[derive(Debug)]
pub struct DeriveError(pub String);

impl std::fmt::Display for DeriveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "derive error: {}", self.0)
    }
}

impl std::error::Error for DeriveError {}
