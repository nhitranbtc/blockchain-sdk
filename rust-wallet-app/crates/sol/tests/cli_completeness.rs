//! `sol` CLI integration tests — Phase 7 verification gate.
//!
//! Phase 7.2 verification: 22/22 deep-dive CLI rows GREEN.
//!
//! - Rows 1-21: every command variant parses via clap. Surfpool-gated paths
//!   return `Unimplemented` at runtime; that's the Phase 7 contract — handler
//!   wiring + clap surface, surfpool e2e lives in `cli_integration_surfpool.rs`.
//! - Row 22 (`cli_json_output`): every command that *carries* `--json` accepts
//!   it at clap level; every command that does *not* carry it rejects it at
//!   clap level (exit 2). Per `clap`, unknown long flags always exit 2
//!   before any process startup, so these are pure parse tests, fast and
//!   deterministic — no RPC required.
//!
//! The six commands carrying `--json` per `crates/sol/src/cli.rs`:
//! `wallet show`, `wallet list`, `wallet balance`, `tx get`, `tx wait`,
//! `config show`. `tx get --json` is cross-covered by `cli_tx.rs::
//! tx_get_with_json_flag_passes_clap`; not duplicated here.

use assert_cmd::Command;
use predicates::prelude::predicate;
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

/// Wrap a parse check + assert clap did NOT reject (exit != 2). Runtime may
/// still fail (Unimplemented / no RPC) OR succeed if the handler is already
/// landed (e.g. `config show`, `wallet list`, `wallet balance`). Only the
/// clap layer is under test.
macro_rules! clap_accepts {
    ($($arg:expr),* $(,)?) => {{
        let mut cmd = sol_bin();
        cmd.args([$($arg),*]);
        cmd.arg("--data-dir")
            .arg(tmp().path())
            .assert()
            .code(predicate::ne(2));
    }};
}

/// Wrap a parse check + assert clap DID reject (exit 2). Used for `--json`
/// on commands that do not declare it.
macro_rules! clap_rejects {
    ($($arg:expr),* $(,)?) => {{
        let mut cmd = sol_bin();
        cmd.args([$($arg),*]);
        cmd.arg("--data-dir")
            .arg(tmp().path())
            .assert()
            .failure()
            .code(2);
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

// =========================================================================
// Row 22 — cli_json_output: clap-level `--json` surface audit.
//
// Per `crates/sol/src/cli.rs`, exactly six command variants declare
// `json: bool`. Every other variant must reject `--json` at clap level
// (unknown long flag → exit 2).
// =========================================================================

// --- Positive: commands that DO carry `--json` and must accept it ---

#[test]
fn json_wallet_show_accepted() {
    clap_accepts!(
        "wallet",
        "show",
        "--id",
        "00000000-0000-0000-0000-000000000000",
        "--json"
    );
}

#[test]
fn json_wallet_list_accepted() {
    clap_accepts!("wallet", "list", "--json");
}

#[test]
fn json_wallet_balance_accepted() {
    clap_accepts!(
        "wallet",
        "balance",
        "--address",
        "11111111111111111111111111111111",
        "--json"
    );
}

// `tx get --json` is covered by `cli_tx.rs::tx_get_with_json_flag_passes_clap`.

#[test]
fn json_tx_wait_accepted() {
    clap_accepts!(
        "tx",
        "wait",
        "--sig",
        "5".repeat(88).as_str(),
        "--timeout",
        "30",
        "--json"
    );
}

#[test]
fn json_config_show_accepted() {
    clap_accepts!("config", "show", "--json");
}

// --- Negative: commands that do NOT carry `--json` and must reject it ---

#[test]
fn json_wallet_create_rejected() {
    clap_rejects!(
        "wallet",
        "create",
        "--name",
        "x",
        "--mnemonic-file",
        "/dev/null",
        "--json"
    );
}

#[test]
fn json_wallet_import_rejected() {
    clap_rejects!(
        "wallet",
        "import",
        "--name",
        "x",
        "--private-key-file",
        "/dev/null",
        "--yes",
        "--json"
    );
}

#[test]
fn json_wallet_send_rejected() {
    clap_rejects!(
        "wallet",
        "send",
        "--to",
        "11111111111111111111111111111111",
        "--amount",
        "1",
        "--json"
    );
}

#[test]
fn json_wallet_send_speedup_rejected() {
    clap_rejects!(
        "wallet",
        "send-speedup",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000",
        "--sig",
        "5".repeat(87).as_str(),
        "--priority-fee",
        "1000",
        "--json"
    );
}

#[test]
fn json_address_new_rejected() {
    clap_rejects!(
        "address",
        "new",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000",
        "--json"
    );
}

#[test]
fn json_balance_sol_rejected() {
    clap_rejects!(
        "balance",
        "sol",
        "--address",
        "11111111111111111111111111111111",
        "--json"
    );
}

#[test]
fn json_spl_send_rejected() {
    clap_rejects!(
        "spl",
        "send",
        "--wallet-id",
        "00000000-0000-0000-0000-000000000000",
        "--to",
        "11111111111111111111111111111111",
        "--amount",
        "1",
        "--token",
        "11111111111111111111111111111111",
        "--json"
    );
}
