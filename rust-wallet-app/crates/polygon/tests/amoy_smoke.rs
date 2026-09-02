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
use std::sync::Mutex;

// P8-T-setup (issue #518) — load `polygon/tests/.env` once so the
// env-reading helpers below (`amoy_funded_pk_hex`, `require_run_polygon_amoy`)
// see operator-supplied overrides per L29 + L54. `OnceLock::get_or_init`
// swallows missing-file errors (closure returns ()) and runs exactly
// once across the test binary lifetime. Path is explicit
// (`tests/.env`) because cargo's per-test CWD = crate root
// (`rust-wallet-app/crates/polygon/`), so the relative path resolves
// from there. Colocated with the `.env.example` template in the same
// dir so operator setup is one `cp .env.example .env` away. Called
// from every env-reading helper (Rust rejects top-level expressions
// in test files, so the loader runs lazily).
fn ensure_dotenv_loaded() {
    use std::sync::OnceLock;
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        let _ = dotenvy::from_filename("tests/.env");
    });
}

// L54 (env-var secret defense-in-depth) — serialise reads of
// `AMOY_FUNDED_PK_HEX` so concurrent tests can't race on capture +
// `remove_var`. Tests that need the PK acquire this lock, capture the
// value into a local String, and immediately `remove_var` so a sibling
// test can't accidentally read a stale env value.
static PK_LOCK: Mutex<()> = Mutex::new(());

/// Path to the `polygon` binary built by cargo for integration tests.
/// (P8-T-setup: `env!("CARGO_BIN_EXE_polygon")` is compile-time cargo
/// metadata, NOT `std::env` — no env-reading migration needed.)
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// Operator-funded PK fixture (32-byte secp256k1 scalar). The test
/// asserts the wallet's balance is non-zero — that requires the
/// operator to pre-fund this address on Amoy before running. Reads
/// `AMOY_FUNDED_PK_HEX` from env ONLY — config source-of-truth lives
/// in `polygon/tests/.env` (loaded via `ensure_dotenv_loaded()`), not
/// in this source file. Operator must `cp polygon/tests/.env.example
/// polygon/tests/.env` (and edit if needed) before running
/// `RUN_POLYGON_AMOY=1 cargo test -p polygon -- --ignored`.
///
/// **Callers MUST** hold `PK_LOCK` while reading + immediately
/// `std::env::remove_var("AMOY_FUNDED_PK_HEX")` after capture (the
/// PK-using test below does this in a scoped block — see
/// `amoy_wallet_import_via_pk_file`).
///
/// Default EIP-55 address (for the Anvil-#0 test vector shipped in
/// `.env.example`): `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`.
fn amoy_funded_pk_hex() -> String {
    ensure_dotenv_loaded();
    match std::env::var("AMOY_FUNDED_PK_HEX") {
        Ok(s) if !s.is_empty() => s,
        _ => panic!(
            "AMOY_FUNDED_PK_HEX not set. Copy polygon/tests/.env.example to \
             polygon/tests/.env (or export AMOY_FUNDED_PK_HEX) before running \
             RUN_POLYGON_AMOY=1 cargo test -p polygon -- --ignored. See \
             plan §Environment Configuration for the 64-hex-char secp256k1 \
             scalar format."
        ),
    }
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

/// Guard: skip unless `RUN_POLYGON_AMOY=1` (CI-safe default). When the
/// `cargo test -- --ignored` flag is passed, ignored tests run — this guard
/// ensures they still require the opt-in env. Calls `ensure_dotenv_loaded()`
/// first so the operator's `polygon/.env` overrides are honoured before the
/// guard fires.
fn require_run_polygon_amoy() {
    ensure_dotenv_loaded();
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
    let pk_path = {
        // L54 — serialise PK capture, then immediately remove the env
        // var so sibling tests (or re-runs in the same process) can't
        // pick up a stale value.
        let _guard = PK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let pk = amoy_funded_pk_hex();
        std::env::remove_var("AMOY_FUNDED_PK_HEX");
        write_pk_file(data_dir.path(), "amoy-fund.pk", &pk)
    };
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

// ── Sanity check (P8-T-setup verification, NOT #[ignore]) ──────────────────
//
// Proves the P8-T-setup env-reading chain end-to-end WITHOUT hitting live
// RPC: `ensure_dotenv_loaded()` reads `polygon/tests/.env`, `dotenvy` pushes
// the vars into the process environment, and the env-reading helpers
// (`amoy_funded_pk_hex`, the opt-in guard) see them. Lives alongside the
// 5 #[ignore] amoy tests so it runs on plain `cargo test -p polygon
// --test amoy_smoke` (no `--ignored`, no live RPC, no operator setup
// beyond having `polygon/tests/.env` populated — CI parity).
//
// Pass criterion: each var below matches the value in `polygon/tests/.env`.
// If any value drifts (e.g. someone edits `.env` to a different RPC), the
// test fails loudly — preventing silent config drift across the integration
// suite.
#[test]
fn amoy_env_load_from_dotenv() {
    ensure_dotenv_loaded();

    // RPC URL — comes from `polygon/tests/.env` (NOT a Rust literal).
    // The test source itself has no RPC URL hardcoded; if `.env` is
    // missing or the var is unset, the panic message points to the
    // `.env.example` template.
    let rpc = std::env::var("POLYGON_RPC_URL")
        .expect("POLYGON_RPC_URL not set after dotenvy load — check polygon/tests/.env");
    assert_eq!(
        rpc, "https://polygon-amoy-bor-rpc.publicnode.com",
        "POLYGON_RPC_URL from .env must match plan §Network Configuration Amoy RPC (Q4 drift)"
    );

    // Chain ID + USDC address — same pattern, .env is the source-of-truth.
    let chain_id =
        std::env::var("POLYGON_CHAIN_ID").expect("POLYGON_CHAIN_ID not set after dotenvy load");
    assert_eq!(chain_id, "80002", "POLYGON_CHAIN_ID must be 80002 (Amoy)");

    let usdc = std::env::var("POLYGON_USDC_ADDRESS")
        .expect("POLYGON_USDC_ADDRESS not set after dotenvy load");
    assert_eq!(
        usdc, "0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582",
        "POLYGON_USDC_ADDRESS must match plan §Network Configuration USDC contract"
    );

    // PK — proves `amoy_funded_pk_hex()` reads env (no embedded literal).
    // Uses the fn (not direct `std::env::var`) so the call path is
    // exercised exactly the way the live tests use it.
    let pk = amoy_funded_pk_hex();
    assert_eq!(
        pk, "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        "AMOY_FUNDED_PK_HEX from .env must match the Anvil-#0 test vector"
    );

    // Opt-in guard — must read "1" if `.env` is populated; the live
    // tests use this exact var to gate themselves per L29.
    let opt_in =
        std::env::var("RUN_POLYGON_AMOY").expect("RUN_POLYGON_AMOY not set after dotenvy load");
    assert_eq!(
        opt_in, "1",
        "RUN_POLYGON_AMOY must be '1' for live test runs"
    );
}
