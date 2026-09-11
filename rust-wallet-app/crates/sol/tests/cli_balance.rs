//! `sol` CLI integration tests — balance command surface.
//!
//! Phase 7 verification — 10/10 CLI files compile. Clap-level rejects only.
//! Surfpool-gated RPC query e2e lives in 7.2.

use assert_cmd::Command;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

#[test]
fn balance_sol_requires_address() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("balance")
        .arg("sol")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn balance_spl_requires_address_and_token() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("balance")
        .arg("spl")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn balance_sol_invalid_base58_rejects() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("balance")
        .arg("sol")
        .arg("--address")
        .arg("not-a-base58-pubkey!!!")
        .assert()
        .failure();
}
