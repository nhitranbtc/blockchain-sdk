//! `polygon` CLI handlers — Issue #426 / Phase 4 of #416.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.3 + §6.2. Batch B (TDD): `parse_network` (this file); remaining
//! handlers land in subsequent batches.

use polygon_wallet_core::{Error, Network};

// Re-export submodules so callers (main.rs) can write `use handlers::*`
// instead of reaching into individual files. Mirrors design doc §5.3.
pub mod erc20;
pub mod sign;

/// Parse `--network` at the polygon CLI boundary. Default = amoy.
///
/// Delegates to `PolygonChain::parse_cli` (already rejects "anvil" +
/// "mumbai" + unknown networks with `Error::InvalidInput` per
/// `evm-wallet-core/src/network.rs:228-237`). The wrapper is here so
/// the CLI can later surface a friendlier drift-#2 message without
/// touching the lib.
pub fn parse_network(s: &str) -> polygon_wallet_core::Result<Network> {
    Ok(Network::Polygon(
        polygon_wallet_core::PolygonChain::parse_cli(s)?,
    ))
}

#[cfg(test)]
mod tests {
    //! Batch B tests (per design doc §6.2): parse_network rejects anvil +
    //! mumbai + unknown; accepts amoy + mainnet.
    use super::parse_network;
    use polygon_wallet_core::{Error, Network, PolygonChain};

    /// Batch B test #1 (failing seed per design doc §6.2): amoy → Ok(Polygon(Amoy)).
    #[test]
    fn parse_network_amoy_returns_polygon_amoy() {
        let net = parse_network("amoy").expect("amoy parses");
        assert!(matches!(net, Network::Polygon(PolygonChain::Amoy)));
    }

    /// Batch B test #2: mainnet → Ok(Polygon(Mainnet)).
    #[test]
    fn parse_network_mainnet_returns_polygon_mainnet() {
        let net = parse_network("mainnet").expect("mainnet parses");
        assert!(matches!(net, Network::Polygon(PolygonChain::Mainnet)));
    }

    /// Batch B test #3 (Drift #2 explicit test): "anvil" rejected.
    #[test]
    fn parse_network_anvil_returns_invalid_input() {
        let r = parse_network("anvil");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "anvil must be rejected (cross-chain identity footgun); got {r:?}"
        );
    }

    /// Batch B test #4: "mumbai" rejected (deprecation 2024-Q2).
    #[test]
    fn parse_network_mumbai_returns_invalid_input() {
        let r = parse_network("mumbai");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "mumbai must be rejected (deprecation 2024-Q2); got {r:?}"
        );
    }

    /// Batch B test #5: unknown network rejected.
    #[test]
    fn parse_network_unknown_returns_invalid_input() {
        let r = parse_network("fakenet");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "unknown network must be rejected; got {r:?}"
        );
    }
}
