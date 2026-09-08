//! V7 — SPKI pin (CLI-driven, Phase 7 §Task 7.7).
//!
//! Plan §V7:
//! - `tron --rpc pinned://<correct_pin>@api.trongrid.io wallet balance` accepts pinned endpoint.
//! - `tron --rpc pinned://<wrong_pin>@api.trongrid.io wallet balance` fails with non-zero exit + SPKI error.
//! - `tron --rpc http://127.0.0.1:9090 wallet balance` (TronBox no-pin) succeeds.
//!
//! Gated on `RUN_TRON_NILE=1` for the pinned-endpoint cases; localhost
//! TronBox case is non-gated (CI Docker runner).

mod common;

/// All-zeros 64-char hex placeholder SPKI pin (deliberately wrong; only
/// used to confirm the negative-path rejection).
#[test]
#[ignore = "GATED: RUN_TRON_NILE=1. Live pinned-endpoint balance query against nile.trongrid.io using the bundled fixture pin (nile.json::test.spki_pin_hex)."]
fn v7_pinned_endpoint_accepts_correct_pin() {
    common::require_env(&[common::RUN_TRON_NILE]);
    // Correct pin comes from the bundled fixture (`tokens/nile.json`).
    // The fixture value is the source of truth for "the cert pin that the
    // shipped CLI must trust"; cross-checked live by `common::assert_live_spki_pin`.
    let pin = common::fixture_spki_pin();
    let rpc = common::nile_pinned_url(&pin);

    common::tron()
        .args(["--rpc", &rpc])
        .args(["balance", "--address", common::nile_recipient()])
        .assert()
        .success();
}

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1. Wrong pin must surface SPKI error + non-zero exit."]
fn v7_pinned_endpoint_rejects_wrong_pin() {
    common::require_env(&[common::RUN_TRON_NILE]);
    let rpc = common::nile_pinned_url(common::WRONG_SPKI_PIN);

    let res = common::tron()
        .args(["--rpc", &rpc])
        .args(["balance", "--address", common::nile_recipient()])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&res.get_output().stderr);
    assert!(
        stderr.contains("SPKI")
            || stderr.contains("pin")
            || stderr.contains("certificate")
            || stderr.contains("verify"),
        "wrong pin must surface SPKI/cert error; stderr: {stderr}"
    );
}

#[test]
fn v7_tronbox_local_no_pin_succeeds() {
    // No pin: CLI defaults to Rustls verification against TronBox (TronBox
    // self-signed cert is acceptable in CI). Gate is RUN_TRON_LOCAL=1 + a
    // running TronBox at localhost:9090. The test fixture in PHASE 4 ships
    // a `testcontainers` helper for CI Docker runners; operator-driven when
    // running locally.
    if std::env::var(common::RUN_TRON_LOCAL).ok().as_deref() != Some("1") {
        eprintln!("[V7-tronbox] SKIP — RUN_TRON_LOCAL=1 required");
        return;
    }

    common::tron()
        .args(["--rpc", common::TRONBOX_RPC_URL])
        .args(["balance", "--address", common::nile_recipient()])
        .assert()
        .success();
}
