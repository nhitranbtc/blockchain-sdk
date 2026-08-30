//! Phase 0 chain-id sanity tests for the new `evm_wallet_core::network::Network`.
//!
//! Mirrors the polygon-v1 spike V1 + V2 acceptance (Issue #417 / #420):
//!   * Each chain family returns the expected EIP-155 chain_id.
//!   * Default RPC URLs match plan Phase 0 + Phase 2 defaults.
//!   * Gas-token labels match Q8 of the polygon plan.
//!   * Family-level `parse_cli` round-trips both Ethereum + Polygon.
//!   * ETH-only `parse_cli_eth` rejects Polygon inputs (eth CLI invariant).
//!
//! All assertions are pure-Rust, no live RPC — runs offline.

use evm_wallet_core::network::{EthereumChain, Network, PolygonChain};

#[test]
fn ethereum_chain_id_returns_eip_155_value() {
    assert_eq!(EthereumChain::Mainnet.chain_id(), 1);
    assert_eq!(EthereumChain::Sepolia.chain_id(), 11_155_111);
    assert_eq!(EthereumChain::Anvil.chain_id(), 31_337);
}

#[test]
fn polygon_chain_id_returns_eip_155_value() {
    assert_eq!(PolygonChain::Mainnet.chain_id(), 137);
    assert_eq!(PolygonChain::Amoy.chain_id(), 80_002);
}

#[test]
fn family_level_chain_id_dispatches_to_inner() {
    assert_eq!(Network::Ethereum(EthereumChain::Mainnet).chain_id(), 1);
    assert_eq!(Network::Polygon(PolygonChain::Mainnet).chain_id(), 137);
    assert_eq!(Network::Polygon(PolygonChain::Amoy).chain_id(), 80_002);
}

#[test]
fn family_level_rpc_url_matches_plan_defaults() {
    // Plan Phase 0 + Phase 2 defaults (Issue #474, 2025-Q4 update):
    //   ETH Mainnet   → ethereum-rpc.publicnode.com (was cloudflare-eth.com)
    //   Polygon Mnet  → polygon-bor-rpc.publicnode.com (was polygon-rpc.com)
    //   Polygon Amoy  → polygon-amoy-bor-rpc.publicnode.com (was polygon-amoy.drpc.org)
    assert_eq!(
        Network::Ethereum(EthereumChain::Mainnet).rpc_url(),
        "https://ethereum-rpc.publicnode.com"
    );
    assert_eq!(
        Network::Polygon(PolygonChain::Mainnet).rpc_url(),
        "https://polygon-bor-rpc.publicnode.com"
    );
    assert_eq!(
        Network::Polygon(PolygonChain::Amoy).rpc_url(),
        "https://polygon-amoy-bor-rpc.publicnode.com"
    );
}

#[test]
fn gas_token_label_per_family() {
    assert_eq!(
        Network::Ethereum(EthereumChain::Mainnet).gas_token_label(),
        "ETH"
    );
    assert_eq!(
        Network::Polygon(PolygonChain::Mainnet).gas_token_label(),
        "POL"
    );
    assert_eq!(
        Network::Polygon(PolygonChain::Amoy).gas_token_label(),
        "POL"
    );
}

#[test]
fn legacy_gas_token_label_for_polygon_only() {
    // Polygon MATIC → POL rebrand 2024-09-04. ETH has no legacy alias.
    assert_eq!(
        Network::Polygon(PolygonChain::Mainnet).legacy_gas_token_label(),
        Some("MATIC")
    );
    assert_eq!(
        Network::Ethereum(EthereumChain::Mainnet).legacy_gas_token_label(),
        None
    );
}

#[test]
fn from_chain_id_resolves_all_five_instances() {
    assert_eq!(
        Network::from_chain_id(1),
        Some(Network::Ethereum(EthereumChain::Mainnet))
    );
    assert_eq!(
        Network::from_chain_id(11_155_111),
        Some(Network::Ethereum(EthereumChain::Sepolia))
    );
    assert_eq!(
        Network::from_chain_id(31_337),
        Some(Network::Ethereum(EthereumChain::Anvil))
    );
    assert_eq!(
        Network::from_chain_id(137),
        Some(Network::Polygon(PolygonChain::Mainnet))
    );
    assert_eq!(
        Network::from_chain_id(80_002),
        Some(Network::Polygon(PolygonChain::Amoy))
    );
    assert_eq!(Network::from_chain_id(999_999), None);
}

#[test]
fn family_level_parse_cli_accepts_all_families() {
    assert_eq!(
        Network::parse_cli("mainnet").unwrap(),
        Network::Ethereum(EthereumChain::Mainnet)
    );
    assert_eq!(
        Network::parse_cli("sepolia").unwrap(),
        Network::Ethereum(EthereumChain::Sepolia)
    );
    assert_eq!(
        Network::parse_cli("anvil").unwrap(),
        Network::Ethereum(EthereumChain::Anvil)
    );
    assert_eq!(
        Network::parse_cli("polygon").unwrap(),
        Network::Polygon(PolygonChain::Mainnet)
    );
    assert_eq!(
        Network::parse_cli("polygon-amoy").unwrap(),
        Network::Polygon(PolygonChain::Amoy)
    );
    assert_eq!(
        Network::parse_cli("137").unwrap(),
        Network::Polygon(PolygonChain::Mainnet)
    );
    assert_eq!(
        Network::parse_cli("80002").unwrap(),
        Network::Polygon(PolygonChain::Amoy)
    );
}

#[test]
fn family_level_parse_cli_rejects_unknown_and_deprecated_mumbai() {
    assert!(Network::parse_cli("bitcoin").is_err());
    // Mumbai (80001) was deprecated 2024-Q2 per Phase 0.0.a drift correction.
    assert!(Network::parse_cli("mumbai").is_err());
    assert!(Network::parse_cli("80001").is_err());
}

#[test]
fn ethereum_chain_parser_rejects_polygon_inputs() {
    // ETH CLI invariant — must reject Polygon-side chain ids even though
    // the family-level parser accepts them.
    assert!(EthereumChain::parse_cli("polygon").is_err());
    assert!(EthereumChain::parse_cli("polygon-amoy").is_err());
    assert!(EthereumChain::parse_cli("137").is_err());
    assert!(EthereumChain::parse_cli("80002").is_err());

    // ETH CLI happy-path aliases still work.
    assert_eq!(
        EthereumChain::parse_cli("mainnet").unwrap(),
        EthereumChain::Mainnet
    );
    assert_eq!(
        EthereumChain::parse_cli("sepolia").unwrap(),
        EthereumChain::Sepolia
    );
    assert_eq!(
        EthereumChain::parse_cli("anvil").unwrap(),
        EthereumChain::Anvil
    );
}

#[test]
fn parse_cli_eth_helper_wraps_into_network_family() {
    // CLI entry-point convenience: ETH-only parse_cli wrapped into
    // Network::Ethereum(...) so downstream code can stay family-typed.
    assert_eq!(
        Network::parse_cli_eth("mainnet").unwrap(),
        Network::Ethereum(EthereumChain::Mainnet)
    );
    assert!(Network::parse_cli_eth("polygon").is_err());
}

#[test]
fn directory_names_match_eth_v0_2_layout() {
    // Migration: pre-Phase-0 disk layout used `<base>/<network>/<wallet_id>.enc`.
    // ETH instances preserve those names exactly; Polygon uses new prefixes.
    assert_eq!(EthereumChain::Mainnet.as_dir_name(), "mainnet");
    assert_eq!(EthereumChain::Sepolia.as_dir_name(), "sepolia");
    assert_eq!(EthereumChain::Anvil.as_dir_name(), "anvil");
    assert_eq!(PolygonChain::Mainnet.as_dir_name(), "polygon_mainnet");
    assert_eq!(PolygonChain::Amoy.as_dir_name(), "polygon_amoy");
}

#[test]
fn default_v0_2_back_compat_returns_sepolia() {
    // v0.2 placeholders expected Sepolia. Phase 0 commits to that default.
    assert_eq!(
        Network::default_v0_2(),
        Network::Ethereum(EthereumChain::Sepolia)
    );
}
