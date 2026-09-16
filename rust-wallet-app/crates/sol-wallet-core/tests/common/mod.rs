//! Phase 6.1 test helpers — cross-file.

// Shared fixture module — each integration-test binary is a separate
// compilation unit, so symbols used by one test file appear unused to
// the others. Allow dead_code crate-wide for this fixture.
#![allow(dead_code)]

// -- Submodules (Phase 7.1c / 7.2 surfpool fixtures) --
pub mod faucet;
pub mod keypair_fixture;
pub mod mock_spl_usdc;
pub mod surfpool_spawn;

// -- Devnet-gated send-test helpers -----------------------------------------
//
// Single source of truth: `src/tokens/devnet.json` (bundled via
// `include_str!`). Tests don't depend on `tokens::load_devnet()`
// (broken at runtime per audit-issue #563 P9-2 deviation).

use solana_sdk::hash::Hash;
use std::str::FromStr;

/// Bundled devnet config (relative path from `tests/common/`).
pub const DEVNET_JSON: &str = include_str!("../../src/tokens/devnet.json");

/// Placeholder blockhash for unit tests that only inspect message shape.
/// Not a real on-chain hash — just satisfies `recent_blockhash` field.
pub const DUMMY_BLOCKHASH: &str = "11111111111111111111111111111111";

/// Build a `Hash` from `DUMMY_BLOCKHASH` for unit-test message builders.
pub fn dummy_blockhash() -> Hash {
    Hash::from_str(DUMMY_BLOCKHASH).expect("valid dummy blockhash")
}

/// Devnet config parsed from `devnet.json`. Every field callers need
/// (mnemonics, pubkeys, USDC mint + decimals, RPC URL) lives here —
/// tests must NOT re-declare any of these as their own constants.
pub struct DevnetConfig {
    /// RPC endpoint (from `rpc-endpoint`).
    pub rpc_endpoint: String,
    /// Sender wallet BIP-39 mnemonic (Phantom-compatible).
    pub sender_mnemonic: String,
    /// Sender wallet base58 pubkey (Phantom-canonical).
    pub sender_pubkey: String,
    /// Recipient wallet BIP-39 mnemonic.
    pub recipient_mnemonic: String,
    /// Recipient wallet base58 pubkey.
    pub recipient: String,
    /// Devnet USDC mint (classic token program).
    pub usdc_mint: String,
    /// Devnet USDC mint decimals (from `tokens[].decimals`).
    pub usdc_decimals: u8,
}

/// Parse `devnet.json` via `serde_json` — bypasses `tokens::load_devnet()`
/// (broken per audit-issue #563 P9-2 deviation).
pub fn load_config() -> DevnetConfig {
    #[derive(serde::Deserialize)]
    struct Wire {
        #[serde(rename = "rpc-endpoint")]
        rpc_endpoint: String,
        #[serde(rename = "sender-devnet-mnemnoic")]
        sender_mnemonic: String,
        #[serde(rename = "sender-devnet")]
        sender_pubkey: String,
        #[serde(rename = "recipient-devnet-mnemnoic")]
        recipient_mnemonic: String,
        #[serde(rename = "recipient-devnet")]
        recipient: String,
        tokens: Vec<Token>,
    }
    #[derive(serde::Deserialize)]
    struct Token {
        symbol: String,
        mint: String,
        decimals: u8,
    }
    let w = serde_json::from_str::<Wire>(DEVNET_JSON).expect(
        "devnet.json: must include rpc-endpoint + sender-devnet-mnemnoic + sender-devnet \
         + recipient-devnet-mnemnoic + recipient-devnet + tokens[].{symbol,mint,decimals}",
    );
    let usdc = w
        .tokens
        .iter()
        .find(|t| t.symbol == "USDC")
        .expect("devnet.json: tokens[] must include symbol=USDC");
    DevnetConfig {
        rpc_endpoint: w.rpc_endpoint,
        sender_mnemonic: w.sender_mnemonic,
        sender_pubkey: w.sender_pubkey,
        recipient_mnemonic: w.recipient_mnemonic,
        recipient: w.recipient,
        usdc_mint: usdc.mint.clone(),
        usdc_decimals: usdc.decimals,
    }
}
