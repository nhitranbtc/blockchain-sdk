//! chain-traits: cross-chain abstraction layer for the rust-wallet-app umbrella.
//!
//! Defines `ChainWallet` trait implemented by per-chain crates (bitcoin-wallet-core, etc.).
//! Umbrella itself is a thin orchestrator holding shared state (mnemonic, address book, history).
//! Per-chain crates own their own DB, signer, and RPC client.
//!
//! Reference spec: docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md

#![warn(missing_docs)]

use async_trait::async_trait;
use bitcoin::Address;
use thiserror::Error;

/// Identifier for a chain family + specific chain instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainId {
    /// Bitcoin mainnet/testnet/regtest/signet.
    Bitcoin(bitcoin::Network),
    /// Ethereum mainnet + L2s.
    Ethereum(u32), // chain_id
    /// Solana mainnet/testnet.
    Solana(SolanaCluster),
}

/// Solana cluster discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolanaCluster {
    Mainnet,
    Testnet,
    Devnet,
}

/// Per-chain wallet trait implemented by every chain crate.
///
/// Umbrella code dispatches through this trait; concrete behaviour lives in
/// per-chain crates (bitcoin-wallet-core for v0.1; ethereum-wallet-core / solana-wallet-core for v0.2+).
#[async_trait]
pub trait ChainWallet: Send + Sync {
    /// Chain this wallet operates on.
    fn chain_id(&self) -> ChainId;

    /// Synchronize chain state with the network. Idempotent.
    async fn sync(&self) -> Result<(), ChainError>;

    /// Return next receive address for the given address kind.
    async fn next_receive_address(&self) -> Result<Address, ChainError>;

    /// Current confirmed balance in the chain's base unit (satoshis, wei, lamports).
    async fn balance(&self) -> Result<u128, ChainError>;
}

/// Cross-chain error type. Each per-chain crate maps its internal errors
/// into this enum at the umbrella boundary.
#[derive(Debug, Error)]
pub enum ChainError {
    #[error("network error: {0}")]
    Network(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("signing error: {0}")]
    Sign(String),
    #[error("not initialized: {0}")]
    NotInitialized,
    #[error("chain not supported: {0}")]
    Unsupported(ChainId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_id_is_hashable() {
        let id = ChainId::Bitcoin(bitcoin::Network::Testnet);
        let mut set = std::collections::HashSet::new();
        set.insert(id);
        assert!(set.contains(&id));
    }
}
