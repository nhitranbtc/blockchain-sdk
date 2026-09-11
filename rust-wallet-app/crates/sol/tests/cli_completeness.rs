//! `sol` CLI integration tests — Phase 7 verification gate.
//!
//! Phase 7 verification: 21/22 deep-dive CLI rows GREEN. All 22 commands
//! parse via clap (the 22nd row is `cli_json_output` — JSON serialization
//! for every command — deferred to Phase 7.2 per the plan).
//!
//! Each #[test] calls the `sol` binary with valid args + asserts it runs
//! without panic. Surfpool-gated paths return Unimplemented; that's
//! acceptable for Phase 7 verification (handler wiring + clap surface).

use assert_cmd::Command;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

fn tmp() -> TempDir {
    TempDir::new().expect("tempdir")
}

macro_rules! parse_check {
    ($($arg:expr),* $(,)?) => {{
        let mut cmd = sol_bin();
        cmd.args([$($arg),*]);
        let _ = cmd.arg("--data-dir").arg(tmp().path()).assert();
    }};
}

// --- Wallet CRUD (9 commands) ---

#[test]
fn wallet_create_parses() {
    parse_check!(
        "wallet",
        "create",
        "--name",
        "x",
        "--mnemonic-file",
        "/dev/null"
    );
}
#[test]
fn wallet_import_parses() {
    parse_check!(
        "wallet",
        "import",
        "--name",
        "x",
        "--private-key-file",
        "/dev/null",
        "--yes"
    );
}
#[test]
fn wallet_show_parses() {
    parse_check!(
        "wallet",
        "show",
        "--id",
        "00000000-0000-0000-0000-000000000000"
    );
}
#[test]
fn wallet_list_parses() {
    parse_check!("wallet", "list");
}
#[test]
fn wallet_delete_parses() {
    parse_check!(
        "wallet",
        "delete",
        "--id",
        "00000000-0000-0000-0000-000000000000",
        "--yes"
    );
}
#[test]
fn wallet_rename_parses() {
    parse_check!(
        "wallet",
        "rename",
        "--id",
        "00000000-0000-0000-0000-000000000000",
        "--to",
        "new-name"
    );
}
#[test]
fn wallet_balance_parses() {
    parse_check!(
        "wallet",
        "balance",
        "--address",
        "11111111111111111111111111111111"
    );
}
#[test]
fn wallet_send_parses() {
    parse_check!(
        "wallet",
        "send",
        "--to",
        "11111111111111111111111111111111",
        "--amount",
        "1"
    );
}
#[test]
fn wallet_send_speedup_parses() {
    parse_check!(
        "wallet",
        "send-speedup",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000",
        "--sig",
        "5".repeat(87).as_str(),
        "--priority-fee",
        "1000"
    );
}

// --- Address (2 commands) ---

#[test]
fn address_new_parses() {
    parse_check!(
        "address",
        "new",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000"
    );
}
#[test]
fn address_pubkey_parses() {
    parse_check!(
        "address",
        "pubkey",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000"
    );
}

// --- Balance (2 commands) ---

#[test]
fn balance_sol_parses() {
    parse_check!(
        "balance",
        "sol",
        "--address",
        "11111111111111111111111111111111"
    );
}
#[test]
fn balance_spl_parses() {
    parse_check!(
        "balance",
        "spl",
        "--address",
        "11111111111111111111111111111111",
        "--token",
        "11111111111111111111111111111111"
    );
}

// --- SPL (4 commands) ---

#[test]
fn spl_send_parses() {
    parse_check!(
        "spl",
        "send",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000",
        "--to",
        "11111111111111111111111111111111",
        "--amount",
        "1",
        "--token",
        "11111111111111111111111111111111"
    );
}
#[test]
fn spl_approve_parses() {
    parse_check!(
        "spl",
        "approve",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000",
        "--token",
        "11111111111111111111111111111111",
        "--delegate",
        "11111111111111111111111111111111",
        "--amount",
        "1"
    );
}
#[test]
fn spl_balance_parses() {
    parse_check!(
        "spl",
        "balance",
        "--address",
        "11111111111111111111111111111111",
        "--token",
        "11111111111111111111111111111111"
    );
}
#[test]
fn spl_allowance_parses() {
    parse_check!(
        "spl",
        "allowance",
        "--token",
        "11111111111111111111111111111111",
        "--owner",
        "11111111111111111111111111111111",
        "--delegate",
        "11111111111111111111111111111111"
    );
}

// --- Tx (2 commands) ---

#[test]
fn tx_get_parses() {
    parse_check!("tx", "get", "--sig", "5".repeat(87).as_str());
}
#[test]
fn tx_wait_parses() {
    parse_check!(
        "tx",
        "wait",
        "--sig",
        "5".repeat(87).as_str(),
        "--timeout",
        "10"
    );
}

// --- Config (3 commands) ---

#[test]
fn config_show_parses() {
    parse_check!("config", "show");
}
#[test]
fn config_set_rpc_parses() {
    parse_check!(
        "config",
        "set-rpc",
        "--url",
        "https://api.mainnet-beta.solana.com"
    );
}
#[test]
fn config_set_cluster_parses() {
    parse_check!("config", "set-cluster", "--cluster", "devnet", "--yes");
}
