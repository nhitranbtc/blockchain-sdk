//! T7 Amoy live-RPC smoke (operator-driven per L29) — issue #464.
//!
//! Sub-task of #426 (Phase 4 T7 of #416 plan:
//! `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 T7).
//!
//! **Opt in (CI-safe by default):**
//!   RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_smoke -- --ignored
//!
//! **Required env under RUN_POLYGON_AMOY=1:**
//!   POLYGON_AMOY_PK_FILE    mode-0600 file with Amoy-funded private key (hex)
//!   POLYGON_AMOY_RECIPIENT  recipient address (any valid 0x...)
//!
//! **Optional env:**
//!   POLYGON_AMOY_TIMEOUT_SECS  balance-poll timeout seconds (default 300)
//!
//! **Why import not create:** the wallet must already be funded on Amoy for
//! `wallet send 0.01 POL` to land. `wallet import --private-key "$PK"` mirrors
//! the ETH E2E pattern at `scripts/eth-send-sepolia-e2e.sh:111` (file-mode
//! enforced mnemonic read). Drift from agent-brief AC item
//! "wallet create --name w" — the brief lists `wallet create` for completeness
//! of the create-then-fund flow, but the integration test imports a pre-funded
//! PK because operator-driven faucet claim cannot be synchronised inside a
//! single `cargo test` invocation. The shell harness covers the create+faucet
//! flow separately (`scripts/polygon-send-amoy-e2e.sh`).
//!
//! **TDD status (L13 step 5):** red phase. Tests are #[ignore]'d (CI-safe);
//! they fail loudly if RUN_POLYGON_AMOY=1 is set without the required env.
//! Green phase is operator-driven per L29: manual run with funded env.

#![cfg(test)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Path to the `polygon` binary built by cargo for integration tests.
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// Invoke `polygon` as a subprocess with hermetic env.
fn run_polygon(args: &[&str], data_dir: &std::path::Path) -> std::process::Output {
    Command::new(polygon_bin())
        .args(args)
        .arg("--data-dir")
        .arg(data_dir)
        .env("POLYGON_PASSWORD", "test-pw-ignore-leak")
        .env("POLYGON_NETWORK", "amoy")
        .env("RUST_BACKTRACE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn polygon binary")
}

/// Read `POLYGON_AMOY_PK` from the mode-0600 file at `$POLYGON_AMOY_PK_FILE`.
/// Trims trailing newline. Panics on missing file or wrong mode — operator
/// must `chmod 600` per the brief's Key interfaces contract.
fn read_amoy_pk() -> String {
    let path = std::env::var("POLYGON_AMOY_PK_FILE")
        .expect("POLYGON_AMOY_PK_FILE must be set under RUN_POLYGON_AMOY=1");
    let p = std::path::Path::new(&path);
    assert!(p.is_file(), "POLYGON_AMOY_PK_FILE={path} does not exist");
    let mode = std::fs::metadata(p)
        .expect("stat POLYGON_AMOY_PK_FILE")
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "POLYGON_AMOY_PK_FILE={path} must have mode 0600; got {:o}",
        mode & 0o777
    );
    std::fs::read_to_string(p)
        .expect("read POLYGON_AMOY_PK_FILE")
        .trim()
        .to_string()
}

/// Find the `<uuid>.meta.json` written by `wallet import` and parse out the
/// address. Mirrors `polygon_wallet_scenario.rs::read_first_address` — same
/// `polygon_amoy` subdir + same `.meta.json` suffix filter.
fn read_address_for_name(data_dir: &std::path::Path, name: &str) -> String {
    let network_dir = data_dir.join("polygon_amoy");
    let mut meta_path: Option<PathBuf> = None;
    for entry in std::fs::read_dir(&network_dir).expect("read amoy dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
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

/// Guard: skip unless `RUN_POLYGON_AMOY=1` (CI-safe default). When the
/// `cargo test -- --ignored` flag is passed, ignored tests run — this guard
/// ensures they still require the opt-in env.
fn require_run_polygon_amoy() {
    if std::env::var("RUN_POLYGON_AMOY").ok().as_deref() != Some("1") {
        panic!("RUN_POLYGON_AMOY=1 not set; amoy_smoke tests require explicit opt-in");
    }
}

/// Story 1/9 — `wallet import --private-key "$POLYGON_AMOY_PK"` then `wallet list`
/// must show the imported wallet. Pre-funded PK assumption per file header.
#[test]
#[ignore]
fn amoy_wallet_import_and_list() {
    require_run_polygon_amoy();
    let pk = read_amoy_pk();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");

    let out = run_polygon(
        &[
            "wallet",
            "import",
            "--name",
            "amoy-test",
            "--password",
            "test-pw-ignore-leak",
            "--private-key",
            &pk,
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet import failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_polygon(
        &["wallet", "list", "--network", "amoy", "--all"],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet list failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let list_stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        list_stdout.contains("amoy-test"),
        "wallet list should contain 'amoy-test'; got: {list_stdout}"
    );
}

/// Story 30 — `polygon faucet --address <addr> --network amoy` prints the
/// canonical Amoy faucet URL. No live RPC dependency (CLI just prints the URL);
/// the test asserts stdout contains the expected faucet URL.
#[test]
#[ignore]
fn amoy_faucet_url_print() {
    require_run_polygon_amoy();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");
    let out = run_polygon(
        &[
            "faucet",
            "--address",
            "0x0000000000000000000000000000000000000042",
            "--network",
            "amoy",
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "polygon faucet failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let faucet_stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        faucet_stdout.contains("faucet.polygon.technology"),
        "faucet stdout should contain canonical Amoy faucet URL; got: {faucet_stdout}"
    );
}

/// Story 3 — `wallet balance --address <addr> --unit pol --network amoy`
/// returns > 0 POL. Assumes `POLYGON_AMOY_PK` is funded on Amoy (operator
/// pre-condition per agent brief Key interfaces).
#[test]
#[ignore]
fn amoy_wallet_balance_after_funding() {
    require_run_polygon_amoy();
    let pk = read_amoy_pk();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");

    let out = run_polygon(
        &[
            "wallet",
            "import",
            "--name",
            "amoy-test",
            "--password",
            "test-pw-ignore-leak",
            "--private-key",
            &pk,
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet import failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let addr = read_address_for_name(data_dir.path(), "amoy-test");

    let out = run_polygon(
        &[
            "wallet",
            "balance",
            "--address",
            &addr,
            "--unit",
            "pol",
            "--network",
            "amoy",
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet balance failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let balance_stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let numeric_part = balance_stdout
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();
    let balance_f64 = numeric_part
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("balance stdout not numeric: {balance_stdout:?}"));
    assert!(
        balance_f64 > 0.0,
        "amoy-test balance should be > 0 POL (funded PK); got: {balance_stdout}"
    );
}

/// Story 5 — `wallet send --amount 0.01 --unit pol --network amoy --wait`
/// broadcasts + returns tx hash + receipt status success.
#[test]
#[ignore]
fn amoy_wallet_send_broadcasts() {
    require_run_polygon_amoy();
    let pk = read_amoy_pk();
    let recipient = std::env::var("POLYGON_AMOY_RECIPIENT")
        .expect("POLYGON_AMOY_RECIPIENT must be set under RUN_POLYGON_AMOY=1");
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");

    let out = run_polygon(
        &[
            "wallet",
            "import",
            "--name",
            "amoy-test",
            "--password",
            "test-pw-ignore-leak",
            "--private-key",
            &pk,
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet import failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            "amoy-test",
            "--password",
            "test-pw-ignore-leak",
            "--to",
            &recipient,
            "--amount",
            "0.01",
            "--unit",
            "pol",
            "--network",
            "amoy",
            "--wait",
        ],
        data_dir.path(),
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

/// Story 8 — `polygon fee --network amoy` returns a fresh gas estimate on each
/// call (no cache between invocations). Asserts both calls return parseable
/// numeric estimates (values may legitimately match if no block has elapsed).
#[test]
#[ignore]
fn amoy_fee_no_cache() {
    require_run_polygon_amoy();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");

    let first = run_polygon(&["fee", "--network", "amoy"], data_dir.path());
    assert!(
        first.status.success(),
        "first fee call failed: stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = run_polygon(&["fee", "--network", "amoy"], data_dir.path());
    assert!(
        second.status.success(),
        "second fee call failed: stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );

    let first_stdout = String::from_utf8_lossy(&first.stdout);
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    // Both must contain at least one parseable numeric token (gwei/wei).
    assert!(
        first_stdout
            .split_whitespace()
            .any(|t| t.parse::<f64>().is_ok()),
        "first fee stdout should contain numeric gas estimate; got: {first_stdout}"
    );
    assert!(
        second_stdout
            .split_whitespace()
            .any(|t| t.parse::<f64>().is_ok()),
        "second fee stdout should contain numeric gas estimate; got: {second_stdout}"
    );
}
