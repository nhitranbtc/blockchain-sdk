//! Phase 6.1 test helpers — cross-file.

// Shared fixture module — each integration-test binary is a separate
// compilation unit, so symbols used by one test file appear unused to
// the others. Allow dead_code crate-wide for this fixture.
#![allow(dead_code)]

// -- Devnet-gated send-test constants + helpers -------------------------------
//
// Shared by `tests/submit_devnet_send.rs` (and any future devnet-gated
// send tests). Pulls `devnet.json` directly via `include_str!` so tests
// don't depend on `tokens::load_devnet()` (currently broken at runtime
// per audit-issue #563 P9-2 deviation).

use solana_sdk::hash::Hash;
use std::str::FromStr;

/// Bundled devnet config (relative path from `tests/common/`).
pub const DEVNET_JSON: &str = include_str!("../../src/tokens/devnet.json");

/// Phantom-canonical BIP-39 mnemonic for the devnet-funded sender.
pub const SENDER_MNEMONIC: &str =
    "pact possible desk flag lawn antique tumble hip staff draw abuse peasant";

/// Expected sender pubkey (Phantom-canonical derivation).
pub const EXPECTED_SENDER_PUBKEY: &str = "27mt9dL81aHVsBnebBBB3ZZnm7UVSbQ354XcZSUss4cd";

/// Recipient wallet base58 pubkey (devnet).
pub const RECIPIENT_PUBKEY: &str = "GSKYBnM2NeGT3ckfMAqEDkRxtgD9bZrcMxPwtiNgG6aj";

/// Devnet USDC mint (classic token program).
pub const USDC_MINT: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

/// Placeholder blockhash for unit tests that only inspect message shape.
/// Not a real on-chain hash — just satisfies `recent_blockhash` field.
pub const DUMMY_BLOCKHASH: &str = "11111111111111111111111111111111";

/// Build a `Hash` from `DUMMY_BLOCKHASH` for unit-test message builders.
pub fn dummy_blockhash() -> Hash {
    Hash::from_str(DUMMY_BLOCKHASH).expect("valid dummy blockhash")
}

/// Devnet config parsed from `devnet.json`.
pub struct DevnetConfig {
    /// RPC endpoint (from `rpc-endpoint`).
    pub rpc_endpoint: String,
    /// Sender wallet BIP-39 mnemonic (Phantom-compatible).
    pub sender_mnemonic: String,
    /// Recipient wallet base58 pubkey.
    pub recipient: String,
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
        #[serde(rename = "recipient-devnet")]
        recipient: String,
    }
    let w = serde_json::from_str::<Wire>(DEVNET_JSON).expect(
        "devnet.json: must include rpc-endpoint + sender-devnet-mnemnoic + recipient-devnet",
    );
    DevnetConfig {
        rpc_endpoint: w.rpc_endpoint,
        sender_mnemonic: w.sender_mnemonic,
        recipient: w.recipient,
    }
}
