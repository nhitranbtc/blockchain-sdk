//! Test-only throwaway keypair generator.

use solana_sdk::{signature::Keypair, signer::Signer};

/// Generate a fresh throwaway Ed25519 keypair.
pub fn throwaway_keypair() -> Keypair {
    Keypair::new()
}

/// Convenience: derive the base58 pubkey string for log output.
pub fn pubkey_base58(keypair: &Keypair) -> String {
    keypair.pubkey().to_string()
}
