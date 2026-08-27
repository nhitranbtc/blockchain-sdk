//! TRON spike V1–V10 verification harness (Issue #403).
//!
//! Each module is a thin verification primitive for one open question from
//! the TRON Rust SDK deep-dive (PR #402, Issue #399). Real implementation
//! lives in `crates/tron-wallet-core/` per the plan in
//! `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`.

use serde::Deserialize;
use tron_v1_spike::address::from_base58check;
use tron_v1_spike::config::nile_config;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Token {
    symbol: String,
    name: String,
    address: String,
    decimals: u8,
    issuer: String,
}

#[test]
fn v9_mainnet_registry_loads_5_entries() {
    let raw = include_str!("../tokens/mainnet.json");
    let tokens: Vec<Token> = serde_json::from_str(raw).expect("mainnet.json parse");
    assert_eq!(tokens.len(), 5, "mainnet must have 5 entries per plan §Q9");

    let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
    assert!(symbols.contains(&"USDT"));
    assert!(symbols.contains(&"USDC"));
    assert!(symbols.contains(&"TUSD"));
    assert!(symbols.contains(&"USDD"));
    assert!(symbols.contains(&"stUSDT"));
}

#[test]
fn v9_nile_registry_loads_1_entry() {
    // Nile config is loaded from the lib's single source of truth.
    let cfg = nile_config();
    assert_eq!(
        cfg.tokens.len(),
        1,
        "nile must have 1 community test USDT entry"
    );
    assert_eq!(cfg.tokens[0].decimals, 6);
    assert_eq!(cfg.tokens[0].symbol, "USDT");
}

#[test]
fn v9_usdt_mainnet_decimals_6_and_address_t_prefix() {
    let raw = include_str!("../tokens/mainnet.json");
    let tokens: Vec<Token> = serde_json::from_str(raw).unwrap();
    let usdt = tokens.iter().find(|t| t.symbol == "USDT").unwrap();
    assert_eq!(usdt.decimals, 6);
    assert!(
        usdt.address.starts_with('T'),
        "USDT address must be T-base58check: {}",
        usdt.address
    );
    assert_eq!(usdt.address, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
}

#[test]
fn v9_usdt_decimals_onchain_live() {
    if std::env::var("RUN_TRON_NILE").ok().as_deref() != Some("1") {
        eprintln!("[SKIP — RUN_TRON_NILE=1 required for V9 live decimals() call]");
        return;
    }

    let cfg = nile_config();
    let rpc_url = cfg.rpc_url;
    let usdt_address_t = cfg
        .tokens
        .iter()
        .find(|t| t.symbol == "USDT")
        .expect("USDT-TEST must be in nile config")
        .address
        .clone();

    // See V5 for root-cause rationale: `/wallet/triggerconstantcontract`
    // requires 21-byte hex form for owner_address + contract_address.
    let owner_t = "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb";
    let owner_hex = hex::encode(from_base58check(owner_t).expect("owner T-address decodes"));
    let contract_hex =
        hex::encode(from_base58check(&usdt_address_t).expect("USDT T-address decodes"));

    let body = serde_json::json!({
        "owner_address": owner_hex,
        "contract_address": contract_hex,
        "function_selector": "decimals()",
        "parameter": "",
        "call_value": 0,
    });

    let resp: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("{rpc_url}/wallet/triggerconstantcontract"))
        .json(&body)
        .send()
        .expect("Nile RPC unreachable")
        .json()
        .expect("non-JSON response");

    // `constant_result` is a hex string of the 32-byte ABI-encoded uint256.
    // For decimals() = 6, the hex is "000...006" (right-padded to 32 bytes).
    let hex = resp
        .get("constant_result")
        .and_then(|v: &serde_json::Value| v.as_array())
        .and_then(|a: &Vec<serde_json::Value>| a.first())
        .and_then(|v: &serde_json::Value| v.as_str())
        .expect("missing constant_result[0]");

    let bytes = hex::decode(hex.trim_start_matches("0x")).expect("hex decode");
    let last = *bytes.last().expect("empty");
    assert_eq!(last, 6, "USDT decimals() must return 6, got {last}");
    eprintln!("[PASS] V9 USDT on-chain decimals = {last}");
}
