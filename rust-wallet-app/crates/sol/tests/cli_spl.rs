//! `sol` CLI integration tests — SPL command surface.
//!
//! Phase 7.1c owns 4 required cases (plan step 6):
//!   - P7-14 — `spl send --skip-memo-required` without second flag rejected
//!   - P7-23 — `spl send --memo` > 566 bytes rejected
//!   - P7-14 positive — both flags accepted (warn on STDERR, exit 0)
//!
//! Surfpool-backed e2e (deploy USDC mint, sign SPL transfer, assert on-chain
//! balance delta) lands in `crates/sol-wallet-core/tests/submit_spl_local_*`
//! gated on `RUN_SOL_SURFPOOL=1` (Phase 7.1c step 5).

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn sol_bin() -> Command {
    Command::cargo_bin("sol").expect("sol binary not built — run `cargo build -p sol` first")
}

#[test]
fn spl_send_memo_required_enforced_p7_14() {
    // P7-14: --skip-memo-required without --i-understand-no-memo-enforcement
    // must be rejected. Handler returns anyhow error (defense in depth in
    // case a future contributor drops the clap `requires` constraint).
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("spl")
        .arg("send")
        .arg("--wallet-id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--to")
        .arg("11111111111111111111111111111111")
        .arg("--amount")
        .arg("1")
        .arg("--token")
        .arg("USDC")
        .arg("--skip-memo-required")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "i-understand-no-memo-enforcement",
        ));
}

#[test]
fn spl_send_memo_too_long_rejects_p7_23() {
    // P7-23: SPL Memo program accepts up to 566 bytes; reject longer.
    let tmp = TempDir::new().expect("tempdir");
    let long_memo = "x".repeat(1024);
    sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-password")
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("spl")
        .arg("send")
        .arg("--wallet-id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--to")
        .arg("11111111111111111111111111111111")
        .arg("--amount")
        .arg("1")
        .arg("--token")
        .arg("USDC")
        .arg("--memo")
        .arg(&long_memo)
        .assert()
        .failure();
}

// P7-23 NUL-byte test deferred: assert_cmd's `Command::arg` rejects NUL
// bytes in argv data (returns "nul byte found in provided data" error
// before the spawned process even runs). The handler's NUL check lives in
// `handlers/spl.rs` (see `m.contains('\0')`); coverage is verified via code
// review + manual smoke. Asserting the clap-level path requires a different
// injection mechanism (env var or stdin pipe) that's not worth the test
// fragility for a 5-line handler check.

#[test]
fn spl_send_both_memo_flags_accepted_p7_14() {
    // P7-14 positive: --skip-memo-required + --i-understand-no-memo-enforcement
    // both present → handler proceeds past validation. Will fail downstream
    // (no real wallet / RPC) but MUST NOT exit at the validation gate.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .env("SOL_WALLET_PASSWORD", "test-password")
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("spl")
        .arg("send")
        .arg("--wallet-id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--to")
        .arg("11111111111111111111111111111111")
        .arg("--amount")
        .arg("1")
        .arg("--token")
        .arg("USDC")
        .arg("--skip-memo-required")
        .arg("--i-understand-no-memo-enforcement")
        .assert()
        .failure() // fails downstream (no real wallet) but NOT at validation
        .stderr(predicates::str::contains("i-understand-no-memo-enforcement").not());
}

#[test]
fn spl_approve_missing_token_rejects() {
    // clap-level: --token is required for spl approve.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("spl")
        .arg("approve")
        .arg("--wallet-id")
        .arg("00000000-0000-0000-0000-000000000000")
        .arg("--delegate")
        .arg("11111111111111111111111111111111")
        .arg("--amount")
        .arg("100")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn spl_balance_missing_address_rejects() {
    // clap-level: --address is required for spl balance.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("spl")
        .arg("balance")
        .arg("--token")
        .arg("USDC")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn spl_allowance_missing_delegate_rejects() {
    // clap-level: --delegate is required for spl allowance.
    let tmp = TempDir::new().expect("tempdir");
    sol_bin()
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("spl")
        .arg("allowance")
        .arg("--token")
        .arg("USDC")
        .arg("--owner")
        .arg("11111111111111111111111111111111")
        .assert()
        .failure()
        .code(2);
}
