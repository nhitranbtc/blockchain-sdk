//! Nile testnet network config — single source of truth (Q6/Q7/Q9).
//!
//! Reads `tokens/nile.json` at compile-time via `include_str!` so the spike
//! stays offline-deterministic. Production replacement: `Network` enum with
//! `{Nile, Shasta, Mainnet}` + `--network` CLI flag (plan §Phase 4).

use serde::Deserialize;

const NILE_CONFIG_RAW: &str = include_str!("../tokens/nile.json");

#[derive(Debug, Deserialize, Clone)]
pub struct NileConfig {
    pub chain_id_hex: String,
    pub chain_id_dec: u64,
    pub rpc_url: String,
    pub faucet_url: String,
    pub explorer_tx_url: String,
    pub spki_pin_env: String,
    pub tokens: Vec<TokenMeta>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TokenMeta {
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub decimals: u8,
    pub issuer: String,
}

impl NileConfig {
    /// Scheme-stripped host portion of `rpc_url`. Use to build
    /// `pinned://<pin>@<host>:443` URLs.
    pub fn rpc_host(&self) -> &str {
        self.rpc_url
            .strip_prefix("https://")
            .or_else(|| self.rpc_url.strip_prefix("http://"))
            .unwrap_or(&self.rpc_url)
    }

    /// Find a token entry by symbol. `None` if not registered.
    pub fn token(&self, symbol: &str) -> Option<&TokenMeta> {
        self.tokens.iter().find(|t| t.symbol == symbol)
    }
}

/// Load the bundled Nile testnet config. Panics on schema mismatch — the
/// `tokens/nile.json` file is the source of truth and is exercised by tests.
pub fn nile_config() -> NileConfig {
    serde_json::from_str(NILE_CONFIG_RAW).expect("tokens/nile.json parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_nile_config() {
        let c = nile_config();
        assert_eq!(c.chain_id_hex, "0xcd8690dc");
        assert_eq!(c.chain_id_dec, 3448148188);
        assert_eq!(c.rpc_url, "https://nile.trongrid.io");
        assert_eq!(c.rpc_host(), "nile.trongrid.io");
        assert_eq!(c.spki_pin_env, "TRON_NILE_SPKI_PIN");
        assert!(!c.tokens.is_empty(), "nile.json must ship ≥1 token");
    }

    #[test]
    fn rpc_host_strips_https_scheme() {
        let c = nile_config();
        assert!(!c.rpc_host().starts_with("https://"));
        assert!(!c.rpc_host().starts_with("http://"));
    }
}
