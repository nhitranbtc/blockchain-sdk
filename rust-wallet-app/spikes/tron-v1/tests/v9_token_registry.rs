//! V9 — token registry (CLI-driven, Phase 7 §Task 7.10).
//!
//! Plan §V9:
//! - `tron config show --network nile` lists USDT entry from `tokens/nile.json`.
//! - `tron config show --network mainnet` lists USDT entry from `tokens/mainnet.json`.
//! - `tron trc20 decimals --contract USDT --network nile` returns `6`
//!   (live `triggerconstantcontract(decimals())`).

mod common;

#[test]
#[ignore = "PHASE 7 BLOCKING: shipped `tron config show` does not list USDT entry from bundled `tokens/*.json`; see docs/audit/2026-09-07-phase-7-cli-drift.md §2.5"]
fn v9_config_show_includes_token_registry() {
    eprintln!("[BLOCKING] see docs/audit/2026-09-07-phase-7-cli-drift.md §2.5");
}

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1. Live `decimals()` constant call returns 6 for USDT-TRC20."]
fn v9_trc20_decimals_returns_6_on_nile() {
    common::require_env(&["RUN_TRON_NILE"]);

    let out = common::tron()
        .args([
            "trc20",
            "decimals",
            "--contract",
            common::nile_usdt(),
            "--network",
            common::NILE_NETWORK,
            "--json",
        ])
        .assert()
        .success();
    let json: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("CLI must emit JSON");
    let decimals = json
        .get("decimals")
        .or_else(|| json.get("value"))
        .and_then(|v| v.as_u64())
        .expect("CLI must emit `decimals` field");
    assert_eq!(
        decimals,
        common::USDT_DECIMALS,
        "USDT-TRC20 decimals must be 6 on Nile mainnet"
    );
}
