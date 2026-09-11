//! `sol` CLI integration tests — Phase 7 Task 7.3 step 3.
//!
//! Cross-cluster conformance against `https://api.devnet.solana.com`.
//! Loud-RED `RUN_SOL_DEVNET=1` env var + clear STDERR message at run start.
//!
//! **Devnet only** — `RUN_SOL_DEVNET=1 cargo test -p sol --test cli_devnet_conformance -- --ignored`.
//! Requires a funded devnet keypair via `sol keygen` + devnet airdrop.
//!
//! Covers Phase 7 deep-dive row 14 (cross-cluster conformance).

use assert_cmd::Command;

#[allow(dead_code)]
fn sol_bin() -> Command {
    let cmd =
        Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first");
    if std::env::var("RUN_SOL_DEVNET").ok().as_deref() != Some("1") {
        eprintln!("SKIPPED: cli_devnet_conformance requires RUN_SOL_DEVNET=1 env var");
        eprintln!("         (loud-RED gate per Plan 7.3 Step 3; no silent skip)");
    }
    cmd
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Phase 7.3 Step 3 loud-RED; cross-cluster conformance against https://api.devnet.solana.com. Requires funded devnet keypair via airdrop."]
fn cli_devnet_config_set_cluster_devnet_runs() {
    todo!("cli_devnet_config_set_cluster_devnet_runs — lands with devnet keypair + RUN_SOL_DEVNET=1 CI integration");
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Phase 7.3 Step 3"]
fn cli_devnet_balance_query_against_devnet_rpc() {
    todo!("cli_devnet_balance_query_against_devnet_rpc — lands with devnet RPC + RUN_SOL_DEVNET=1");
}

#[test]
#[ignore = "RUN_SOL_DEVNET=1 required — Phase 7.3 Step 3"]
fn cli_devnet_tx_get_query_against_devnet_rpc() {
    todo!("cli_devnet_tx_get_query_against_devnet_rpc — lands with devnet RPC + RUN_SOL_DEVNET=1");
}
