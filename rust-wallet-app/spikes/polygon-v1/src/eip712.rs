//! EIP-712 typed-data signing (Q7 — cross-chain replay protection).
//!
//! The EIP-712 domain separator includes `chainId` per spec. A payload
//! signed with domain `chainId = 137` (Polygon mainnet) MUST NOT recover
//! on any verifier that uses `chainId = 1` (Ethereum mainnet) — even
//! though the rest of the typed data + signature is byte-identical.
//!
//! V10 exercises this property by signing with one chain-id domain and
//! asserting recovery with a different chain-id domain fails (via the
//! `v10_sign_then_recover_with_wrong_chain_id_fails` test in
//! `tests/v10_eip712_replay.rs`).
//!
//! Production `evm-wallet-core` uses the same pattern with the domain
//! fed by `config::ChainConfig::chain_id`.

use alloy_primitives::{Address, B256};
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner};
use alloy_sol_types::{eip712_domain, Eip712Domain};

use crate::config::{ETHEREUM_MAINNET_CHAIN_ID, POLYGON_AMOY_CHAIN_ID, POLYGON_MAINNET_CHAIN_ID};

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
/// V10 uses this to sign payloads across two chain-id domains.
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

/// Wrap an arbitrary 32-byte message digest as an `alloy_primitives::B256`.
/// L12 finding: previously named `typed_data_hash` which falsely implied
/// EIP-712 semantics — this helper only performs a `B256::from` wrapping,
/// no `keccak256(0x1901 || domainSeparator || hashStruct(...))` pipeline.
/// Renamed to clarify intent; production `polygon-wallet-core` will wire
/// real `Eip712::hash_struct` calls instead.
pub fn wrap_digest(message: [u8; 32]) -> B256 {
    B256::from(message)
}

/// Convenience re-export: Ethereum mainnet chain id (1).
pub const CHAIN_ID_ETHEREUM: u64 = ETHEREUM_MAINNET_CHAIN_ID;
/// Convenience re-export: Polygon mainnet chain id (137).
pub const CHAIN_ID_POLYGON: u64 = POLYGON_MAINNET_CHAIN_ID;
/// Convenience re-export: Polygon Amoy testnet chain id (80002).
pub const CHAIN_ID_POLYGON_AMOY: u64 = POLYGON_AMOY_CHAIN_ID;
