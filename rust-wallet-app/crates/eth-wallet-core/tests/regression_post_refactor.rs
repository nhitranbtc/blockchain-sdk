//! T8 ETH mainnet regression smoke (operator-driven per L29) — issue #458.
//!
//! Sub-task of #426 (Phase 4 T8 of #416 plan:
//! `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 T8).
//!
//! **Opt in (CI-safe by default):**
//!   RUN_ETH_MAINNET=1 cargo test -p eth-wallet-core --test regression_post_refactor -- --ignored
//!
//! **Scope:** AC2 from issue #458 — confirm `eth wallet balance --address
//! <addr> --network mainnet` still works post Phase 0 refactor of
//! `eth-wallet-core` into `evm-wallet-core` (Q1 Option A). The regression
//! proves the ETH CLI surface area was preserved when the implementation
//! moved to `evm-wallet-core` and `eth-wallet-core` was demoted to
//! `pub use evm_wallet_core::*` re-exports (PR #304, Issue #416).
//!
//! **Why this test lives here (file-path drift from sister pattern):**
//! the sister `polygon/tests/amoy_smoke.rs` invokes the `polygon` binary
//! via `env!("CARGO_BIN_EXE_polygon")`. `eth-wallet-core` has no `[[bin]]`
//! (it's a pure re-export shell), so `CARGO_BIN_EXE_eth` is undefined from
//! this crate's perspective. We locate the `eth` binary at runtime via
//! `CARGO_MANIFEST_DIR`-relative path resolution (debug + release profile).
//! Alternative would be to relocate this test to `crates/eth/tests/` —
//! keeping it here honors issue #458's stated file path; the cost is the
//! manual path resolution below.
//!
//! **TDD status:** red by default (`#[ignore]` + `RUN_ETH_MAINNET=1` guard).
//! Green is operator-driven per L29.

#![cfg(test)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Locate the `eth` CLI binary in the workspace `target/` directory.
/// Builds in debug profile by default; falls back to release if debug
/// is missing (covers `cargo test --release` invocations).
fn eth_bin() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // `manifest_dir` = `rust-wallet-app/crates/eth-wallet-core/`. The cargo
    // workspace root (where `target/` lives) is two `.parent()` calls up:
    //   `crates/eth-wallet-core/` → `crates/` → `rust-wallet-app/` (workspace root)
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    // Try debug first; fall back to release so `--release` test runs work.
    let debug = workspace_root.join("target").join("debug").join("eth");
    if debug.exists() {
        debug
    } else {
        workspace_root.join("target").join("release").join("eth")
    }
}

/// Guard: skip unless `RUN_ETH_MAINNET=1` (CI-safe default).
fn require_run_eth_mainnet() {
    if std::env::var("RUN_ETH_MAINNET").ok().as_deref() != Some("1") {
        panic!("RUN_ETH_MAINNET=1 not set; regression_post_refactor tests require explicit opt-in");
    }
}

/// Resolve ETH mainnet RPC URL — operator can override via `ETH_RPC_URL`
/// env var when the default (`cloudflare-eth.com`) is rate-limited or
/// requires an API key. Sister to the `POLYGON_RPC_URL` override in
/// `polygon/tests/mainnet_smoke.rs` (drift from plan §T2).
fn eth_mainnet_rpc_url() -> String {
    if let Ok(override_url) = std::env::var("ETH_RPC_URL") {
        if !override_url.is_empty() {
            return override_url;
        }
    }
    eth_wallet_core::Network::Ethereum(eth_wallet_core::EthereumChain::Mainnet)
        .rpc_url()
        .to_string()
}

/// AC2 — `eth wallet balance --address <addr> --network mainnet` still works
/// post-refactor. Pre-refactor: `eth-wallet-core` carried the implementation;
/// post-refactor (Phase 0 of #416): `eth-wallet-core` is `pub use
/// evm_wallet_core::*` and the `eth` CLI consumes `eth-wallet-core` which
/// re-exports from `evm-wallet-core`. This test confirms the re-export
/// chain preserves the user-visible CLI surface.
///
/// We use a well-known Ethereum address with a verifiable mainnet balance
/// (the ETH2 deposit contract address — public, non-operator).
#[test]
#[ignore]
fn eth_mainnet_wallet_balance_works_post_refactor() {
    require_run_eth_mainnet();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");
    let rpc_url = eth_mainnet_rpc_url();
    let out = Command::new(eth_bin())
        .args([
            "wallet",
            "balance",
            "--address",
            "0x00000000219ab540356cBB839Cbe05303d7705Fa", // ETH2 deposit contract
            "--network",
            "mainnet",
            "--unit",
            "eth",
            "--rpc-url",
            &rpc_url,
        ])
        .arg("--data-dir")
        .arg(data_dir.path())
        .env("ETH_PASSWORD", "test-pw-ignore-leak")
        .env("RUST_BACKTRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn eth binary");
    assert!(
        out.status.success(),
        "eth wallet balance --network mainnet failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Output shape: `<value> ETH` (human-readable per design §3.5).
    assert!(
        stdout.contains("ETH"),
        "balance stdout should include ETH unit; got: {stdout}"
    );
    let leading_num = stdout
        .split_whitespace()
        .next()
        .and_then(|t| t.parse::<f64>().ok());
    let value = leading_num.expect("balance stdout should start with a numeric value");
    assert!(
        value > 0.0,
        "expected real mainnet ETH balance > 0; got {value} from: {stdout}"
    );
}
