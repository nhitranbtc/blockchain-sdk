//! `disambig` tests — Phase 3 Task 5 of #425 (sub-task of #416).
//!
//! V10 mirror + Story 31 (legacy alias).
//!
//! Per issue body:
//! - `reject_bridged_usdc_e(address) -> Result<(), Error>` — fails if address matches
//!   bridged USDC.e address list (Polygon mainnet `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`).
//! - `gas_token_label(use_legacy: bool) -> &'static str` — returns "POL" or "MATIC".

use alloy_primitives::Address;
use evm_wallet_core::Error;
use polygon_wallet_core::disambig::{gas_token_label, reject_bridged_usdc_e};

#[test]
fn gas_token_label_default_returns_pol() {
    assert_eq!(gas_token_label(false), "POL");
}

#[test]
fn gas_token_label_legacy_returns_matic() {
    assert_eq!(gas_token_label(true), "MATIC");
}

#[test]
fn reject_bridged_usdc_e_accepts_native_mainnet_usdc() {
    // Native USDC on Polygon mainnet (Circle's official native issuance).
    // Per polygon-wallet-core/tokens/mainnet.json.
    let native: Address = "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
        .parse()
        .expect("native USDC mainnet address must parse");
    assert!(reject_bridged_usdc_e(native).is_ok());
}

#[test]
fn reject_bridged_usdc_e_passes_for_unrelated_address() {
    // Negative-check semantics: any address not on the disallow list
    // passes, including Address::ZERO and arbitrary ERC-20 addresses.
    // Regression guard against a future refactor that inverts the
    // check.
    assert!(reject_bridged_usdc_e(Address::ZERO).is_ok());
    let dai: Address = "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063"
        .parse()
        .expect("DAI mainnet");
    assert!(reject_bridged_usdc_e(dai).is_ok());
}

#[test]
fn reject_bridged_usdc_e_rejects_bridged_usdc_e() {
    // Bridged USDC.e on Polygon mainnet (legacy USDC bridged from Ethereum).
    // Historical canonical address per issue body.
    let bridged: Address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
        .parse()
        .expect("bridged USDC.e address must parse");
    let err = reject_bridged_usdc_e(bridged).expect_err("bridged USDC.e must be rejected");
    assert!(
        matches!(err, Error::InvalidInput(_)),
        "bridged USDC.e rejection must surface as InvalidInput (exit 2), got: {err:?}"
    );
    let msg = format!("{err}");
    assert!(
        msg.contains("USDC.e") || msg.contains("BRIDGED_USDC_REJECTED"),
        "error message must name USDC.e or carry the BRIDGED_USDC_REJECTED marker for log greps, got: {msg}"
    );
}
