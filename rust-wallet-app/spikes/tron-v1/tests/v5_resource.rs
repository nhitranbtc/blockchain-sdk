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

const NILE_HOST: &str = "https://nile.trongrid.io";

#[test]
fn v5_resource_estimate_energy_for_usdt_decimals() {
    if std::env::var("RUN_TRON_NILE").ok().as_deref() != Some("1") {
        eprintln!("[SKIP — RUN_TRON_NILE=1 required for V5 live Nile RPC]");
        return;
    }

    // USDT-TRC20 on Nile (community test token, same selector behavior).
    let contract_address_t = "TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z";

    let body = serde_json::json!({
        "owner_address": "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb", // any valid T-address
        "contract_address": contract_address_t,
        "function_selector": "decimals()",
        "parameter": "",
        "call_value": 0,
    });

    // Synchronous reqwest call (no tokio runtime needed for this single blocking call).
    let resp = reqwest::blocking::Client::new()
        .post(format!("{NILE_HOST}/wallet/triggerconstantcontract"))
        .json(&body)
        .send()
        .expect("Nile RPC unreachable");
    let json: serde_json::Value = resp.json().expect("non-JSON response");

    let energy_used = json
        .get("energy_used")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| panic!("missing energy_used in response: {json}"));

    assert!(
        (50_000..=150_000).contains(&energy_used),
        "energy_used out of expected band: {energy_used}"
    );

    eprintln!("[PASS] V5 Nile USDT decimals() energy_used = {energy_used}");
}
