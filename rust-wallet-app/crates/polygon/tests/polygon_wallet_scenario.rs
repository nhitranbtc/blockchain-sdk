//! Issue #438 / Phase 4 T7 prep — polygon CLI wallet-subcommand
//! end-to-end scenario against a local Anvil node.
//!
//! Per `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4
//! T7 prep + `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`.
//!
//! **Scope (per #438 acceptance):** `polygon wallet {create,list,balance,send}` only.
//! `tx` / `erc20` / `fee` / `sign` are out of scope (separate tracking).
//!
//! **Why NOT `#[ignore]`'d:** #438 explicitly requires the Anvil leg to
//! gate in CI ("Anvil path runs in CI (< 60s wall-clock)"). The Amoy-fork
//! leg is operator-driven per L29 and runs through
//! `scripts/polygon-wallet-scenario.sh --env=amoy-fork` instead.
//!
//! **TDD status (L13 step 3):** this test is the failing seed for #438.
//! It currently fails because:
//!   1. `WalletAction::Create` and `WalletAction::Import` are still stubs
//!      in `src/main.rs:169-170` (T6c4 handler bodies land, but the
//!      `match` arm in `run()` is not yet wired) — `polygon wallet create`
//!      exits 1 with `Error::Rpc("wallet create: deferred past T6b …")`.
//!   2. `alloy-node-bindings` + `alloy-provider` are not yet in
//!      `[dev-dependencies]` of `polygon/Cargo.toml`.
//!
//! The GREEN step wires `main.rs::run()` to `handlers::wallet::wallet_create` + `wallet_import` and adds the dev-deps so this scenario can spawn Anvil in-process.
//!
//! Bugs surfaced (and fixed) by this scenario:
//! `WalletAction::{Balance,Sync}.address` field type mismatch
//! (`value_parser = parse_address` returns `Address` but field was
//! `String` — downcast panic on every invocation); `SendArgs.to` same
//! type mismatch; `wallet_list` `Path::extension` filter
//! (`xxx.meta.json` → `"json"` not `"meta.json"`, so the function
//! returned empty); `WalletManager::scan_disk_into` allowlist
//! silently dropped polygon subdirs → in-memory cache empty after
//! restart → `unlock_signer` `NotFound`.

#![cfg(test)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

use alloy_node_bindings::{Anvil, AnvilInstance};
use alloy_provider::Provider;
use tempfile::TempDir;

/// Path to the `polygon` binary built by cargo for integration tests.
///
/// `CARGO_BIN_EXE_<name>` is set by cargo when building integration
/// tests for a crate that has a `[[bin]]` named `<name>` — see
/// <https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates>.
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// Invoke `polygon` as a subprocess with hermetic env.
///
/// `POLYGON_PASSWORD` is the env-source for `resolve_password`; setting
/// it here avoids the TTY-prompt branch (which would block under
/// non-interactive CI). The CLI's kernel removes the var immediately
/// after read (L54) so leftover env on this side does not bleed into
/// later assertions.
fn run_polygon(args: &[&str], data_dir: &std::path::Path, rpc_url: &str) -> std::process::Output {
    Command::new(polygon_bin())
        .args(args)
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--rpc-url")
        .arg(rpc_url)
        .env("POLYGON_PASSWORD", "test-pw-ignore-leak")
        .env("POLYGON_NETWORK", "amoy")
        .env("RUST_BACKTRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn polygon binary")
}

/// Find the `<uuid>.meta.json` written by `wallet_create` and parse
/// out the address (the test funds this address so the subsequent
/// `wallet send` has a balance to spend).
///
/// `WalletMeta` JSON layout per `evm-wallet-core::wallet::WalletMeta`:
/// `{ "wallet_id": "<uuid>", "name": "...", "network": ...,
///    "address": "0x...", "derivation_path": "...", "created_at_secs": N }`.
/// Field names verified at `crates/evm-wallet-core/src/wallet.rs:WalletMeta`.
fn read_first_address(data_dir: &std::path::Path, name: &str) -> String {
    // `PolygonChain::as_dir_name()` returns "polygon_amoy" / "polygon_mainnet"
    // (NOT just "amoy"/"mainnet") — see `crates/evm-wallet-core/src/network.rs:220`.
    let network_dir = data_dir.join("polygon_amoy");
    let mut meta_path: Option<PathBuf> = None;
    for entry in std::fs::read_dir(&network_dir).expect("read amoy dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        // Match the full filename suffix `.meta.json` — `Path::extension`
        // returns only the LAST component (`json` for `xxx.meta.json`).
        // Mirrors `evm-wallet-core/src/wallet.rs:list_wallets` which
        // does the same `.ends_with(META_EXT)` check.
        let is_meta = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.ends_with(".meta.json"));
        if !is_meta {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read meta.json");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse meta.json as JSON");
        if v.get("name").and_then(|n| n.as_str()) == Some(name) {
            meta_path = Some(path);
            break;
        }
    }
    let meta_path = meta_path.expect("wallet meta.json not found for name");
    let bytes = std::fs::read(&meta_path).expect("re-read meta.json");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse meta.json");
    v.get("address")
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
        .expect("address field in meta.json")
}

/// Fund `address` on the Anvil instance via the raw `anvil_setBalance`
/// RPC. Anvil default-prefunded accounts are not the wallet just
/// created by `polygon wallet create`, so we must top up the new
/// wallet's address before `wallet send` has any balance to spend.
async fn anvil_set_balance(anvil: &AnvilInstance, address: &str, wei_hex: &str) {
    let endpoint: alloy_transport_http::reqwest::Url =
        anvil.endpoint().parse().expect("valid Anvil endpoint");
    let provider = alloy_provider::ProviderBuilder::new().connect_http(endpoint);
    provider
        .raw_request::<_, ()>("anvil_setBalance".into(), (address, wei_hex))
        .await
        .expect("anvil_setBalance must succeed");
}

/// Story 1 + 9 + 3 + 5 — full happy-path wallet lifecycle against Anvil.
///
/// Asserts (per #438 acceptance):
///   - `wallet create` exits 0 and stdout mentions the new wallet
///   - `wallet list` shows ≥ 1 wallet
///   - `wallet balance <addr>` returns a numeric balance string
///   - `wallet send` returns `0x…` 64-char tx hash + receipt status `1`
///     (status `1` is checked indirectly: `wallet_send_native_v2`
///     returns the B256 only after `get_transaction_receipt` succeeds,
///     per its impl in `handlers/wallet.rs`.)
#[tokio::test]
async fn polygon_wallet_scenario_anvil_full_lifecycle() {
    // Spawn Anvil with Polygon Amoy chain_id (80002) so the wallet
    // created with `--network amoy` matches the RPC chain (handlers
    // verify chain_id at send time per L13 critical-tier review).
    let anvil = Anvil::new().chain_id(80_002).spawn();
    let rpc_url = anvil.endpoint().clone();
    let data_dir = TempDir::new().expect("tempdir for data-dir");

    // 1) wallet create — Story 1
    let out = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "alice",
            "--password",
            "test-pw-ignore-leak",
        ],
        data_dir.path(),
        &rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet create failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let create_stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        create_stdout.contains("alice"),
        "wallet create stdout should mention wallet name; got: {create_stdout}"
    );

    // Parse alice's first address out of <data_dir>/amoy/<uuid>.meta.json
    // so we can fund + balance + send against it.
    let alice_addr = read_first_address(data_dir.path(), "alice");

    // Fund alice on Anvil — 10 POL = 10 * 10^18 wei.
    let ten_pol_wei = "0x".to_string() + &format!("{:x}", 10_u128 * 10_u128.pow(18));
    anvil_set_balance(&anvil, &alice_addr, &ten_pol_wei).await;

    // 2) wallet list — Story 9 (must show alice after create)
    let out = run_polygon(
        &["wallet", "list", "--network", "amoy"],
        data_dir.path(),
        &rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet list failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let list_stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        list_stdout.contains("alice"),
        "wallet list should contain 'alice'; got: {list_stdout}"
    );

    // 3) wallet balance — Story 3 (must return numeric string)
    // Address is a required `--address` flag, not positional (see
    // `WalletAction::Balance` in cli.rs).
    let out = run_polygon(
        &[
            "wallet",
            "balance",
            "--address",
            &alice_addr,
            "--unit",
            "wei",
        ],
        data_dir.path(),
        &rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet balance failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let balance_stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // CLI formats as `<amount> <unit>` (e.g. "10000000000000000000 wei").
    // Strip the trailing unit token before parsing the numeric prefix.
    let numeric_part = balance_stdout
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();
    assert!(
        numeric_part.parse::<u128>().is_ok(),
        "wallet balance stdout should parse as u128 (wei); got: {balance_stdout:?}"
    );
    assert!(
        numeric_part.parse::<u128>().unwrap() >= 10 * 10_u128.pow(18) / 2,
        "alice balance should reflect the 10 POL we funded (minus any gas spent); got: {balance_stdout}"
    );

    // 4) wallet send — Story 5 (must return 0x + 64 hex chars + receipt status 1)
    let recipient = "0x0000000000000000000000000000000000000042";
    let out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            "alice",
            "--password",
            "test-pw-ignore-leak",
            "--to",
            recipient,
            "--amount",
            "0.001",
            "--unit",
            "pol",
        ],
        data_dir.path(),
        &rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet send failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let send_stdout = String::from_utf8_lossy(&out.stdout);
    let tx_hash_line = send_stdout
        .lines()
        .find(|l| l.starts_with("tx_hash: 0x"))
        .expect("wallet send stdout should contain 'tx_hash: 0x...' line");
    let tx_hash = tx_hash_line.trim_start_matches("tx_hash: 0x").trim();
    assert_eq!(
        tx_hash.len(),
        64,
        "tx_hash hex must be 64 chars (32 bytes); got: {tx_hash:?}"
    );
    assert!(
        tx_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "tx_hash must be hex; got: {tx_hash:?}"
    );
}

/// Negative case (per #438 acceptance): invalid `--to` recipient must
/// produce non-zero exit + human-readable error message.
#[tokio::test]
async fn polygon_wallet_send_invalid_recipient_errors_cleanly() {
    // Spawn Anvil with Polygon Amoy chain_id (80002) so the wallet
    // created with `--network amoy` matches the RPC chain (handlers
    // verify chain_id at send time per L13 critical-tier review).
    let anvil = Anvil::new().chain_id(80_002).spawn();
    let rpc_url = anvil.endpoint().clone();
    let data_dir = TempDir::new().expect("tempdir for data-dir");

    // Create + fund bob so the only failure surface is --to.
    let out = run_polygon(
        &[
            "wallet",
            "create",
            "--name",
            "bob",
            "--password",
            "test-pw-ignore-leak",
        ],
        data_dir.path(),
        &rpc_url,
    );
    assert!(
        out.status.success(),
        "wallet create (bob) failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bob_addr = read_first_address(data_dir.path(), "bob");
    let ten_pol_wei = "0x".to_string() + &format!("{:x}", 10_u128 * 10_u128.pow(18));
    anvil_set_balance(&anvil, &bob_addr, &ten_pol_wei).await;

    let out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            "bob",
            "--password",
            "test-pw-ignore-leak",
            "--to",
            "0xnotavalidaddress",
            "--amount",
            "0.001",
            "--unit",
            "pol",
            "--rpc-url",
            &rpc_url,
        ],
        data_dir.path(),
        &rpc_url,
    );
    assert!(
        !out.status.success(),
        "wallet send to invalid recipient must exit non-zero; got exit 0 with stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid") || stderr.contains("--to"),
        "stderr should mention invalid / --to; got: {stderr}"
    );
    // Quietly swallow unused-warning suppression — bob_addr + ten_pol_wei used above.
    let _ = (bob_addr, ten_pol_wei);
}
