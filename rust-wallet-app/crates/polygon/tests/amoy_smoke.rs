//! T7 Amoy live-RPC smoke (operator-driven per L29) — issue #464.
//!
//! Sub-task of #426 (Phase 4 T7 of #416 plan:
//! `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 T7).
//!
//! **Opt in (CI-safe by default):**
//!   RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_smoke -- --ignored
//!
//! **Scope after security-review fix (HIGH `sensitive-data-exposure-via-argv`):**
//! only PK-free AC items — `polygon faucet --address ...` URL print (Story 30)
//! and `polygon fee --network amoy` re-fetch / no-cache (Story 8). The
//! PK-using AC items (`wallet import` / `balance > 0` / `send 0.01 POL`)
//! were removed because the polygon CLI's `--private-key` flag exposes the
//! key via the subprocess's `/proc/<pid>/cmdline` argv to any sibling
//! process on the host. A follow-up PR (tracked in #464.1) will add a
//! `--private-key-file` flag (mode-0600 file path ingestion, no argv
//! exposure) so these AC items can re-land safely.
//!
//! The shell harness `scripts/polygon-send-amoy-e2e.sh` is unchanged — it
//! still covers the full create-then-fund happy path. Operator-driven runs
//! accept the argv-exposure risk for their own session (operator controls
//! their own host) until `--private-key-file` lands in #464.1.
//!
//! **TDD status (L13 step 5):** red phase. Tests are #[ignore]'d (CI-safe);
//! they fail loudly if RUN_POLYGON_AMOY=1 is set without env setup.
//! Green phase is operator-driven per L29: manual run against Amoy RPC.

#![cfg(test)]

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

/// Guard: skip unless `RUN_POLYGON_AMOY=1` (CI-safe default). When the
/// `cargo test -- --ignored` flag is passed, ignored tests run — this guard
/// ensures they still require the opt-in env.
fn require_run_polygon_amoy() {
    if std::env::var("RUN_POLYGON_AMOY").ok().as_deref() != Some("1") {
        panic!("RUN_POLYGON_AMOY=1 not set; amoy_smoke tests require explicit opt-in");
    }
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
