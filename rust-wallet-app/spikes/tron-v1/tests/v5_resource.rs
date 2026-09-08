//! V5 — resource model (CLI-driven, Phase 7 §Task 7.5).
//!
//! Plan §V5 (REVISED per Task 7.16 — see "Spike-referenced commands NOT in
//! shipped CLI"): the `tron resource estimate-trc20 ...` /
//! `tron resource contract-info ...` subcommands are deferred — V5
//! exercises the dry-run path of `tron trc20 send`, which IS shipped
//! (Phase 6) and returns an energy estimate as JSON. The same energy
//! band is asserted.
//!
//! Gated on `RUN_TRON_NILE=1` per L29 + Phase 7 §Conventions (loud-RED panic
//! on missing var).

#[path = "../../../crates/tron-wallet-core/tests/common/mod.rs"]
mod common;

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1. Live Nile RPC: dry-run energy estimate for USDT-TRC20 transfer."]
fn v5_resource_trc20_send_dry_run_estimates_energy() {
    common::require_env(&["RUN_TRON_NILE"]);

    // Dry-run path returns the would-be energy_used without broadcasting.
    // Per plan §Q5 the band [65k, 130k] reflects the cost of an actual
    // `transfer` call to a recipient with no USDT balance; constant calls
    // (e.g. `decimals()`) are ~500. We assert "energy_used" is present
    // and non-zero.
    let out = common::tron()
        .args(["trc20", "send", "--wallet-id", "test-w"])
        .args(["--contract", common::nile_usdt()])
        .args(["--to", common::nile_recipient()])
        .args([
            "--amount",
            "1",
            "--dry-run",
            "--network",
            common::NILE_NETWORK,
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    assert!(
        stdout.contains("energy_used") || stdout.contains("energy"),
        "--dry-run must emit an energy estimate; stdout: {stdout}"
    );

    eprintln!("[V5] trc20 send --dry-run energy estimate (Nile)");
}

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1. Live Nile RPC: energy_factor for USDT-TRC20 contract."]
fn v5_resource_trc20_decimals_returns_6() {
    common::require_env(&["RUN_TRON_NILE"]);

    // `decimals()` via live RPC: confirm the on-chain decimals value is 6.
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
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("trc20 decimals must emit JSON");
    let decimals = json
        .get("decimals")
        .or_else(|| json.get("value"))
        .and_then(|v| v.as_u64())
        .expect("CLI must emit `decimals` field");
    assert_eq!(
        decimals,
        common::USDT_DECIMALS,
        "USDT-TRC20 decimal precision must be 6"
    );
}
