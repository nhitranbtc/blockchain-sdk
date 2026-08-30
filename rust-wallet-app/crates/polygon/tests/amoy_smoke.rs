//! T7 Amoy live-RPC smoke (operator-driven per L29) — issues #464 + #469.
//!
//! Sub-task of #426 (Phase 4 T7 of #416 plan:
//! `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 T7).
//!
//! **Opt in (CI-safe by default):**
//!   RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_smoke -- --ignored
//!
//! **Scope (post #469 restore):** the 3 PK-using AC items that were
//! removed in #464 are restored here, now using the new
//! `--private-key-file` flag (mode-0600 file ingestion — no argv
//! exposure per L12 H-1 sister finding closed by PR #456 for
//! `--mnemonic`). The PK-free AC items remain: `polygon faucet`
//! URL print (Story 30) + `polygon fee --network amoy` no-cache
//! (Story 8).
//!
//! The shell harness `scripts/polygon-send-amoy-e2e.sh` is unchanged —
//! operator-driven runs accept argv exposure for their own session
//! (operator controls their own host) and prefer `--private-key-file`
//! when they migrate to #469's new path.
//!
//! **TDD status:** red by default (#[ignore] + `RUN_POLYGON_AMOY` guard).
//! Green is operator-driven per L29: manual run against Amoy RPC
//! requires operator-funded PK (see fixture below).

#![cfg(test)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Path to the `polygon` binary built by cargo for integration tests.
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// Operator-funded PK fixture (32-byte secp256k1 scalar). The test
/// asserts the wallet's balance is non-zero — that requires the
/// operator to pre-fund this address on Amoy before running. The PK
/// value is a deterministic test vector (not a real wallet's key);
/// operators can swap to their own funded PK without code changes.
///
/// Address (EIP-55): `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`
const AMOY_FUNDED_PK_HEX: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

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

/// Guard: skip unless `RUN_POLYGON_AMOY=1` (CI-safe default). When the
/// `cargo test -- --ignored` flag is passed, ignored tests run — this guard
/// ensures they still require the opt-in env.
fn require_run_polygon_amoy() {
    if std::env::var("RUN_POLYGON_AMOY").ok().as_deref() != Some("1") {
        panic!("RUN_POLYGON_AMOY=1 not set; amoy_smoke tests require explicit opt-in");
    }
}

/// Write the operator-funded PK to a mode-0600 file inside `dir`.
/// Returns the file path. Unix-only (Windows lacks `PermissionsExt`).
#[cfg(unix)]
fn write_pk_file(dir: &std::path::Path, name: &str, hex: &str) -> PathBuf {
    let path = dir.join(name);
    let bytes = hex.as_bytes();
    std::fs::write(&path, bytes).expect("write pk file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .expect("set pk file mode 0o600");
    path
}

/// Story 30 — `polygon faucet --address <addr> --network amoy` prints the
/// canonical Amoy faucet URL. No live RPC dependency for the print path
/// (CLI just prints the URL); the test asserts stdout contains the
/// expected faucet URL.
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

/// Story 8 — `polygon fee --network amoy` returns a fresh gas estimate on
/// each call (no cache between invocations). Asserts both calls return
/// parseable numeric estimates (values may legitimately match if no block
/// has elapsed between the two fetches).
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

// ============================================================
// #469 restored: 3 PK-using AC items that were removed from
// #464's partial land. All #[ignore] + RUN_POLYGON_AMOY=1 guard
// per L29 (operator-driven live RPC). Operator must pre-fund
// `AMOY_FUNDED_PK_HEX` on Amoy before running.
// ============================================================

/// Story 2 (PK variant) — `polygon wallet import --private-key-file
/// <mode-0600-path> --name w --password pw --network amoy` succeeds.
/// Sister pattern to `polygon-send-amoy-e2e.sh` but exercises the new
/// file-path flag (closes the L12 H-1 argv-exposure finding for PK
/// import).
#[test]
#[ignore]
#[cfg(unix)]
fn amoy_wallet_import_via_pk_file() {
    require_run_polygon_amoy();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");
    let pk_path = write_pk_file(data_dir.path(), "amoy-fund.pk", AMOY_FUNDED_PK_HEX);
    let out = run_polygon(
        &[
            "wallet",
            "import",
            "--name",
            "amoy-smoke-pk",
            "--private-key-file",
            pk_path.to_str().expect("tempdir path is utf-8 for tests"),
            "--network",
            "amoy",
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet import --private-key-file failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // CLI prints: `wallet imported: name=... id=<uuid> address=0x...`
    assert!(
        stdout.contains("wallet imported:"),
        "stdout should confirm import; got: {stdout}"
    );
    assert!(
        stdout.contains("address=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        "stdout should echo the canonical EIP-55 address of the funded PK; got: {stdout}"
    );
}

/// Story 4 — `polygon wallet balance --address <addr> --network amoy`
/// returns a numeric POL balance. The operator-funded PK must have a
/// non-zero balance for this assertion to pass; that's the operator's
/// pre-condition per L29.
#[test]
#[ignore]
fn amoy_balance_after_pk_wallet_import() {
    require_run_polygon_amoy();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");
    let out = run_polygon(
        &[
            "wallet",
            "balance",
            "--address",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "--network",
            "amoy",
            "--unit",
            "pol",
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet balance failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Output shape: `<value> POL` (human-readable per design §3.5).
    assert!(
        stdout.contains("POL"),
        "balance stdout should include POL unit; got: {stdout}"
    );
    // The operator-funded assertion: parse the leading numeric token
    // and require it > 0. `split_whitespace` already strips leading
    // whitespace, so no `trim_start` prefix is needed (clippy
    // `trim_split_whitespace` lint).
    let leading_num = stdout
        .split_whitespace()
        .next()
        .and_then(|t| t.parse::<f64>().ok());
    let value = leading_num.expect("balance stdout should start with a numeric value");
    assert!(
        value > 0.0,
        "expected operator-funded balance > 0 POL; got {value} from: {stdout}"
    );
}

/// Story 5 (sister to PK-file path) — `polygon wallet send` against an
/// operator-funded wallet. Sends 0.01 POL to a deterministic sink
/// address; asserts the CLI exits 0 and stdout confirms the tx hash.
/// `network amoy` default per cli.rs:165.
#[test]
#[ignore]
fn amoy_send_0_01_pol_after_pk_wallet_import() {
    require_run_polygon_amoy();
    let data_dir = tempfile::TempDir::new().expect("tempdir for data-dir");
    let out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            "amoy-smoke-pk",
            "--to",
            "0x0000000000000000000000000000000000000042",
            "--amount",
            "10000000000000000", // 0.01 POL in wei (design §3.5 unit = wei in handler)
            "--network",
            "amoy",
        ],
        data_dir.path(),
    );
    assert!(
        out.status.success(),
        "wallet send 0.01 POL failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("tx") || stdout.contains("0x"),
        "send stdout should confirm transaction; got: {stdout}"
    );
}
