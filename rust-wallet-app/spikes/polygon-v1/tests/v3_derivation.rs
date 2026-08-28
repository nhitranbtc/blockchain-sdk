//! V3 — cross-chain derivation identity (Q1).
//!
//! Same BIP-39 mnemonic → same EVM address on Ethereum + Polygon + Amoy.
//! Asserts the EIP-reuse thesis: same secp256k1 + keccak256(last-20-bytes(pubkey))
//! for every EVM chain. Always runs (deterministic, no I/O).

use polygon_v1_spike::address::derive_evm_address;
use polygon_v1_spike::config::Network;

/// Canonical BIP-39 test mnemonic (12-word, English) — derives a known
/// Ethereum address at m/44'/60'/0'/0/0 across every SLIP-44-60 wallet.
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[test]
fn v3_canonical_mnemonic_derives_same_evm_address_on_all_networks() {
    let addr_eth =
        derive_evm_address(TEST_MNEMONIC, Network::Ethereum).expect("valid mnemonic on Ethereum");
    let addr_polygon =
        derive_evm_address(TEST_MNEMONIC, Network::Polygon).expect("valid mnemonic on Polygon");
    let addr_amoy =
        derive_evm_address(TEST_MNEMONIC, Network::PolygonAmoy).expect("valid mnemonic on Amoy");

    assert_eq!(
        addr_eth, addr_polygon,
        "cross-chain ETH ↔ Polygon identity (Q1)"
    );
    assert_eq!(
        addr_eth, addr_amoy,
        "cross-chain ETH ↔ PolygonAmoy identity (Q1)"
    );

    // EVM addresses are 20 bytes.
    assert_eq!(addr_eth.as_slice().len(), 20);

    eprintln!(
        "[V3] PASS — derived address {addr_eth} is identical across ETH + Polygon + PolygonAmoy"
    );
}

#[test]
fn v3_derivation_matches_canonical_test_vector() {
    // Independently verified against MetaMask / Ledger Live / MyEtherWallet /
    // ethers-rs / alloy reference output at m/44'/60'/0'/0/0.
    const EXPECTED: &str = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
    let actual = derive_evm_address(TEST_MNEMONIC, Network::Ethereum).expect("valid mnemonic");
    assert_eq!(
        format!("{actual}"),
        EXPECTED,
        "MnemonicBuilder derived {actual} but expected {EXPECTED} at m/44'/60'/0'/0/0",
    );
}
