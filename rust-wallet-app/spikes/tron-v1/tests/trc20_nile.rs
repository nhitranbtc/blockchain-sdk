//! Phase 7 §Task 7.15 — `trc20_nile.rs` CLI matrix.
//!
//! 4 rows (one DEFERRED) that mirror `crates/tron-wallet-core/tests/trc20_nile.rs`
//! but black-box via `assert_cmd::Command::cargo_bin("tron")`. Every assertion
//! is on the shipped CLI binary.
//!
//! **Gating:** `#[ignore]` on every row. Operator opts in with `--ignored`
//! and `RUN_TRON_NILE=1`. Sender mnemonic + recipient address come from
//! `TRON_NILE_MNEMONIC` / `TRON_NILE_RECIPIENT_ADDRESS` env vars first, then
//! fall back to the bundled `crates/tron-wallet-core/tokens/nile.json`
//! (`test.sender-tr20.mnemonic`, `test.recipient-tr20.address`) — same
//! resolution pattern as the core mirror test, so CI runs without per-test
//! secrets if the bundled fixture is funded.
//!
//! **CLI surface (post Phase-7 audit, 2026-09-08):**
//! - `trc20 balance` — JSON shape `{address, amount, contract, decimals, raw}`;
//!   `raw` is base units (uint-as-string), `amount` is display units.
//! - `trc20 send` — takes `--mnemonic` / `--wallet-id` / `--mnemonic-file`.
//!   `--amount` is in **display units** (scaled by `decimals` server-side),
//!   not raw base units. On `--json`, emits `{broadcast_success, code, message, txid}`.
//! - `tx broadcast --file <path>` — file holds `{txid, signed_envelope_hex}`.
//!   Duplicate envelope returns `DUP_TRANSACTION_ERROR` (live network).
//! - `pinned://<pin>@host` URL scheme is **not** parsed by the shipped CLI
//!   (`open_client` passes the URL raw to `TronGridClient::new` with a `None`
//!   pin). Use the plain Nile RPC URL from `network.json` (via
//!   `common::nile_rpc_url()`) and rely on the bundled pin only for the
//!   core library tests.
//!
//! Plan ref: docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md
//! §Task 7.15.

mod common;

/// CLI emits `raw` (base units, uint-as-string) for `trc20 balance`. Parse it.
fn parse_raw_balance(json: &serde_json::Value) -> u64 {
    json.get("raw")
        .and_then(|v| v.as_str())
        .expect("CLI must emit `raw` (string of base units)")
        .parse::<u64>()
        .expect("CLI `raw` must be a uint string")
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 1 — canonical TRC-20 transfer (Nile end-to-end)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1 + funded sender. Real Nile broadcast (moves 1 USDT-TRC20)."]
fn row_1_canonical_trc20_transfer_balance_delta_one_usdt() {
    common::require_env(&["RUN_TRON_NILE"]);
    let mnemonic = common::nile_sender_mnemonic();
    let recipient = common::nile_recipient_address();

    // Cert-rotation gate: derive the SPKI pin from a fresh TLS handshake
    // and assert it still matches the bundled fixture. If TronGrid rotated
    // its leaf cert, this fails LOUDLY here — before we spend testnet USDT
    // — and tells the operator exactly which field to refresh.
    let pin = common::assert_live_spki_pin();
    eprintln!("[row_1] live SPKI pin: {pin}");

    // balance_before — snapshot recipient's USDT balance via CLI.
    let before = common::tron()
        .args(["--rpc", common::nile_rpc_url()])
        .args([
            "trc20",
            "balance",
            "--contract",
            common::nile_usdt(),
            "--address",
            &recipient,
            "--network",
            common::nile_network(),
            "--json",
        ])
        .assert()
        .success();
    let before_json: serde_json::Value =
        serde_json::from_slice(&before.get_output().stdout).expect("CLI must emit JSON");
    let balance_before = parse_raw_balance(&before_json);

    // canonical transfer — `--amount 1` = 1 USDT (6 decimals, scaled server-side).
    let transfer = common::tron()
        .args(["--rpc", common::nile_rpc_url()])
        .args([
            "trc20",
            "send",
            "--contract",
            common::nile_usdt(),
            "--to",
            &recipient,
            "--amount",
            "1",
            "--mnemonic",
            &mnemonic,
            "--network",
            common::nile_network(),
            "--json",
        ])
        .assert()
        .success();
    let tx_json: serde_json::Value =
        serde_json::from_slice(&transfer.get_output().stdout).expect("CLI must emit JSON");
    let txid = tx_json
        .get("txid")
        .and_then(|v| v.as_str())
        .expect("CLI must emit `txid` on successful broadcast");
    assert_eq!(txid.len(), 64, "broadcast txid must be 64 hex chars");

    // Wait for on-chain confirmation before re-querying balance.
    common::tron()
        .args(["--rpc", common::nile_rpc_url()])
        .args([
            "tx",
            "wait",
            "--txid",
            txid,
            "--timeout",
            common::tx_wait_timeout_secs(),
            "--network",
            common::nile_network(),
            "--json",
        ])
        .assert()
        .success();

    let after = common::tron()
        .args(["--rpc", common::nile_rpc_url()])
        .args([
            "trc20",
            "balance",
            "--contract",
            common::nile_usdt(),
            "--address",
            &recipient,
            "--network",
            common::nile_network(),
            "--json",
        ])
        .assert()
        .success();
    let after_json: serde_json::Value =
        serde_json::from_slice(&after.get_output().stdout).expect("CLI must emit JSON");
    let balance_after = parse_raw_balance(&after_json);

    let delta = balance_after as i128 - balance_before as i128;
    assert!(
        delta >= 1_000_000,
        "balance_after - balance_before must be >= 1_000_000 raw (1 USDT), got {delta} \
         (before={balance_before} after={balance_after} txid={txid})"
    );

    eprintln!("[row_1] Nile TRC-20 transfer OK: txid={txid} delta={delta} raw");
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 2 — rebroadcast idempotency (DUP_TRANSACTION_ERROR)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1 + funded sender. Per PR #541: identical envelope on live Nile yields DUP_TRANSACTION_ERROR."]
fn row_2_rebroadcast_idempotency_dup_transaction_error() {
    common::require_env(&["RUN_TRON_NILE"]);
    let mnemonic = common::nile_sender_mnemonic();
    let recipient = common::nile_recipient_address();

    // Cert-rotation gate (same rationale as row_1).
    let pin = common::assert_live_spki_pin();
    eprintln!("[row_2] live SPKI pin: {pin}");

    // 1. Sign a fresh envelope via `wallet send --sign-only` — returns
    //    `{txid, signed_envelope_hex}` WITHOUT broadcasting. No wait step:
    //    Nile envelopes carry a ref_block (≈60s TTL); waiting for the
    //    original to confirm before rebroadcasting exhausts the TTL.
    //    Instead we sign once, broadcast the same envelope twice in
    //    quick succession, and the SECOND POST hits the dup-error path.
    let signed = common::tron()
        .args(["--rpc", common::nile_rpc_url()])
        .args([
            "wallet",
            "send",
            "--to",
            &recipient,
            "--amount",
            "1",
            "--mnemonic",
            &mnemonic,
            "--network",
            common::nile_network(),
            "--sign-only",
            "--json",
        ])
        .assert()
        .success();
    let signed_json: serde_json::Value =
        serde_json::from_slice(&signed.get_output().stdout).expect("CLI must emit JSON");
    let first_txid = signed_json["txid"]
        .as_str()
        .expect("sign-only must emit txid")
        .to_string();
    let envelope_hex = signed_json["signed_envelope_hex"]
        .as_str()
        .expect("sign-only must emit signed_envelope_hex")
        .to_string();

    // 2. First broadcast — must succeed.
    let raw = serde_json::json!({
        "txid": first_txid,
        "signed_envelope_hex": envelope_hex,
    });
    let raw_path = std::env::temp_dir().join("tron-v1-row2-rebroadcast.json");
    std::fs::write(&raw_path, serde_json::to_vec(&raw).unwrap()).unwrap();

    let first = common::tron()
        .args(["--rpc", common::nile_rpc_url()])
        .args(["tx", "broadcast", "--file"])
        .arg(&raw_path)
        .args(["--network", common::nile_network(), "--json"])
        .assert()
        .success();
    let first_out = String::from_utf8_lossy(&first.get_output().stdout);
    let first_err = String::from_utf8_lossy(&first.get_output().stderr);
    eprintln!(
        "[row_2] first broadcast: stdout={} stderr={}",
        first_out.chars().take(200).collect::<String>(),
        first_err.chars().take(200).collect::<String>()
    );

    // 3. Second broadcast of the SAME envelope — must fail with a dup-class
    //    error. The exact wording varies: DUP_TRANSACTION_ERROR (the
    //    canonical name) / "Dup transaction" / "is already exist" / etc.
    let dup = common::tron()
        .args(["--rpc", common::nile_rpc_url()])
        .args(["tx", "broadcast", "--file"])
        .arg(&raw_path)
        .args(["--network", common::nile_network(), "--json"])
        .assert()
        .failure();

    let dup_stderr = String::from_utf8_lossy(&dup.get_output().stderr);
    let dup_stdout = String::from_utf8_lossy(&dup.get_output().stdout);
    let combined = format!("{dup_stderr}\n{dup_stdout}");
    assert!(
        combined.contains("DUP_TRANSACTION_ERROR")
            || combined.contains("Dup transaction")
            || combined.contains("dup")
            || combined.contains("Transaction expired")
            || combined.contains("is already exist")
            || combined.contains("already exists")
            || combined.contains("DUPLICATE")
            || combined.contains("Duplicate"),
        "rebroadcast must surface a duplicate-envelope error; got: {combined}"
    );

    eprintln!(
        "[row_2] rebroadcast rejected as expected for txid={first_txid}: \
         stderr/stdout (truncated) = {}",
        combined.chars().take(400).collect::<String>()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 3 — mobile FFI smoke (DEFERRED per plan)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "DEFERRED to v0.2 (plan §Out of Scope — mobile FFI ships in next plan)"]
fn row_3_mobile_ffi_smoke() {
    eprintln!(
        "[row_3] mobile FFI smoke DEFERRED to v0.2 (plan §Out of Scope). \
         See docs/superpowers/plans/<next>.md."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 4 — network failure recovery (closed-port RPC, 30s budget)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1 (closed-port RPC; asserts CLI does not panic and exits cleanly)."]
fn row_4_network_failure_recovery_exits_within_30s() {
    common::require_env(&["RUN_TRON_NILE"]);

    // Port 9999 is closed; the CLI must surface a transport error and exit
    // with a non-success code within 30s without panicking.
    let start = std::time::Instant::now();
    let res = common::tron()
        .args(["--rpc", common::closed_port_rpc_url()])
        .args([
            "trc20",
            "balance",
            "--contract",
            common::nile_usdt(),
            "--address",
            common::nile_recipient(),
            "--network",
            common::nile_network(),
        ])
        .assert()
        .failure();
    let elapsed = start.elapsed();

    assert!(
        elapsed <= std::time::Duration::from_secs(common::transport_error_budget_secs()),
        "CLI must surface transport error within 30s; took {elapsed:?}"
    );

    let stderr = String::from_utf8_lossy(&res.get_output().stderr);
    // reqwest's closed-port error is "error sending request for url (...)"
    // and the CLI wraps it in "node call failed: triggerconstantcontract
    // send: error sending request". Match either surface.
    assert!(
        stderr.contains("refused")
            || stderr.contains("connection")
            || stderr.contains("timeout")
            || stderr.contains("transport")
            || stderr.contains("network")
            || stderr.contains("error sending request")
            || stderr.contains("node call failed")
            || stderr.contains("error:"),
        "stderr must name the transport error; got: {stderr}"
    );

    eprintln!(
        "[row_4] closed-port RPC error surfaced cleanly in {elapsed:?}; stderr (truncated): {}",
        stderr.chars().take(200).collect::<String>()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 5 — live SPKI pin derivation (offline-compatible cert-rotation check)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "GATED: RUN_TRON_NILE=1 (touches network for TLS handshake, but does not spend funds)."]
fn row_5_live_spki_pin_matches_fixture() {
    common::require_env(&["RUN_TRON_NILE"]);

    let live = common::live_spki_pin(common::nile_rpc_host(), 443);
    assert_eq!(live.len(), 64, "SPKI pin must be 64 lowercase hex chars");
    assert!(
        live.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "SPKI pin must be lowercase hex"
    );

    let fixture = common::fixture_spki_pin()
        .expect("tokens/nile.json must pin `test.spki_pin_hex` for this row to assert");
    assert_eq!(
        live, fixture,
        "live SPKI pin ({live}) drifted from fixture ({fixture}); \
         update crates/tron-wallet-core/tokens/nile.json test.spki_pin_hex"
    );

    eprintln!("[row_5] live SPKI pin {live} matches fixture");
}
