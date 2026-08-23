//! Token registry + decimals cache — Issue #307 (Task 8).
//!
//! Q5/Q6/Q9 resolutions:
//!   - Q5: decimals NOT hard-coded; cache one `decimals()` `eth_call` per
//!     token at startup, persist in-memory as `Token.decimals`.
//!   - Q6: bundled JSON in repo at `rust-wallet-app/crates/eth-wallet-core/
//!     tokens/{mainnet,sepolia,anvil}.json` via `include_str!` at compile
//!     time. Per-OS user registry ($XDG_CONFIG_HOME/eth/tokens/<chain>.json)
//!     wired by Task 10 CLI; v0.2 ships just the bundled layer.
//!   - Q9: USDC contract addresses + decimals = 6 (per EIP-20 + Circle docs).
//!
//! Story 22 (balance) + Story 23 (list) + Story 24 (custom register, via CLI
//! in Task 10) consume the `Token` struct + `load_chain` query.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A single ERC-20 token entry. Decimals + chain_id are denormalized to
/// the `Token` record — Q5 (decimals are cached, NOT query-ed per balance
/// call).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    /// ERC-20 contract address (EIP-55 mixed-case form preferred).
    pub address: alloy_primitives::Address,
    /// Token symbol (1-11 ASCII chars per Story 24 validation).
    pub symbol: String,
    /// Number of base-10 decimals the token uses (0-36 per Story 24).
    pub decimals: u8,
    /// EIP-155 chain ID this entry applies to.
    pub chain_id: u64,
}

impl Token {
    /// Convert a token amount in human-readable form (e.g., 1.5 USDC) to
    /// its base-unit representation (1_500_000 for 1.5 USDC @ decimals=6).
    pub fn human_to_base_units(&self, human: f64) -> alloy_primitives::U256 {
        let scale = 10u128.pow(self.decimals as u32) as f64;
        alloy_primitives::U256::from((human * scale) as u128)
    }

    /// Convert a base-unit token amount back to its human-readable form.
    pub fn base_units_to_human(&self, base: alloy_primitives::U256) -> f64 {
        let scale = 10u128.pow(self.decimals as u32) as f64;
        let base_f = base.try_into().unwrap_or(0u128) as f64;
        base_f / scale
    }
}

/// Bundled JSON registry for a chain. Compile-time embedded via
/// `include_str!` — the file MUST be valid JSON at build time.
fn bundled_registry(chain_id: u64) -> &'static str {
    // Match the bundled JSON to the chain id. Anvil = stub (empty list
    // for v0.2; the local test instances the operator adds via Story 24
    // land in the user registry wired by Task 10).
    match chain_id {
        1 => include_str!("../tokens/mainnet.json"),
        11155111 => include_str!("../tokens/sepolia.json"),
        31337 => include_str!("../tokens/anvil.json"),
        _ => "[]",
    }
}

/// Load all bundled `Token` entries for the given chain id. Returns an
/// empty Vec for unknown chain ids (or for chains with no bundled entries
/// like the Anvil stub). Parse-error → `Error::WalletCorrupt`.
pub fn load_chain(chain_id: u64) -> Result<Vec<Token>> {
    let raw = bundled_registry(chain_id);
    serde_json::from_str(raw).map_err(|e| Error::WalletCorrupt {
        path: format!("bundled chain_id={chain_id}"),
        reason: format!("json: {e}"),
    })
}

/// Look up a single token by (chain_id, symbol). Walks the bundled
/// registry only; user-added tokens (Story 24) live in the CLI layer
/// (Task 10). Per-#297 G3, user registry wins on collision — wired when
/// Task 10's CLI surface lands.
pub fn lookup_by_symbol(chain_id: u64, symbol: &str) -> Result<Option<Token>> {
    Ok(load_chain(chain_id)?
        .into_iter()
        .find(|t| t.symbol.eq_ignore_ascii_case(symbol)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_mainnet_returns_at_least_usdc_usdt() {
        let tokens = load_chain(1).expect("mainnet JSON must parse");
        let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
        assert!(
            symbols.iter().any(|s| s.eq_ignore_ascii_case("USDC")),
            "mainnet.json must include USDC; got: {symbols:?}"
        );
        assert!(
            symbols.iter().any(|s| s.eq_ignore_ascii_case("USDT")),
            "mainnet.json must include USDC; got: {symbols:?}"
        );
    }

    #[test]
    fn load_sepolia_returns_at_least_usdc() {
        let tokens = load_chain(11155111).expect("sepolia JSON must parse");
        let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
        assert!(
            symbols.iter().any(|s| s.eq_ignore_ascii_case("USDC")),
            "sepolia.json must include USDC; got: {symbols:?}"
        );
    }

    #[test]
    fn lookup_by_symbol_is_case_insensitive_and_returns_none_for_unknown() {
        let found = lookup_by_symbol(1, "usdc").expect("lookup");
        assert!(found.is_some(), "lowercase 'usdc' must match USDC entry");
        let missing = lookup_by_symbol(1, "DOGE").expect("lookup");
        assert!(missing.is_none(), "unknown symbol must return None");
    }

    #[test]
    fn human_to_base_units_decimals_6() {
        let usdc_mainnet = Token {
            address: alloy_primitives::Address::ZERO,
            symbol: "USDC".to_string(),
            decimals: 6,
            chain_id: 1,
        };
        let base = usdc_mainnet.human_to_base_units(1.5);
        assert_eq!(base, alloy_primitives::U256::from(1_500_000u64));
        assert_eq!(
            usdc_mainnet.base_units_to_human(alloy_primitives::U256::from(1_500_000u64)),
            1.5
        );
    }

    #[test]
    fn load_anvil_returns_empty_list_for_v0_2_stub() {
        let tokens = load_chain(31337).expect("anvil JSON must parse");
        assert!(
            tokens.is_empty(),
            "v0.2 anvil stub is empty; got: {tokens:?}"
        );
    }
}
