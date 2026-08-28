//! Network + chain config (Q1 — ETH + Polygon share EVM derivation).
//!
//! Carries the same `Network` enum shape that `evm-wallet-core` will adopt
//! in Phase 0 of the plan. Pre-step per plan §Phase 0.0.

use alloy_primitives::Address;

/// EVM network selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Ethereum,
    Polygon,
    PolygonAmoy,
}

/// Chain-id constants per EIP-155.
pub const ETHEREUM_MAINNET_CHAIN_ID: u64 = 1;
pub const POLYGON_MAINNET_CHAIN_ID: u64 = 137;
pub const POLYGON_AMOY_CHAIN_ID: u64 = 80_002;

/// Chain config bundle — populated from `Network` variant.
#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub network: Network,
    pub chain_id: u64,
    pub default_rpc_url: &'static str,
}

impl ChainConfig {
    pub fn for_network(network: Network) -> Self {
        match network {
            Network::Ethereum => Self {
                network,
                chain_id: ETHEREUM_MAINNET_CHAIN_ID,
                default_rpc_url: "https://eth.llamarpc.com",
            },
            Network::Polygon => Self {
                network,
                chain_id: POLYGON_MAINNET_CHAIN_ID,
                default_rpc_url: "https://polygon-rpc.com",
            },
            Network::PolygonAmoy => Self {
                network,
                chain_id: POLYGON_AMOY_CHAIN_ID,
                default_rpc_url: "https://rpc-amoy.polygon.technology",
            },
        }
    }
}

/// Trivial helper to make `address` import non-dead during Phase 1 scaffold.
pub fn _placeholder(_a: Address) {}
