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
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run. Speedup CLI not yet wired; see follow-up issue. Test contract per Issue #363 AC #3."]
async fn wallet_speedup_replaces_pending_tx_with_higher_fee_and_same_nonce() {
    // Contract: import alpha wallet with Anvil's default mnemonic, send
    // a low-fee tx, capture (hash, nonce, original_max_fee), speedup the
    // tx with higher max_fee + max_priority_fee, assert new tx hash,
    // same nonce, higher fees in the broadcast envelope.
    anvil_or_skip!();

    let anvil = alloy_node_bindings::Anvil::new().spawn();
    let endpoint = anvil.endpoint();

    let tmp = TempDir::new().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();

    unimplemented!(
        "speedup CLI not yet wired — fee-bumping impl is a separate follow-up issue. \
         This test contract exists per Issue #363 AC #3. Test body will be implemented \
         when the `eth wallet speedup --speedup <hash> --max-fee-per-gas <N> \
         --max-priority-fee-per-gas <N>` subcommand lands. Setup: anvil endpoint `{endpoint}` \
         is available; data dir at `{}`. Pattern follows `send_command_against_anvil_returns_tx_hash` \
         in `cli_localnet.rs` (import alpha from Anvil default mnemonic, send low-fee tx, \
         capture hash + nonce, speedup with higher fees, assert replacement).",
        data_dir.display(),
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run. Speedup CLI not yet wired; see follow-up issue. Test contract per Issue #363 AC #3."]
async fn wallet_speedup_with_lower_max_fee_rejected_as_fee_too_low() {
    // Contract: speedup with max_fee_per_gas <= in-pool max_fee_per_gas
    // → `Error::FeeTooLow` (exit 2). Per Issue #354's M-3 fee-resolution
    // precedence + L12 review HIGH #1 (max_fee_per_gas=0 must yield
    // FeeTooLow, exit 2). Mirrors the guard in `handlers.rs::resolve_overrides`
    // (existing for native + ERC-20 sends) but extended for speedup.
    anvil_or_skip!();

    unimplemented!(
        "speedup CLI not yet wired — see follow-up issue. Test contract per Issue #363 AC #3. \
         Will assert exit code 2 + stderr contains 'fee too low' (mirroring \
         `send_command_with_only_max_fee_per_gas_yields_exit_2` in cli_localnet.rs but for \
         the speedup subcommand's lower-fee rejection path).",
    );
}

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ANVIL_E2E=1 to run. Speedup CLI not yet wired; see follow-up issue. Test contract per Issue #363 AC #3."]
async fn wallet_speedup_with_mismatched_nonce_rejected() {
    // Contract: speedup against a tx_hash whose on-chain nonce doesn't
    // match the wallet's current nonce → `Error::NonceMismatch` (exit 2).
    // Per `handlers.rs::resolve_gas` trust-boundary check pattern + L12
    // security L-1 (chain_id trust-boundary guard). The speedup path must
    // look up the pending tx's nonce, compare to the wallet's next nonce,
    // and reject if they don't match (otherwise the new tx gets a different
    // nonce and creates a second pending tx instead of replacing).
    anvil_or_skip!();

    unimplemented!(
        "speedup CLI not yet wired — see follow-up issue. Test contract per Issue #363 AC #3. \
         Will assert exit code 2 + stderr contains 'nonce mismatch' (mirroring \
         nonce-mismatch handling in handlers::resolve_gas).",
    );
}
