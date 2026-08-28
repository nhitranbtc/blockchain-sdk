//! RPC provider wiring (Q4 connectivity + Q5 fee-estimation cadence).
//!
//! Wraps `alloy_provider::ProviderBuilder` + `alloy_transport_http::Http` to
//! give a typed builder for both Polygon mainnet + Amoy testnet.

use crate::config::ChainConfig;

/// Placeholder for the typed provider builder — implemented in Phase 2.
pub fn _rpc_url_placeholder(cfg: &ChainConfig) -> &str {
    cfg.default_rpc_url
}
