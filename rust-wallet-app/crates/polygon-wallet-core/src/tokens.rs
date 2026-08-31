//! Polygon token registry — Phase 3 Task 4 of #425.
//!
//! Bundled JSON at `polygon-wallet-core/tokens/{mainnet,amoy}.json`,
//! compile-time embedded via `include_str!` per Q6 of the polygon plan
//! (mirrors how `evm-wallet-core::tokens` loads its chain registries).
//!
//! **API shape amendment (vs issue #425 body)**: the issue body
//! specified `tokens.rs::load(Network::Polygon, "mainnet")`. The
//! shipped surface uses two chain-named loaders (`load_mainnet()` +
//! `load_amoy()`) instead — Polygon-only, no `Network` discriminator.
//! Rationale: L12 type-design review flagged that re-exporting the
//! generic `lookup_by_address(chain_id, …)` /
//! `lookup_by_symbol(chain_id, …)` helpers from `evm-wallet-core`
//! would let a Polygon caller pass `chain_id = 1` (ETH mainnet) and
//! receive the Ethereum USDC — undermining the "Polygon-typed API
//! surface" premise of the thin wrapper. The chain-named loaders
//! eliminate that footgun by construction.
//!
//! Per Q5: decimals are baked into the bundled JSON, not query-ed per
//! balance call. V6 verifies USDC = 6, DAI = 18.

use evm_wallet_core::tokens::Token;
use evm_wallet_core::{Error, Result};

/// Bundled Polygon mainnet registry. Compile-time embedded — the
/// JSON file MUST be valid at build time or this module fails to
/// compile.
const MAINNET_JSON: &str = include_str!("../tokens/mainnet.json");

/// Bundled Polygon Amoy testnet registry.
const AMOY_JSON: &str = include_str!("../tokens/amoy.json");

/// Load all bundled `Token` entries for Polygon mainnet (chain_id
/// 137). Parse-error → `Error::WalletCorrupt`.
pub fn load_mainnet() -> Result<Vec<Token>> {
    serde_json::from_str(MAINNET_JSON).map_err(|e| Error::WalletCorrupt {
        path: "polygon-wallet-core/tokens/mainnet.json".to_string(),
        reason: format!("json: {e}"),
    })
}

/// Load all bundled `Token` entries for Polygon Amoy testnet
/// (chain_id 80002). Parse-error → `Error::WalletCorrupt`.
pub fn load_amoy() -> Result<Vec<Token>> {
    serde_json::from_str(AMOY_JSON).map_err(|e| Error::WalletCorrupt {
        path: "polygon-wallet-core/tokens/amoy.json".to_string(),
        reason: format!("json: {e}"),
    })
}
