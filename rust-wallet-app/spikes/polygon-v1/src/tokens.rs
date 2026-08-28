//! Token registry loader (Q3 ERC-20 surface).
//!
//! Reads bundled `tokens/{mainnet,amoy}.json` + on-chain `decimals()` verify
//! in V6. The Polygon ERC-20 footgun: USDC (`0x3c499c542cef5e3811e1192ce70d8cc03d5c3359`,
//! native, 6 decimals) vs USDC.e (`0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`, bridged,
//! 6 decimals) — different contracts, same symbol. Registry pins the canonical
//! native USDC; production code MUST surface a `native_usdc_only` flag.

use serde::Deserialize;

/// One row in a `tokens/*.json` registry file.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenEntry {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
}

/// Placeholder for the registry loader — implemented in Phase 2.
pub fn _registry_placeholder() -> &'static str {
    "tokens/{mainnet,amoy}.json"
}
