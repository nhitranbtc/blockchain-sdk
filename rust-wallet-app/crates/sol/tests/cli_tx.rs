//! `sol` CLI integration tests — tx command surface.
//!
//! Phase 7 verification — clap-level rejects for `tx get` + `tx wait`. The
//! `Wait` variant is the carrier of the P7-11 timeout bound (1..=600s), which
//! is enforced by clap's `value_parser` rather than a runtime guard, so these
//! tests are the gate that catches regressions in the boundary. Surfpool-gated
//! poll-for-confirm e2e lives in `cli_integration_surfpool.rs`.

use std::net::TcpListener;

use assert_cmd::Command;
use predicates::prelude::{predicate, PredicateBooleanExt};
use predicates::str::contains;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

// A 94-character base58 string — long enough to pass clap's argument
// parsing. It is NOT a parseable Solana signature (those are 64 bytes, ~87
// base58 characters), so it stops at the signature parse rather than
// reaching the RPC. That is the point for the tests that use it: they assert
// only that clap accepted the flag set. Tests that must reach the RPC need a
// parseable signature, e.g. `"5".repeat(87)`.
const VALID_BASE58_SIG: &str =
    "5555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555555";

#[test]
fn tx_get_requires_signature() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("get")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn tx_get_invalid_signature_base58_rejects() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("get")
        .arg("--sig")
        .arg("not-a-base58-sig!!!")
        .assert()
        .failure()
        .stderr(contains("invalid signature base58"));
}

#[test]
fn tx_get_with_json_flag_passes_clap() {
    // `--json` is a real flag on `tx get`. Clap should accept it; runtime
    // will then fail (no RPC configured in tmpdir), but the failure must NOT
    // be a clap usage error. The exit code cannot carry that signal — clap's
    // usage error is 2 and the shared `wallet-errors` table also maps
    // `Network` to 2 — so clap's `Usage:` banner on stderr is the check.
    //
    // This test therefore asserts exactly one thing: clap did not reject the
    // arguments. It deliberately does NOT pin the exit code — a change to the
    // command's exit code is not caught here.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("get")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:").not());
}

#[test]
fn tx_wait_requires_signature() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn tx_wait_invalid_signature_base58_rejects() {
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg("not-a-base58-sig!!!")
        .assert()
        .failure()
        .stderr(contains("invalid signature base58"));
}

#[test]
fn tx_wait_timeout_zero_rejects_p7_11() {
    // P7-11: timeout bound is `1..=600` enforced by clap `value_parser`.
    // Anything below 1 must exit 1 (a clap usage error is a `User` error)
    // before any process startup.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("0")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn tx_wait_timeout_above_max_rejects_p7_11() {
    // P7-11 upper bound: 600s. 601s must be rejected at clap level.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("601")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn tx_wait_timeout_boundary_max_passes_clap_p7_11() {
    // P7-11 inclusive upper bound: 600s accepted by clap. Runtime then fails
    // (no RPC) but the failure must NOT be a clap usage error. The exit code
    // cannot carry that signal — clap's usage error is 2 and the shared
    // `wallet-errors` table also maps `Network` to 2 — so clap's `Usage:`
    // banner on stderr is the check.
    //
    // This test therefore asserts exactly one thing: clap did not reject the
    // arguments. It deliberately does NOT pin the exit code — a change to the
    // command's exit code is not caught here.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("600")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:").not());
}

#[test]
fn tx_wait_timeout_negative_rejects_p7_11() {
    // clap `value_parser!(u64)` rejects negative integers at parse time.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        .arg(VALID_BASE58_SIG)
        .arg("--timeout")
        .arg("-1")
        .assert()
        .failure()
        .code(1);
}

/// Exit 2 is `Network` — the code an automation rule retries on — so the
/// *default* suite must pin it. After the `ne(2)` lock-down guards were
/// replaced with clap's `Usage:` banner, the only assertion of exit 2 left
/// was the `#[ignore]`d surfpool gate, which does not run by default.
///
/// `tx wait` against a closed port surfaces `Error::Transport` (per
/// `chain/client.rs:267` — all RpcClient POST failures wrap as
/// `Error::Transport(...)`), which the classifier maps to `Network` → exit 2.
/// The connection is refused immediately, so the test fails fast.
///
/// The port is obtained by binding an ephemeral port and dropping the
/// listener, so the refusal is guaranteed on any OS — `127.0.0.1:1` is
/// `tcpmux` and may be bound on hardened CI images.
#[test]
fn tx_wait_unreachable_rpc_exits_network_two() {
    let closed_port = {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = l.local_addr().expect("local_addr").port();
        drop(l);
        port
    };
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("tx")
        .arg("wait")
        .arg("--sig")
        // 87 chars of `5` decode to a 64-byte signature (the same fixture
        // the surfpool gate uses). `VALID_BASE58_SIG` is 94 chars and is
        // rejected by the base58 length check before the RPC is reached.
        .arg("5".repeat(87).as_str())
        .arg("--timeout")
        .arg("1")
        .arg("--rpc")
        .arg(format!("http://127.0.0.1:{closed_port}"))
        .assert()
        .failure()
        .code(2)
        .stderr(contains("RPC POST getSignatureStatuses"));
}
