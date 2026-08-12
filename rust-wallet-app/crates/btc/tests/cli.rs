//! Integration tests for `btc` CLI.
//!
//! Run via `cargo test -p btc --test cli`. Live-Esplora tests
//! marked `#[ignore]` per L29 (operator-driven).
//!
//! **L28 regression coverage**: `create_routes_mnemonic_to_stderr_not_stdout`
//! enforces F49 (mnemonic never on STDOUT) — the regression test the
//! issue body mandates.

use std::process::{Command, Stdio};

/// Subprocess runner. `CARGO_BIN_EXE_btc` is set by cargo for
/// integration tests; resolves to the just-built `btc` binary.
fn btc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_btc"))
}

/// Run `btc wallet create` against a fresh tempdir (XDG_DATA_HOME
/// override) and return the captured output + status.
fn run_create(
    words: &str,
    network: &str,
    password: &str,
    data_home: &std::path::Path,
) -> std::process::Output {
    btc()
        .args([
            "wallet",
            "create",
            "--words",
            words,
            "--network",
            network,
            "--password",
            password,
        ])
        .env("XDG_DATA_HOME", data_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn btc wallet create")
}

#[test]
fn wallet_create_help_exits_zero() {
    let out = btc()
        .args(["wallet", "create", "--help"])
        .output()
        .expect("spawn btc");
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}; stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--words"),
        "--words flag missing from help: {stdout}"
    );
    assert!(
        stdout.contains("--network"),
        "--network flag missing from help: {stdout}"
    );
    assert!(
        stdout.contains("--password"),
        "--password flag missing from help: {stdout}"
    );
}

#[test]
fn wallet_show_help_exits_zero() {
    let out = btc()
        .args(["wallet", "show", "--help"])
        .output()
        .expect("spawn btc");
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}; stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--network"),
        "--network flag missing from help: {stdout}"
    );
    assert!(
        stdout.contains("--password"),
        "--password flag missing from help: {stdout}"
    );
}

#[test]
fn wallet_create_unknown_subcommand_fails() {
    let out = btc()
        .args(["wallet", "frobnicate"])
        .output()
        .expect("spawn btc");
    assert!(!out.status.success(), "unknown subcommand should fail");
}

/// L28 regression test — F49 closure.
///
/// Asserts:
/// - 12-word mnemonic appears on STDERR (operator reads it once)
/// - mnemonic NEVER appears on STDOUT (would leak via shell history,
///   logs, CI capture, etc.)
/// - wallet_id (UUID) appears on STDOUT (scriptable output)
#[test]
fn create_routes_mnemonic_to_stderr_not_stdout() {
    let temp = tempfile::tempdir().expect("tempdir");
    let out = run_create("12", "testnet", "test-password", temp.path());
    assert!(
        out.status.success(),
        "create should succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Locate the 12-word line on stderr (BIP-39 phrase).
    let mnemonic_line = stderr
        .lines()
        .find(|l| l.split_whitespace().count() == 12)
        .unwrap_or_else(|| {
            panic!("expected 12-word mnemonic line on stderr; stderr was:\n{stderr}")
        });
    let mnemonic = mnemonic_line.trim();

    // Mnemonic MUST NOT appear on stdout.
    assert!(
        !stdout.contains(mnemonic),
        "L28 regression: mnemonic leaked to STDOUT\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // UUID v4 wallet_id MUST appear on stdout (scriptable).
    assert!(
        stdout.contains('-') && stdout.trim().len() >= 36,
        "wallet_id (UUID) expected on STDOUT; stdout was:\n{stdout}"
    );
}

/// Persistence integration test — verifies the wallet blob lands at
/// the ADR 0001 path layout under XDG_DATA_HOME.
#[test]
fn create_persists_wallet_blob_to_xdg_data_home() {
    let temp = tempfile::tempdir().expect("tempdir");
    let out = run_create("12", "testnet", "test-password", temp.path());
    assert!(out.status.success(), "create failed: {:?}", out.status);

    // ADR 0001: $XDG_DATA_HOME/btc/wallets/<network>/<wallet_id>.enc
    let blob_dir = temp.path().join("btc").join("wallets").join("testnet");
    assert!(
        blob_dir.is_dir(),
        "expected wallet dir at {}; ls: {:?}",
        blob_dir.display(),
        std::fs::read_dir(temp.path()).map(|d| d
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect::<Vec<_>>())
    );
    let blobs: Vec<_> = std::fs::read_dir(&blob_dir)
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("enc"))
        .collect();
    assert_eq!(
        blobs.len(),
        1,
        "expected exactly 1 .enc blob, found {}: {:?}",
        blobs.len(),
        blobs.iter().map(|e| e.path()).collect::<Vec<_>>()
    );
}

/// Live testnet roundtrip — `show` requires Esplora, gated behind
/// L29 manual smoke. Excluded from default `cargo test` runs.
#[test]
#[ignore = "requires live testnet Esplora; run via `cargo test --ignored -p btc` per L29"]
fn create_then_show_roundtrips_against_testnet() {
    let temp = tempfile::tempdir().expect("tempdir");
    let create_out = run_create("12", "testnet", "test-password", temp.path());
    assert!(create_out.status.success(), "create failed");
    let wallet_id = String::from_utf8_lossy(&create_out.stdout)
        .trim()
        .to_string();
    assert!(!wallet_id.is_empty(), "wallet_id missing from stdout");

    let show_out = btc()
        .args([
            "wallet",
            "show",
            &wallet_id,
            "--network",
            "testnet",
            "--password",
            "test-password",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ])
        .env("XDG_DATA_HOME", temp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn btc wallet show");
    assert!(
        show_out.status.success(),
        "show failed: stderr={}",
        String::from_utf8_lossy(&show_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&show_out.stdout);
    assert!(
        stdout.contains("receive_addresses"),
        "expected JSON keys on stdout: {stdout}"
    );
    assert!(
        stdout.contains("balance_sat"),
        "expected balance_sat on stdout: {stdout}"
    );
}

// -- wallet import (Issue #99 / Story 2) -----------------------------------

/// BIP-39 standard test vector (12-word, valid checksum).
const IMPORT_PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Run `btc wallet import` against a fresh tempdir + return captured output.
fn run_import(
    mnemonic: &str,
    network: &str,
    password: &str,
    data_home: &std::path::Path,
) -> std::process::Output {
    btc()
        .args([
            "wallet",
            "import",
            "--mnemonic",
            mnemonic,
            "--network",
            network,
            "--password",
            password,
        ])
        .env("XDG_DATA_HOME", data_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn btc wallet import")
}

#[test]
fn wallet_import_help_exits_zero() {
    let out = btc()
        .args(["wallet", "import", "--help"])
        .output()
        .expect("spawn btc");
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}; stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_accepts_valid_12_word_phrase_and_persists_blob() {
    let temp = tempfile::tempdir().expect("tempdir");
    let out = run_import(IMPORT_PHRASE, "testnet", "test-password", temp.path());
    assert!(
        out.status.success(),
        "import failed: {:?}; stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // Wallet_id on STDOUT (scriptable).
    let stdout = String::from_utf8_lossy(&out.stdout);
    let wallet_id = stdout.trim();
    assert!(!wallet_id.is_empty(), "expected wallet_id on stdout");
    // Basic UUID-shape sanity check (8-4-4-4-12 hex chars).
    let parts: Vec<&str> = wallet_id.split('-').collect();
    assert_eq!(parts.len(), 5, "expected UUID v4 shape, got: {wallet_id}");

    // ADR 0001 path: $XDG_DATA_HOME/btc/wallets/<network>/<wallet_id>.enc
    let blob_path = temp
        .path()
        .join("btc")
        .join("wallets")
        .join("testnet")
        .join(format!("{wallet_id}.enc"));
    assert!(
        blob_path.exists(),
        "expected blob at {}; ls: {:?}",
        blob_path.display(),
        std::fs::read_dir(temp.path().join("btc").join("wallets").join("testnet")).map(|d| d
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect::<Vec<_>>())
    );
}

#[test]
fn import_rejects_invalid_checksum() {
    let temp = tempfile::tempdir().expect("tempdir");
    // Same 12 words but with last word changed — checksum broken.
    let bad = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
    let out = run_import(bad, "testnet", "test-password", temp.path());
    assert!(
        !out.status.success(),
        "expected non-zero exit for invalid checksum; got {:?}; stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn import_does_not_echo_mnemonic_to_stdout() {
    // Story 2 AC: "Output does not echo the mnemonic back to the terminal."
    let temp = tempfile::tempdir().expect("tempdir");
    let out = run_import(IMPORT_PHRASE, "testnet", "test-password", temp.path());
    assert!(out.status.success(), "import failed: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Only wallet_id expected on stdout — no mnemonic anywhere.
    assert!(
        !stdout.contains("abandon"),
        "mnemonic leaked to stdout: {stdout}"
    );
    // STDERR is also clean of the mnemonic — we only print wallet_id
    // and a success line.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("abandon"),
        "mnemonic leaked to stderr: {stderr}"
    );
}

#[test]
fn import_then_create_produces_distinct_wallet_ids() {
    // Importing the same phrase twice yields two distinct WalletIds
    // (fresh UUID per import); the create flow generates a different
    // mnemonic entirely.
    let temp = tempfile::tempdir().expect("tempdir");
    let import_out = run_import(IMPORT_PHRASE, "testnet", "test-password", temp.path());
    assert!(import_out.status.success());
    let import_id = String::from_utf8_lossy(&import_out.stdout)
        .trim()
        .to_string();

    let create_out = run_create("12", "testnet", "test-password", temp.path());
    assert!(
        create_out.status.success(),
        "create failed: {:?}",
        create_out.status
    );
    let create_id = String::from_utf8_lossy(&create_out.stdout)
        .trim()
        .to_string();

    assert_ne!(
        import_id, create_id,
        "import + create should produce distinct wallet ids"
    );
}

#[test]
fn import_accepts_24_word_phrase() {
    let phrase24 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
    let temp = tempfile::tempdir().expect("tempdir");
    let out = run_import(phrase24, "bitcoin", "test-password", temp.path());
    assert!(
        out.status.success(),
        "24-word import failed: {:?}; stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}
