//! `sol` CLI integration tests — address command surface.
//!
//! Phase 7 verification — 10/10 CLI files compile. Clap-level rejects only
//! (P7-16: --mnemonic inline rejected; --mnemonic-file valid base58 required).
//! Surfpool-gated derivation e2e lives in 7.2.

use assert_cmd::Command;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

#[test]
fn address_new_requires_wallet_id_or_mnemonic_file() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("new")
        .assert()
        .failure()
        .stderr(predicates::str::contains("specify one of"));
}

#[test]
fn address_pubkey_requires_wallet_id() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("address")
        .arg("pubkey")
        .assert()
        .failure()
        .code(2);
}
