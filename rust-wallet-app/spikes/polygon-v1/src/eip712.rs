//! EIP-712 typed-data signing (Q7 — cross-chain replay protection).
//!
//! The EIP-712 domain separator includes `chainId` per spec. A payload
//! signed with domain `chainId = 137` (Polygon mainnet) MUST NOT recover
//! on any verifier that uses `chainId = 1` (Ethereum mainnet) — even
//! though the rest of the typed data + signature is byte-identical.
//!
//! V10 exercises this property by signing with one chain-id domain,
//! then attempting recovery with a different chain-id domain. Recovery
//! MUST fail.
//!
//! Production `evm-wallet-core` uses the same pattern with the domain
//! fed by `config::ChainConfig::chain_id`.

use alloy_primitives::{Address, FixedBytes, B256};
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner};
use alloy_sol_types::{eip712_domain, Eip712Domain};

use crate::config::Network;
use crate::config::ETHEREUM_MAINNET_CHAIN_ID;
use crate::config::POLYGON_AMOY_CHAIN_ID;
use crate::config::POLYGON_MAINNET_CHAIN_ID;

/// EIP-712 domain for a chain — used by V10 + future `evm-wallet-core`
/// `sign_typed_data` paths.
pub fn domain_for_chain(chain_id: u64, verifying_contract: Address) -> Eip712Domain {
    eip712_domain! {
        name: "PolygonV1Spike",
        version: "1",
        chain_id: chain_id,
        verifying_contract: verifying_contract,
    }
}

/// Build the canonical test signer ("abandon ×11 + about" at m/44'/60'/0'/0/0).
/// V10 uses this to sign a payload with one chain-id domain.
pub fn build_test_signer() -> PrivateKeySigner {
    const TEST_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    MnemonicBuilder::english()
        .phrase(TEST_MNEMONIC)
        .index(0)
        .expect("hard-coded index 0")
        .build()
        .expect("valid mnemonic")
}

/// A trivial typed-data hash for V10. Real `evm-wallet-core` would feed
/// a Permit2 / EIP-2612 typed struct here; the spike uses a 32-byte
/// message digest to keep the dependency surface narrow.
pub fn typed_data_hash(message: [u8; 32]) -> B256 {
    B256::from(FixedBytes::<32>::from(message))
}

/// Re-exports of chain-id constants for tests that want to construct
/// a domain without importing `crate::config` directly.
pub fn chain_id(network: Network) -> u64 {
    match network {
        Network::Ethereum => ETHEREUM_MAINNET_CHAIN_ID,
        Network::Polygon => POLYGON_MAINNET_CHAIN_ID,
        Network::PolygonAmoy => POLYGON_AMOY_CHAIN_ID,
    }
}
