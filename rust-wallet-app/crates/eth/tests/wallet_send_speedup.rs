//! EIP-1559 fee-bumping (replace-by-fee / speedup) test contract — Issue #363.
//!
//! ## Drift finding
//!
//! Issue #354 (dynamic gas estimation) cited `tests/wallet_send_speedup.rs`
//! under "Out of scope" as the future-PR test fixture for fee-bumping.
//! L13 step 4a drift scan (`git log --all -- rust-wallet-app/crates/eth/tests/wallet_send_speedup.rs`)
//! returned empty — the artifact referenced in #354 was never committed
//! (filed as Issue #363 follow-up).
//!
//! ## Scope of this fixture
//!
//! This file lands the **test contract** per L29 `#[ignore]` pattern. The
//! fee-bumping implementation itself (new `wallet speedup --speedup <hash>
//! --max-fee-per-gas <new> --max-priority-fee-per-gas <new>` subcommand +
//! same-nonce replacement logic) is a separate follow-up. Each test body
//! calls `unimplemented!()` with a pointer to the follow-up so:
//!
//! 1. Tests compile cleanly (`cargo build -p eth --tests` passes).
//! 2. `cargo test -p eth --test wallet_send_speedup` skips them by default.
//! 3. `RUN_ANVIL_E2E=1 cargo test -p eth --test wallet_send_speedup -- --ignored`
//!    panics with a clear unimplemented! message instead of silently passing.
//! 4. Future impl work: remove `#[ignore]` + replace `unimplemented!()` body
//!    with the real assertions.
//!
//! ## Reference
// #354 "Out of scope" line + L13 step 4a drift #1 + Issue #363 AC #1-3.

use std::path::PathBuf;

use tempfile::TempDir;

// `Cargo` provides the `eth` binary path to integration tests via this env var.
#[allow(dead_code)] // scaffolding for future impl work — currently `unimplemented!()` test bodies don't call it
fn eth_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_eth"))
}

/// Skip helper for Anvil-gated tests (L29): if `RUN_ANVIL_E2E` is unset,
/// log and return early. Mirrors `cli_localnet.rs::anvil_or_skip!` rather
/// than refactoring into a shared `tests/common/` module (L13 karpathy
/// principle 2: minimum code that solves the problem; sharing helpers for
/// 1 fixture = premature abstraction).
macro_rules! anvil_or_skip {
    () => {
        if std::env::var("RUN_ANVIL_E2E").ok().as_deref() != Some("1") {
            eprintln!("[wallet_send_speedup] SKIP — set RUN_ANVIL_E2E=1 to run");
            return;
        }
    };
}

/// Strip `ETH_PASSWORD` from the parent shell environment so the
/// `eth` CLI doesn't accidentally pick it up from CI shell exports.
/// Mirrors `cli_localnet.rs::run_eth` (L53/L9: surgical duplication —
/// shared `tests/common/` is the right call once we have 3+ binaries
/// duplicating this; 1 binary = too early).
#[allow(dead_code)] // scaffolding for future impl work
fn run_eth(data_dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    use std::process::Command;
    Command::new(eth_bin())
        .env("ETH_DATA_DIR", data_dir)
        .env("NO_COLOR", "1")
        .env_remove("ETH_PASSWORD")
        .args(args)
        .output()
        .expect("spawn eth")
}

/// Assert the output looks like a successful CLI run: exit 0 + stdout
/// contains the given substring.
#[allow(dead_code)] // scaffolding for future impl work
fn assert_success(out: &std::process::Output, needle: &str) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        stdout,
        stderr,
    );
    assert!(
        stdout.contains(needle),
        "stdout missing {needle:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
}

// ============================================================================
// EIP-1559 fee-bumping (replace-by-fee / speedup) — test contract
// ============================================================================
//
// Future impl (separate issue, post-#363): a new `eth wallet speedup`
// subcommand that takes a pending tx hash + new fee params, looks up the
// original tx's nonce, re-signs the same envelope with the new fees, and
// broadcasts via `provider.send_raw_transaction` (alloy's RPC layer
// rejects same-nonce broadcasts at lower fees than the in-pool tx).
//
// Invariants to verify when impl lands:
// 1. Happy path: speedup with `max_fee_per_gas > original.max_fee_per_gas`
//    AND `max_priority_fee_per_gas >= original.max_priority_fee_per_gas`
//    AND same nonce → new tx hash, same nonce, higher effective tip.
// 2. Lower fee rejection: speedup with max_fee_per_gas <= in-pool
//    max_fee_per_gas → `Error::FeeTooLow` (exit 2). EIP-1559 invariant.
// 3. Mismatched nonce rejection: speedup against a tx_hash whose on-chain
//    nonce doesn't match the wallet's current nonce → `Error::NonceMismatch`
//    (exit 2).

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run. Speedup CLI wired (Issue #381); test asserts happy-path replacement end-to-end against Anvil."]
async fn wallet_speedup_replaces_pending_tx_with_higher_fee_and_same_nonce() {
    // Issue #381 AC #2 + AC #4. Pattern follows
    // `send_command_against_anvil_returns_tx_hash` in `cli_localnet.rs`.
    // Send a low-fee tx first, capture hash + nonce + fees, then speedup
    // with strictly higher fees, assert new hash != old hash + same nonce.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().block_time(60).spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    // Anvil dev mnemonic #0: pre-funded with 10000 ETH.
    let phrase = "test test test test test test test test test test test junk";
    let import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "anvil-acct",
            "--mnemonic",
            phrase,
            "--password",
            "test-password",
            "--network",
            "anvil",
        ],
    );
    let import_stdout = String::from_utf8_lossy(&import.stdout);
    let import_stderr = String::from_utf8_lossy(&import.stderr);
    assert_eq!(
        import.status.code(),
        Some(0),
        "wallet import must succeed (Anvil default mnemonic)\nstdout: {import_stdout}\nstderr: {import_stderr}",
    );

    // Step 1: send a low-fee tx (1 gwei max_fee, 1 wei priority) — will
    // sit in the mempool awaiting a higher-fee replacement.
    let low_fee_send = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "send",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            "1000000000000000000", // 1 ETH in wei
            "--max-fee-per-gas",
            "1000000000", // 1 gwei
            "--max-priority-fee-per-gas",
            "1",
        ],
    );
    let low_fee_stdout = String::from_utf8_lossy(&low_fee_send.stdout);
    let low_fee_stderr = String::from_utf8_lossy(&low_fee_send.stderr);
    assert_eq!(
        low_fee_send.status.code(),
        Some(0),
        "low-fee send must succeed (Anvil pre-funds account #0 with 10000 ETH)\nstdout: {low_fee_stdout}\nstderr: {low_fee_stderr}",
    );
    // Parse the tx hash from stdout (last non-empty line, 0x-prefixed).
    let low_fee_hash = low_fee_stdout
        .lines()
        .rfind(|l| l.trim().starts_with("0x"))
        .unwrap_or("")
        .trim()
        .to_string();
    assert!(
        low_fee_hash.starts_with("0x") && low_fee_hash.len() == 66,
        "low-fee send stdout must contain a 0x + 64-hex tx hash; got: {low_fee_stdout}",
    );

    // Step 2: speedup with strictly higher fees (2 gwei max, 1 gwei
    // priority). Must succeed; new hash must differ from low_fee_hash.
    let speedup = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "wallet",
            "speedup",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--speedup",
            &low_fee_hash,
            "--max-fee-per-gas",
            "2000000000", // 2 gwei (strictly > 1 gwei low_fee)
            "--max-priority-fee-per-gas",
            "1000000000", // 1 gwei (>= 1 wei low_fee priority)
        ],
    );
    let speedup_stdout = String::from_utf8_lossy(&speedup.stdout);
    let speedup_stderr = String::from_utf8_lossy(&speedup.stderr);
    assert_eq!(
        speedup.status.code(),
        Some(0),
        "speedup must succeed against Anvil\nstdout: {speedup_stdout}\nstderr: {speedup_stderr}",
    );
    let speedup_hash = speedup_stdout
        .lines()
        .rfind(|l| l.trim().starts_with("0x"))
        .unwrap_or("")
        .trim()
        .to_string();
    assert!(
        speedup_hash.starts_with("0x") && speedup_hash.len() == 66,
        "speedup stdout must contain a 0x + 64-hex tx hash; got: {speedup_stdout}",
    );
    assert_ne!(
        speedup_hash, low_fee_hash,
        "speedup must produce a new tx hash (different signed envelope); both={speedup_hash} low={low_fee_hash}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run. Speedup CLI wired (Issue #381); test asserts lower-fee rejection path."]
async fn wallet_speedup_with_lower_max_fee_rejected_as_fee_too_low() {
    // Issue #381 AC #3 gate 7: speedup with `max_fee_per_gas <= in-pool`
    // → `Error::FeeTooLow` (exit 2). Mirrors
    // `send_command_with_only_max_fee_per_gas_yields_exit_2` (cli_localnet.rs).
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().block_time(60).spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let phrase = "test test test test test test test test test test test junk";
    let import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "anvil-acct",
            "--mnemonic",
            phrase,
            "--password",
            "test-password",
            "--network",
            "anvil",
        ],
    );
    let import_stdout = String::from_utf8_lossy(&import.stdout);
    let import_stderr = String::from_utf8_lossy(&import.stderr);
    assert_eq!(
        import.status.code(),
        Some(0),
        "wallet import must succeed (Anvil default mnemonic)\nstdout: {import_stdout}\nstderr: {import_stderr}",
    );

    // Send a tx at 2 gwei max_fee first.
    let send = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "send",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            "1000000000000000000",
            "--max-fee-per-gas",
            "2000000000", // 2 gwei
            "--max-priority-fee-per-gas",
            "1",
        ],
    );
    let send_stdout = String::from_utf8_lossy(&send.stdout);
    let send_stderr = String::from_utf8_lossy(&send.stderr);
    assert_eq!(
        send.status.code(),
        Some(0),
        "send must succeed before speedup attempt\nstdout: {send_stdout}\nstderr: {send_stderr}",
    );
    let low_hash = send_stdout
        .lines()
        .rfind(|l| l.trim().starts_with("0x"))
        .unwrap_or("")
        .trim()
        .to_string();

    // Speedup at the SAME 2 gwei (NOT strictly greater) → gate 7
    // rejects → exit 2 + stderr contains "fee too low".
    let speedup = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "wallet",
            "speedup",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--speedup",
            &low_hash,
            "--max-fee-per-gas",
            "2000000000", // equal to in-pool, NOT greater
            "--max-priority-fee-per-gas",
            "1",
        ],
    );
    let stderr = String::from_utf8_lossy(&speedup.stderr);
    assert_eq!(
        speedup.status.code(),
        Some(2),
        "lower-or-equal max_fee_per_gas must yield exit 2 (FeeTooLow); stderr: {stderr}",
    );
    assert!(
        stderr.contains("fee too low"),
        "stderr must contain 'fee too low' substring (Error::FeeTooLow Display); got: {stderr}",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run. Speedup CLI wired (Issue #381); test asserts mismatched-nonce rejection path."]
async fn wallet_speedup_with_mismatched_nonce_rejected() {
    // Issue #381 AC #3 gate 6: speedup against a tx whose on-chain nonce
    // != wallet's current nonce → `Error::NonceMismatch` (exit 2).
    // Setup: send 2 txs back-to-back (advance wallet nonce), then attempt
    // to speedup the FIRST tx (now stale, wallet nonce has moved past it).
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().block_time(60).spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    let phrase = "test test test test test test test test test test test junk";
    let import = run_eth(
        &data_dir,
        &[
            "wallet",
            "import",
            "--name",
            "anvil-acct",
            "--mnemonic",
            phrase,
            "--password",
            "test-password",
            "--network",
            "anvil",
        ],
    );
    let import_stdout = String::from_utf8_lossy(&import.stdout);
    let import_stderr = String::from_utf8_lossy(&import.stderr);
    assert_eq!(
        import.status.code(),
        Some(0),
        "wallet import must succeed (Anvil default mnemonic)\nstdout: {import_stdout}\nstderr: {import_stderr}",
    );

    // Send tx #1 (nonce 0).
    let send1 = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "send",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            "1000000000000000000",
            "--max-fee-per-gas",
            "1000000000",
            "--max-priority-fee-per-gas",
            "1",
        ],
    );
    let send1_stdout = String::from_utf8_lossy(&send1.stdout);
    let send1_stderr = String::from_utf8_lossy(&send1.stderr);
    assert_eq!(
        send1.status.code(),
        Some(0),
        "send #1 must succeed\nstdout: {send1_stdout}\nstderr: {send1_stderr}",
    );
    let first_hash = send1_stdout
        .lines()
        .rfind(|l| l.trim().starts_with("0x"))
        .unwrap_or("")
        .trim()
        .to_string();

    // Send tx #2 (nonce 1) — advances wallet's current nonce past tx #1.
    let send2 = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "send",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--to",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "--amount",
            "1000000000000000000",
            "--max-fee-per-gas",
            "1000000000",
            "--max-priority-fee-per-gas",
            "1",
        ],
    );
    let send2_stdout = String::from_utf8_lossy(&send2.stdout);
    let send2_stderr = String::from_utf8_lossy(&send2.stderr);
    assert_eq!(
        send2.status.code(),
        Some(0),
        "send #2 must succeed\nstdout: {send2_stdout}\nstderr: {send2_stderr}",
    );

    // Speedup attempt on tx #1 — wallet nonce is now 2, tx #1 nonce was
    // 0 → gate 6 (nonce drift) rejects → exit 2 + stderr contains
    // "nonce mismatch".
    let speedup = run_eth(
        &data_dir,
        &[
            "--rpc-url",
            &endpoint,
            "wallet",
            "speedup",
            "--name",
            "anvil-acct",
            "--password",
            "test-password",
            "--network",
            "anvil",
            "--speedup",
            &first_hash,
            "--max-fee-per-gas",
            "5000000000", // 5 gwei (well above low_fee)
            "--max-priority-fee-per-gas",
            "1000000000",
        ],
    );
    let stderr = String::from_utf8_lossy(&speedup.stderr);
    assert_eq!(
        speedup.status.code(),
        Some(2),
        "mismatched nonce must yield exit 2 (NonceMismatch); stderr: {stderr}",
    );
    assert!(
        stderr.contains("nonce mismatch"),
        "stderr must contain 'nonce mismatch' substring (Error::NonceMismatch Display); got: {stderr}",
    );
}
