//! Shared helpers for L29 Sepolia e2e tests (Issue #310, Plan Task 11).
//!
//! All helpers are operator-facing only — tests that depend on them are
//! `#[ignore]` per L29 (operator-driven, never in CI). The env-var opt-in
//! pattern + safe skip-on-missing-env is the single-source-of-truth for
//! how these tests behave.
//!
//! Required env under `RUN_ETH_E2E=1`:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (12+ words; signs m/44'/60'/0'/0/0)
//!   ETH_E2E_TOKEN_ADDRESS   required only for ERC-20 tests
//!   ETH_E2E_RECIPIENT       optional; default = m/44'/60'/0'/0/1 (self-derived)
//!
//! Operator script: `rust-wallet-app/scripts/eth-send-sepolia-e2e.sh`
//! exits non-zero on any `#[ignore]`-driven FAIL.

#![allow(dead_code)]

use alloy_network::Ethereum;
use alloy_primitives::Address;
use alloy_provider::RootProvider;
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner};
use evm_wallet_core::new_http;
use std::str::FromStr;

pub const SEPOLIA_CHAIN_ID: u64 = 11155111;
pub const DEFAULT_TESTNET: &str = "Sepolia (chain id 11155111)";

/// Returns true iff `RUN_ETH_E2E=1` (or `true`) — operator opt-in per L29.
pub fn env_opt_in() -> bool {
    std::env::var("RUN_ETH_E2E")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Read a required env var; on missing returns the formatted error message
/// (test prints + returns silently).
pub fn require_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("missing required env var: {name}"))
}

/// Print a SKIP line to stderr with the canonical `[<label> SKIP] <msg>`
/// shape used by the spike samples (Issue #299 — ref for #298).
pub fn eprintln_skip(label: &str, msg: &str) {
    eprintln!("[{label} SKIP] {msg}");
}

/// Parse an env var as an `Address`, returning the formatted error on
/// invalid input. Operator-facing.
pub fn require_env_as_address(name: &str) -> Result<Address, String> {
    let raw = require_env(name)?;
    Address::from_str(&raw).map_err(|e| format!("{name} invalid: {e}"))
}

/// Build a `PrivateKeySigner` from a phrase at the given account index
/// (`m/44'/60'/0'/0/<index>`). Returns Err with operator-facing text.
pub fn build_signer(phrase: &str, index: u32) -> Result<PrivateKeySigner, String> {
    MnemonicBuilder::english()
        .phrase(phrase)
        .index(index)
        .expect("valid account index")
        .build()
        .map_err(|e| format!("mnemonic build at index {index}: {e}"))
}

/// Build signer at `m/44'/60'/0'/0/0` (the canonical sender index).
pub fn build_signer_index_0(phrase: &str) -> Result<PrivateKeySigner, String> {
    build_signer(phrase, 0)
}

/// Resolve `ETH_E2E_RECIPIENT` (override) or derive `m/44'/60'/0'/0/1` from
/// the mnemonic. Mirrors the spike V5 pattern.
pub fn resolve_recipient(phrase: &str) -> Result<Address, String> {
    match std::env::var("ETH_E2E_RECIPIENT") {
        Ok(v) => Address::from_str(&v).map_err(|e| format!("ETH_E2E_RECIPIENT invalid: {e}")),
        Err(_) => build_signer(phrase, 1).map(|s| s.address()),
    }
}

/// Open the canonical `new_http`-backed provider (library surface — Issue #305).
/// Returns Err with operator-facing text on URL parse failure.
pub fn open_provider(label: &str) -> Result<RootProvider<Ethereum>, String> {
    let rpc_url = require_env("ETH_E2E_RPC_URL")?;
    let url = rpc_url
        .parse()
        .map_err(|e| format!("{label}: ETH_E2E_RPC_URL unparsable: {e}"))?;
    new_http(url).map_err(|e| format!("{label}: new_http failed: {e}"))
}

/// Preflight that gates every e2e test: env opt-in + required env vars +
/// signer + provider. Returns `Some((provider, sender))` if the test should
/// proceed; `None` if the test should skip with a friendly message.
///
/// `label` is used in SKIP lines + in error messages.
pub fn preflight_or_skip(label: &str) -> Option<(RootProvider<Ethereum>, PrivateKeySigner)> {
    if !env_opt_in() {
        eprintln_skip(label, &format!("set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC (default testnet = {DEFAULT_TESTNET})"));
        return None;
    }
    let phrase = match require_env("ETH_E2E_MNEMONIC") {
        Ok(v) => v,
        Err(e) => {
            eprintln_skip(label, &e);
            return None;
        }
    };
    let provider = match open_provider(label) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[{label} FAIL] {e}");
            return None;
        }
    };
    let signer = match build_signer_index_0(&phrase) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[{label} FAIL] {e}");
            return None;
        }
    };
    Some((provider, signer))
}
