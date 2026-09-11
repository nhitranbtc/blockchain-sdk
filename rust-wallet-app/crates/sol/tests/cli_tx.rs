//! `sol` CLI integration tests — tx command surface.
//!
//! Phase 7 verification — 10/10 CLI files compile. Clap-level rejects + P7-11
//! timeout bounds already exercised via cli_wallet.rs (see tx_wait_timeout_*
//! tests). Surfpool-gated poll-for-confirm e2e lives in 7.2.

use assert_cmd::Command;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

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
        .failure();
}
