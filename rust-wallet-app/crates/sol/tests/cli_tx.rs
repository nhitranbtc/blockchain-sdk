//! `sol` CLI integration tests — tx command surface.
//!
//! Phase 7 verification — clap-level rejects for `tx get` + `tx wait`. The
//! `Wait` variant is the carrier of the P7-11 timeout bound (1..=600s), which
//! is enforced by clap's `value_parser` rather than a runtime guard, so these
//! tests are the gate that catches regressions in the boundary. Surfpool-gated
//! poll-for-confirm e2e lives in `cli_integration_surfpool.rs`.

use assert_cmd::Command;
use predicates::prelude::predicate;
use predicates::str::contains;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

// 88-char base58 Solana signature fixture — all `5`s decode cleanly so it
// reaches the runtime layer (no RPC) rather than failing clap parse. Used to
// verify a subcommand *accepts* the flag set without tripping clap.
const VALID_BASE58_SIG: &str =
    "5555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555";

#[test]
fn tx_get_requires_signature() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("get")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn tx_get_invalid_signature_base58_rejects() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("get")
        .arg("--sig")
        .arg("not-a-base58-sig!!!")
        .assert()
        .failure()
        .stderr(contains("invalid signature base58"));
}

#[test]
fn tx_get_with_json_flag_passes_clap() {
    // `--json` is a real flag on `tx get`. Clap should accept it; runtime
    // will then fail (no RPC configured in tmpdir), but the failure must NOT
    // be clap exit 2.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("get")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--json")
        .assert()
        .failure()
        .code(predicate::ne(2));
}

#[test]
fn tx_wait_requires_signature() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn tx_wait_invalid_signature_base58_rejects() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg("not-a-base58-sig!!!")
        .assert()
        .failure()
        .stderr(contains("invalid signature base58"));
}

#[test]
fn tx_wait_timeout_zero_rejects_p7_11() {
    // P7-11: timeout bound is `1..=600` enforced by clap `value_parser`.
    // Anything below 1 must exit 2 before any process startup.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("0")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn tx_wait_timeout_above_max_rejects_p7_11() {
    // P7-11 upper bound: 600s. 601s must be rejected at clap level.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("601")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn tx_wait_timeout_boundary_max_passes_clap_p7_11() {
    // P7-11 inclusive upper bound: 600s accepted by clap. Runtime then fails
    // (no RPC) but the failure must NOT be clap exit 2.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("600")
        .assert()
        .failure()
        .code(predicate::ne(2));
}

#[test]
fn tx_wait_timeout_negative_rejects_p7_11() {
    // clap `value_parser!(u64)` rejects negative integers at parse time.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("-1")
        .assert()
        .failure()
        .code(2);
}
