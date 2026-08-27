//! V5 — Nile resource model (Q5) — GATED behind RUN_TRON_NILE=1 (L29).
//!
//! Plan §Q5: Stake 2.0 (April 2023 via proposal #84 / TIP-467); 1 TRX = 1 TP;
//! USDT-TRC20 `transfer` = ~65,000 Energy if recipient holds USDT, ~130,300 if empty;
//! Bandwidth: free 600/day + 1,000 sun/byte TRX burn fallback. DEM penalty scales
//! per-contract energy by `max_factor = 3.4` per 6-hour cycle. `fee_limit` denominated
//! in SUN, not TRX. Estimation: `wallet/triggerconstantcontract` returns
//! `energy_used` + optional `energy_penalty`; fallback `wallet/estimateenergy`.
//!
//! Without RUN_TRON_NILE=1 this prints `[SKIP — RUN_TRON_NILE=1 required]` and exits 0.
//! With it, this test calls `wallet/triggerconstantcontract` against USDT-TRC20's
//! `decimals()` selector and asserts `energy_used` falls in [50_000, 150_000]
//! (USDT constants call range, generous bounds).

use tron_v1_spike::address::from_base58check;
use tron_v1_spike::config::nile_config;

#[test]
fn v5_resource_estimate_energy_for_usdt_decimals() {
    if std::env::var("RUN_TRON_NILE").ok().as_deref() != Some("1") {
        eprintln!("[SKIP — RUN_TRON_NILE=1 required for V5 live Nile RPC]");
        return;
    }

    // USDT-TRC20 on Nile (resolved via config; verified live 2026-08-27).
    let contract_address_t = tron_v1_spike::config::nile_config()
        .token("USDT")
        .map(|t| t.address.clone())
        .expect("USDT token must be present in tokens/nile.json");

    // `/wallet/triggerconstantcontract` requires 21-byte hex form for
    // `owner_address` + `contract_address` (NOT T-base58check). The Java-tron
    // server parses both fields as hex before protobuf serialization; sending
    // T-base58check produces `INVALID hex String` at the first non-hex char.
    // Verified live 2026-08-27: error position 1:36 = 'G' in T-base58check
    // owner_address = non-hex char. `triggersmartcontract` is permissive on
    // address format (use_case flow passes); this endpoint is strict.
    let owner_t = "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb";
    let owner_hex = hex::encode(from_base58check(owner_t).expect("owner T-address decodes"));
    let contract_hex =
        hex::encode(from_base58check(&contract_address_t).expect("USDT address decodes"));

    let body = serde_json::json!({
        "owner_address": owner_hex,
        "contract_address": contract_hex,
        "function_selector": "decimals()",
        "parameter": "",
        "call_value": 0,
    });

    // Synchronous reqwest call (no tokio runtime needed for this single blocking call).
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{}/wallet/triggerconstantcontract",
            nile_config().rpc_url
        ))
        .json(&body)
        .send()
        .expect("Nile RPC unreachable");
    let json: serde_json::Value = resp.json().expect("non-JSON response");

    let energy_used = json
        .get("energy_used")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| panic!("missing energy_used in response: {json}"));

    // `decimals()` is a constant getter — costs ~500 energy (vs `transfer`'s
    // ~65k-130k). Original `[50_000, 150_000]` band was a copy-paste from the
    // transfer test; corrected 2026-08-27 to reflect the constant-call cost.
    assert!(
        (100..=10_000).contains(&energy_used),
        "decimals() energy_used out of expected band: {energy_used}"
    );

    eprintln!("[PASS] V5 Nile USDT decimals() energy_used = {energy_used}");
}
