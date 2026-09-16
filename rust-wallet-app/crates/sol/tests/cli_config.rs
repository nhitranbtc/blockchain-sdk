//! `sol` CLI integration tests — config command surface.
//!
//! Phase 7 verification — 10/10 CLI files compile. Clap-level rejects + P7-8
//! URL validation (https-only / no userinfo) + P7-20 mainnet transition gate
//! already exercised via cli_wallet.rs (config_set_rpc_* + config_set_cluster_*).

use assert_cmd::Command;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

#[test]
fn config_show_runs_on_clean_dir() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("config")
        .arg("show")
        .assert()
        .success();
}

#[test]
fn config_set_rpc_requires_url() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("config")
        .arg("set-rpc")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn config_set_cluster_requires_cluster_arg() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("config")
        .arg("set-cluster")
        .assert()
        .failure()
        .code(2);
}
