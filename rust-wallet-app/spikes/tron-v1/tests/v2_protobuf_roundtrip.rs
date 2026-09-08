//! V2 — protobuf roundtrip (CLI-driven, Phase 7 §Task 7.2).
//!
//! Plan §V2 table:
//! - `tron tx encode --file <raw.json>` produces a hex blob.
//! - `tron tx decode --hex <blob>` round-trips JSON byte-equal.
//! - `tron trc20 encode-call transfer --to <addr> --amount <num>` hex starts
//!   with `a9059cbb` (selector at bytes [0..4]).
//!
//! Note: the original v2 used `prost-build` against vendored
//! `proto/core/Tron.proto`. Task 7.13 deleted `spikes/tron-v1/src/` — proto
//! generation is the CLI's responsibility now (it owns `crates/tron/build.rs`).
//! The spike only verifies the CLI end-to-end. Offline invariant:
//! selector-prefixed hex emitted by the shipped CLI matches the canonical
//! keccak256-derived selectors (single source of truth =
//! `tron_wallet_core::trc20`).
//!
//! Three assertions below cover what's actually shipped today:
//! 1. `trc20 encode-call transfer` emits 68-byte hex starting `a9059cbb`.
//! 2. `trc20 encode-call approve`  emits 68-byte hex starting `095ea7b3`.
//! 3. Drift detector: `tron tx --help` does NOT expose `encode` / `decode`
//!    subcommands. When that flips (CLI ships them per
//!    `docs/audit/2026-09-07-phase-7-cli-drift.md` §5 step 1) this fails
//!    and forces the test author to wire in a positive protobuf-roundtrip
//!    assertion. Until then it PASSES as a documented BLOCKING.

#[path = "../../../crates/tron-wallet-core/tests/common/mod.rs"]
mod common;

/// First 4 bytes of keccak256("transfer(address,uint256)") = 0xa9059cbb.
/// Plan §Q3 documents this constant; reproduced here so a future selector
/// change in `tron_wallet_core::trc20` fails the test loudly (not silently).
const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

/// First 4 bytes of keccak256("approve(address,uint256)") = 0x095ea7b3.
const APPROVE_SELECTOR: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];

/// TRC-20 transfer/approve calldata layout: 4-byte selector + 32-byte
/// address slot + 32-byte uint256 slot = 68 bytes.
const TRC20_CALLDATA_LEN: usize = 68;

/// CLI emits `0x`-prefixed lowercase hex via
/// `crates/tron/src/handlers/trc20.rs::encode_call::println!("0x{}", …)`;
/// strip the prefix before decoding so a future flag change at the call
/// site (e.g. dropping `0x` for `--raw`) does not break this test.
fn strip_0x(hex: &str) -> &str {
    hex.strip_prefix("0x").unwrap_or(hex)
}

/// `tron trc20 encode-call transfer --to <addr> --amount <num>` emits
/// 68-byte calldata whose first 4 bytes are
/// keccak256("transfer(address,uint256)")[..4] = 0xa9059cbb. Pure offline
/// transform — no signing, no network, no `--mnemonic`.
#[test]
fn v2_trc20_encode_call_transfer_starts_with_a9059cbb() {
    let recipient = common::nile_recipient();
    let amount = "1000000";

    let assert = common::tron()
        .args([
            "trc20",
            "encode-call",
            "transfer",
            "--to",
            recipient,
            "--amount",
            amount,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let hex = stdout.trim();
    assert!(
        !hex.is_empty(),
        "`tron trc20 encode-call transfer` emitted empty stdout; got {stdout:?}"
    );

    let bytes =
        hex::decode(strip_0x(hex)).unwrap_or_else(|e| panic!("non-hex stdout ({e}): {hex:?}"));

    assert_eq!(
        bytes.len(),
        TRC20_CALLDATA_LEN,
        "TRC-20 transfer calldata must be exactly 68 bytes \
         (selector + 32-byte address slot + 32-byte uint256 slot); got {} bytes",
        bytes.len()
    );

    assert_eq!(
        &bytes[..4],
        &TRANSFER_SELECTOR,
        "TRC-20 transfer calldata must begin with keccak256(\
         \"transfer(address,uint256)\")[..4] = 0xa9059cbb; got 0x{}",
        hex::encode(&bytes[..4])
    );
}

/// Companion for `approve`: identical layout, different selector.
#[test]
fn v2_trc20_encode_call_approve_starts_with_095ea7b3() {
    let spender = common::nile_spender();
    let amount = "1000000";

    let assert = common::tron()
        .args([
            "trc20",
            "encode-call",
            "approve",
            "--to",
            spender,
            "--amount",
            amount,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let hex = stdout.trim();
    assert!(
        !hex.is_empty(),
        "`tron trc20 encode-call approve` emitted empty stdout; got {stdout:?}"
    );

    let bytes =
        hex::decode(strip_0x(hex)).unwrap_or_else(|e| panic!("non-hex stdout ({e}): {hex:?}"));

    assert_eq!(bytes.len(), TRC20_CALLDATA_LEN);
    assert_eq!(
        &bytes[..4],
        &APPROVE_SELECTOR,
        "TRC-20 approve calldata must begin with keccak256(\
         \"approve(address,uint256)\")[..4] = 0x095ea7b3; got 0x{}",
        hex::encode(&bytes[..4])
    );
}

/// Drift detector for the BLOCKING entries in
/// `docs/audit/2026-09-07-phase-7-cli-drift.md` §2.2: shipped `tron tx
/// --help` exposes only `get`, `wait`, `broadcast` — no `encode` /
/// `decode`. This test PASSES today (asserting absence) and FAILS the
/// day the follow-up plan ships those subcommands. At that point migrate
/// to a positive protobuf-roundtrip assertion (e.g. raw
/// `Transaction` JSON ↔ hex round-trip via the new CLI).
///
/// We anchor the absence check on clap's subcommand banner, not raw
/// substring search — `encode` could legitimately appear in `--help`
/// boilerplate (env-var descriptions, footer examples). An anchor on
/// the first whitespace-delimited token of each banner line matches the
/// subcommand name only.
#[test]
fn v2_tron_tx_help_does_not_expose_encode_or_decode() {
    let assert = common::tron().args(["tx", "--help"]).assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_lowercase();

    let lines: Vec<&str> = stdout.lines().collect();
    // clap's default template prints `Commands:` before the subcommand
    // banner. If a future `--style=json` adds a header line, fall back to
    // scanning the whole output — false-positive risk is low because
    // `encode` / `decode` as bare first-tokens is rare.
    let banner_start = lines
        .iter()
        .position(|l| l.trim_start().starts_with("commands:"))
        .unwrap_or(0);
    let banner = &lines[banner_start..];

    let has_encode_action = banner
        .iter()
        .any(|l| l.split_whitespace().next() == Some("encode"));
    let has_decode_action = banner
        .iter()
        .any(|l| l.split_whitespace().next() == Some("decode"));

    assert!(
        !has_encode_action && !has_decode_action,
        "BLOCKING resolved: shipped `tron tx --help` now exposes `encode` or `decode` \
         subcommands. Migrate this test to a positive protobuf-roundtrip assertion per \
         docs/audit/2026-09-07-phase-7-cli-drift.md §2.2 + §5 step 1. help-output:\n{stdout}"
    );
}
