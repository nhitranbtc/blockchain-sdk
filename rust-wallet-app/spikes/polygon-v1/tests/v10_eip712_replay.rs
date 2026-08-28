//! V10 — EIP-712 cross-chain replay protection (Q7).
//!
//! Per EIP-712, the domain separator includes `chainId`. A payload signed
//! with domain `chainId = 137` (Polygon mainnet) MUST NOT verify against
//! any verifier that uses a different `chainId` — even if every other
//! field is byte-identical. This is the cross-chain replay defense.

use alloy_primitives::{Address, B256};
use alloy_signer_local::PrivateKeySigner;

use polygon_v1_spike::config::Network;
use polygon_v1_spike::config::{
    ETHEREUM_MAINNET_CHAIN_ID, POLYGON_AMOY_CHAIN_ID, POLYGON_MAINNET_CHAIN_ID,
};
use polygon_v1_spike::eip712::{build_test_signer, chain_id, domain_for_chain, typed_data_hash};

/// Placeholder verifying contract — any 20-byte address; the value is
/// fixed across the domain comparison so the only variable is chainId.
const VERIFYING_CONTRACT: Address = Address::new([0x11; 20]);

#[test]
fn v10_eip712_domain_separator_includes_chain_id() {
    let domain_polygon = domain_for_chain(POLYGON_MAINNET_CHAIN_ID, VERIFYING_CONTRACT);
    let domain_eth = domain_for_chain(ETHEREUM_MAINNET_CHAIN_ID, VERIFYING_CONTRACT);

    // The domain separator hashes MUST differ when chain_id differs.
    let sep_polygon = domain_polygon.separator();
    let sep_eth = domain_eth.separator();
    assert_ne!(
        sep_polygon, sep_eth,
        "domain separators MUST differ when chain_id differs (replay defense)"
    );

    eprintln!(
        "[V10] PASS — domain separators differ between Polygon ({}) and Ethereum ({}): polygon={:?} eth={:?}",
        POLYGON_MAINNET_CHAIN_ID,
        ETHEREUM_MAINNET_CHAIN_ID,
        sep_polygon,
        sep_eth
    );
}

#[test]
fn v10_chain_id_constants_match_eip_155() {
    // Lock in the chain-id constants the production code will use.
    assert_eq!(chain_id(Network::Ethereum), ETHEREUM_MAINNET_CHAIN_ID);
    assert_eq!(chain_id(Network::Polygon), POLYGON_MAINNET_CHAIN_ID);
    assert_eq!(chain_id(Network::PolygonAmoy), POLYGON_AMOY_CHAIN_ID);
    assert_eq!(ETHEREUM_MAINNET_CHAIN_ID, 1);
    assert_eq!(POLYGON_MAINNET_CHAIN_ID, 137);
    assert_eq!(POLYGON_AMOY_CHAIN_ID, 80_002);
}

#[test]
fn v10_test_signer_address_matches_canonical_vector() {
    // Independent sanity: the test signer (canonical "abandon ×11 + about"
    // mnemonic) MUST derive the well-known address. If the canonical
    // vector ever drifts, this test fails and surfaces the drift before
    // any cross-chain replay check goes green.
    let signer: PrivateKeySigner = build_test_signer();
    let addr = signer.address();
    let expected = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94"
        .parse::<Address>()
        .expect("known vector");
    assert_eq!(
        addr, expected,
        "test signer address must match canonical vector"
    );
}

#[test]
fn v10_typed_data_hash_is_deterministic() {
    // The hash function MUST be deterministic — same input → same output.
    let msg = [0x42_u8; 32];
    let h1: B256 = typed_data_hash(msg);
    let h2: B256 = typed_data_hash(msg);
    assert_eq!(h1, h2, "typed_data_hash must be deterministic");
    assert_ne!(
        h1,
        B256::ZERO,
        "typed_data_hash must not return zero for non-zero input"
    );
}
