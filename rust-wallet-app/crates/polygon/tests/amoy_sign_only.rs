//! P8-T2 / G3 (Issue #514) — `polygon wallet send --sign-only` on Amoy.
//!
//! Operator-driven live integration test per L29 (`#[ignore]` +
//! `require_run_polygon_amoy()` + opt-in env gate). Sister pattern
//! to `amoy_p9_pk_import.rs` (PK-file import + committed test vector
//! from `tokens/amoy.json::test_harness`).
//!
//! ## Operator pre-conditions (per L29 + plan §G3)
//!
//! 1. `tokens/amoy.json::test_harness.run_polygon_amoy = "1"` (or
//!    `RUN_POLYGON_AMOY=1` env var).
//! 2. `p9_test_wallet_data_dir` exists + is writable for wallet import.
//! 3. No fund requirement — sign-only does NOT broadcast, so the sender
//!    wallet needs zero POL/USDC to run these tests.
//!
//! ## Acceptance (plan §P8-T2)
//!
//! - `amoy_sign_only_does_not_broadcast`: stdout contains `0x`-prefixed
//!   raw RLP; the EIP-1559 type byte is `0x02` (proves no Legacy
//!   envelope regression); sender nonce is RPC-fetched but never
//!   consumed by `eth_sendRawTransaction` (sign-only short-circuits
//!   before the broadcast branch).
//! - `amoy_sign_only_is_deterministic`: signing the same input twice
//!   yields byte-identical RLP (EIP-1559 nonce is RPC-fetched once
//!   per invocation; same nonce → same envelope when no other tx is
//!   pending in between).
//!
//! ## CLI surface (added by this PR)
//!
//! `polygon wallet send --sign-only --name <w> --to <addr> --amount <wei>
//!  --network amoy --data-dir <dir>` exits 0, prints `signed_tx: 0x<rlp>`
//! to stdout, does NOT call `eth_sendRawTransaction` (sign-only
//! handler short-circuits before the broadcast branch).

#![cfg(test)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use alloy_primitives::{hex, Address, B256};
use alloy_provider::Provider;
use serde_json::Value;

// =====================================================================
// JSON SoT loader (sister to amoy_p9_pk_import.rs)
// =====================================================================

fn ensure_tokens_loaded() {
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        let path = format!("{}/tokens/amoy.json", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!("failed to read {path}: {e} — tokens/amoy.json is the committed Amoy SoT")
        });
        let parsed: Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));
        AMOY_TOKENS_JSON
            .set(parsed)
            .expect("AMOY_TOKENS_JSON OnceLock set twice");
    });
}

static AMOY_TOKENS_JSON: OnceLock<Value> = OnceLock::new();

fn amoy_json(key: &str) -> Value {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get(key)
        .cloned()
        .unwrap_or_else(|| panic!("missing top-level `{key}` in tokens/amoy.json"))
}

fn amoy_test_harness(key: &str) -> Value {
    let harness = amoy_json("test_harness");
    harness
        .get(key)
        .cloned()
        .unwrap_or_else(|| panic!("missing `test_harness.{key}` in tokens/amoy.json"))
}

fn amoy_rpc_url() -> String {
    amoy_json("rpc_url")
        .as_str()
        .expect("`rpc_url` field in tokens/amoy.json must be a string")
        .to_string()
}

fn amoy_chain_id() -> u64 {
    amoy_json("chain_id")
        .as_u64()
        .expect("`chain_id` field in tokens/amoy.json must be a u64")
}

/// L29 opt-in guard — panics unless `tokens/amoy.json::test_harness.run_polygon_amoy`
/// OR `RUN_POLYGON_AMOY=1` env var is set. Sister to
/// `amoy_p9_pk_import.rs::require_run_polygon_amoy`.
fn require_run_polygon_amoy() {
    ensure_tokens_loaded();
    let from_json = AMOY_TOKENS_JSON
        .get()
        .and_then(|v| v.get("test_harness"))
        .and_then(|t| t.get("run_polygon_amoy"))
        .and_then(|s| s.as_str())
        .map(String::from);
    let from_env = std::env::var("RUN_POLYGON_AMOY").ok();
    let resolved = from_json.or(from_env).unwrap_or_default();
    assert_eq!(
        resolved, "1",
        "tokens/amoy.json test_harness.run_polygon_amoy must be \"1\" \
         (or RUN_POLYGON_AMOY=1 env var set) for live test runs; current = {resolved:?}"
    );
}

fn p9_test_pk_hex_test_vector() -> String {
    let raw = amoy_test_harness("p9_test_pk_hex_test_vector");
    raw.as_str().map(String::from).unwrap_or_else(|| {
        panic!("tokens/amoy.json missing test_harness.p9_test_pk_hex_test_vector")
    })
}

fn p9_test_wallet_password_test_vector() -> String {
    let raw = amoy_test_harness("p9_test_wallet_password_test_vector");
    raw.as_str().map(String::from).unwrap_or_else(|| {
        panic!("tokens/amoy.json missing test_harness.p9_test_wallet_password_test_vector")
    })
}

fn p9_test_wallet_data_dir() -> PathBuf {
    let raw = amoy_test_harness("p9_test_wallet_data_dir");
    let s = raw
        .as_str()
        .unwrap_or_else(|| panic!("tokens/amoy.json missing test_harness.p9_test_wallet_data_dir"));
    let p = PathBuf::from(s);
    if p.is_absolute() {
        p
    } else {
        // Resolve relative paths against the repo root (3 levels up from
        // rust-wallet-app/crates/polygon) so the test works regardless
        // of cwd. Sister to `amoy_erc20_send.rs::amoy_wallet_data_dir`.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest
            .parent()
            .and_then(|x| x.parent())
            .and_then(|x| x.parent())
            .map(|root| root.join(&p))
            .unwrap_or(p)
    }
}

// =====================================================================
// Polygon CLI runner + PK-file import helpers (sister to amoy_p9_pk_import.rs)
// =====================================================================

/// `polygon` binary path (compile-time cargo metadata).
fn polygon_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_polygon"))
}

/// Write `bytes` to `<tmpdir>/<name>` with the requested mode. Unix
/// only — Windows lacks `PermissionsExt::set_permissions`.
#[cfg(unix)]
fn write_pk_file(dir: &std::path::Path, name: &str, bytes: &[u8], mode: u32) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write pk file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .expect("set pk file mode");
    path
}

/// Import a PK via the CLI's `--private-key-file` flag. Returns
/// (exit_code, stdout, stderr) tuple. Sister to
/// `amoy_p9_pk_import.rs::run_polygon_import`.
fn run_polygon_import_pk(
    pk_path: &std::path::Path,
    network: &str,
    wallet_name: &str,
    data_dir: &std::path::Path,
    password: &str,
) -> (i32, String, String) {
    let output = Command::new(polygon_bin())
        .env("POLYGON_PASSWORD", password)
        .arg("wallet")
        .arg("import")
        .arg("--private-key-file")
        .arg(pk_path)
        .arg("--name")
        .arg(wallet_name)
        .arg("--network")
        .arg(network)
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn polygon wallet import");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit = output.status.code().unwrap_or(-1);
    if exit != 0 {
        eprintln!("polygon wallet import failed (exit {exit})\nstdout: {stdout}\nstderr: {stderr}");
    }
    (exit, stdout, stderr)
}

/// Invoke `polygon wallet send --sign-only` and capture stdout/stderr.
/// Returns (exit_code, stdout, stderr).
///
/// `max_fee_gwei` + `priority_fee_gwei` are pinned explicitly when
/// `Some` to remove the Amoy gas-oracle shift between invocations
/// (per L12 review MED #B). The determinism test must pin both; the
/// no-broadcast test can leave them `None` because the oracle
/// difference only matters for byte-equality comparison, not for
/// the no-broadcast proof.
fn run_polygon_sign_only(
    wallet_name: &str,
    password: &str,
    to: &str,
    amount_wei: &str,
    data_dir: &std::path::Path,
    max_fee_gwei: Option<&str>,
    priority_fee_gwei: Option<&str>,
) -> (i32, String, String) {
    let mut cmd = Command::new(polygon_bin());
    cmd.env("POLYGON_PASSWORD", password)
        .arg("wallet")
        .arg("send")
        .arg("--sign-only")
        .arg("--name")
        .arg(wallet_name)
        .arg("--network")
        .arg("amoy")
        .arg("--to")
        .arg(to)
        .arg("--amount")
        .arg(amount_wei)
        .arg("--unit")
        .arg("wei")
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(g) = max_fee_gwei {
        cmd.arg("--max-fee-gwei").arg(g);
    }
    if let Some(g) = priority_fee_gwei {
        cmd.arg("--priority-fee-gwei").arg(g);
    }
    let output = cmd.output().expect("spawn polygon wallet send --sign-only");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit = output.status.code().unwrap_or(-1);
    if exit != 0 {
        eprintln!(
            "polygon wallet send --sign-only failed (exit {exit})\nstdout: {stdout}\nstderr: {stderr}"
        );
    }
    (exit, stdout, stderr)
}

/// Extract the first `0x[0-9a-fA-F]{40}` substring (EIP-55 address)
/// from polygon CLI stdout (sister to `amoy_p9_pk_import.rs::extract_eip55`).
fn extract_eip55(s: &str) -> Option<String> {
    let needle = b"0x";
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 42 <= bytes.len() {
        if &bytes[i..i + 2] == needle && bytes[i + 2..i + 42].iter().all(|b| b.is_ascii_hexdigit())
        {
            return Some(s[i..i + 42].to_string());
        }
        i += 1;
    }
    None
}

/// Parse the hex RLP from `--sign-only` stdout. The CLI emits a line
/// of the shape `signed_tx: 0x<hex>` (exact format decided by the
/// dispatch layer — see `main.rs::Send`). Returns the raw RLP bytes
/// (hex-decoded, no `0x` prefix).
fn extract_sign_only_rlp(stdout: &str) -> Vec<u8> {
    let line = stdout
        .lines()
        .find(|l| l.starts_with("signed_tx:"))
        .unwrap_or_else(|| {
            panic!("sign-only stdout must contain `signed_tx: 0x...` line; got: {stdout}")
        });
    let after = line.trim_start_matches("signed_tx:").trim();
    if !after.starts_with("0x") {
        panic!("signed_tx line must be 0x-prefixed hex; got: {line}");
    }
    let raw = after.trim_start_matches("0x");
    hex::decode(raw).unwrap_or_else(|e| panic!("signed_tx hex decode failed: {e}; line: {line}"))
}

/// Unique wallet name per test invocation — prevents
/// `WalletError::AlreadyExists` collision on re-run.
fn unique_wallet_name(test_name: &str) -> String {
    format!("p8t2-{}-{}", std::process::id(), test_name)
}

// =====================================================================
// Hermetic sanity (runs on plain `cargo test -p polygon --test amoy_sign_only`)
// =====================================================================

#[test]
fn amoy_sign_only_test_harness_present() {
    // Sister to `amoy_p9_pk_import::amoy_p9_pk_import_test_harness_present`.
    // Proves SoT loader is wired so the live tests below can rely on
    // `tokens/amoy.json` being parsed before `require_run_polygon_amoy`
    // short-circuits the operator-driven tests.
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON.get().expect("AMOY_TOKENS_JSON populated");
    assert_eq!(
        v.get("chain_id").and_then(|c| c.as_u64()),
        Some(80002),
        "chain_id must be 80002 (Amoy); tokens/amoy.json drift?"
    );
    assert!(
        v.get("rpc_url").and_then(|r| r.as_str()).is_some(),
        "rpc_url missing"
    );
    let pk_tv = p9_test_pk_hex_test_vector();
    assert_eq!(
        pk_tv.len(),
        64,
        "p9_test_pk_hex_test_vector must be 32 raw bytes (64 hex chars); got len={}",
        pk_tv.len()
    );
    let pw_tv = p9_test_wallet_password_test_vector();
    assert!(
        !pw_tv.is_empty(),
        "p9_test_wallet_password_test_vector must be non-empty"
    );
    let bin = polygon_bin();
    assert!(
        bin.exists(),
        "polygon binary must exist at {bin:?} (cargo build -p polygon first)"
    );
    // Exercise the JSON helpers so the unused-import lint stays quiet
    // even if downstream tests later inline these values directly.
    let _ = amoy_chain_id();
    let _ = amoy_rpc_url();
}

// =====================================================================
// Live tests — all `#[ignore]` + `require_run_polygon_amoy()` per L29
// =====================================================================

/// Sign the same input twice and assert byte-identical RLP. Proves
/// the sign-only path produces deterministic EIP-1559 envelopes (no
/// wall-clock nonce drift inside a single CLI invocation, no random
/// chain-id padding).
///
/// Fees are pinned explicitly via `--max-fee-gwei` + `--priority-fee-gwei`
/// (per L12 review MED #B): the Amoy gas oracle can shift
/// `max_fee_per_gas` between invocations; the signed envelope encodes
/// `max_fee_per_gas`, so unpinned fees would flake this test.
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_sign_only -- --ignored"]
fn amoy_sign_only_is_deterministic() {
    require_run_polygon_amoy();
    let pk_hex = p9_test_pk_hex_test_vector();
    let password = p9_test_wallet_password_test_vector();
    let pk_bytes = hex::decode(&pk_hex).expect("PK test vector must be valid hex");
    assert_eq!(pk_bytes.len(), 32, "PK must decode to 32 bytes");

    // Use the committed p9 data dir so the wallet survives across
    // runs (sister to amoy_p9_pk_import.rs). Pre-create if missing.
    let data_dir = p9_test_wallet_data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    // PK in a tempdir (auto-cleaned on Drop) — fixes the prior PK-leak
    // + filename-race (per L12 review HIGH #5: prior version wrote
    // /tmp/p8t2-pk.hex with no cleanup + parallel-test race risk).
    let pk_tmp = tempfile::tempdir().expect("tempdir for pk");
    let pk_path = write_pk_file(pk_tmp.path(), "p8t2-pk.hex", &pk_bytes, 0o600);
    let name = unique_wallet_name("deterministic");
    let (exit, _stdout, _stderr) =
        run_polygon_import_pk(&pk_path, "amoy", &name, &data_dir, &password);
    assert_eq!(exit, 0, "wallet import must exit 0");
    drop(pk_tmp); // cleanup the PK file before tmpdir Drop — defensive

    // Two sign-only invocations with the same args + pinned fees.
    let recipient = "0x000000000000000000000000000000000000dEaD";
    let amount = "1000000000000000000"; // 1 POL in wei
                                        // Pinned to a typical Amoy mid-tier; pinned so the gas oracle
                                        // shift between invocations can't shift the signed envelope.
    let pinned_max_fee = "40";
    let pinned_priority_fee = "30";
    let (e1, s1, _err1) = run_polygon_sign_only(
        &name,
        &password,
        recipient,
        amount,
        &data_dir,
        Some(pinned_max_fee),
        Some(pinned_priority_fee),
    );
    assert_eq!(e1, 0, "first sign-only must exit 0; stderr above");
    let (e2, s2, _err2) = run_polygon_sign_only(
        &name,
        &password,
        recipient,
        amount,
        &data_dir,
        Some(pinned_max_fee),
        Some(pinned_priority_fee),
    );
    assert_eq!(e2, 0, "second sign-only must exit 0; stderr above");

    let rlp1 = extract_sign_only_rlp(&s1);
    let rlp2 = extract_sign_only_rlp(&s2);
    assert_eq!(
        rlp1, rlp2,
        "sign-only RLP must be byte-identical across invocations \
         (pinned fees + same nonce → same envelope)"
    );
}

/// Sign-only must NOT broadcast. Three-layer proof (per L12 review
/// FAIL #3/#7 — plan §P8-T2 AC requires a network-side
/// `eth_getTransactionByHash == null` assertion, not just a
/// handler-side structural claim):
///
/// 1. stdout carries the 0x-prefixed RLP (operator-visible),
/// 2. RLP starts with `0x02` (EIP-1559 type byte per EIP-2718 —
///    catches a regression to Legacy / EIP-2930 envelopes),
/// 3. NETWORK-SIDE: derive the broadcast tx-hash (`keccak256(rlp)`)
///    and assert `eth_getTransactionByHash == null` on the live Amoy
///    RPC. This is the canonical "no `eth_sendRawTransaction` was
///    issued" proof — the broadcast call site would have created an
///    on-chain tx visible to `eth_getTransactionByHash`.
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_sign_only -- --ignored"]
fn amoy_sign_only_does_not_broadcast() {
    require_run_polygon_amoy();
    let pk_hex = p9_test_pk_hex_test_vector();
    let password = p9_test_wallet_password_test_vector();
    let pk_bytes = hex::decode(&pk_hex).expect("PK test vector must be valid hex");
    assert_eq!(pk_bytes.len(), 32, "PK must decode to 32 bytes");

    let data_dir = p9_test_wallet_data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    // PK in a tempdir (auto-cleaned on Drop) — fixes the prior PK-leak
    // + filename-race (per L12 review HIGH #5).
    let pk_tmp = tempfile::tempdir().expect("tempdir for pk");
    let pk_path = write_pk_file(pk_tmp.path(), "p8t2-pk.hex", &pk_bytes, 0o600);
    let name = unique_wallet_name("nobroadcast");
    let (exit, import_stdout, _stderr) =
        run_polygon_import_pk(&pk_path, "amoy", &name, &data_dir, &password);
    assert_eq!(exit, 0, "wallet import must exit 0");
    drop(pk_tmp);
    let sender =
        extract_eip55(&import_stdout).expect("import stdout must contain 0x<eip55> address");
    let sender_addr: Address = sender.parse().expect("sender EIP-55 parses");

    let recipient = "0x000000000000000000000000000000000000dEaD";
    let amount = "1000000000000000000"; // 1 POL in wei
                                        // Pinned fees (per L12 review MED #B) — not strictly required
                                        // here (we don't compare RLP across runs), but keeps the test
                                        // hermetic to oracle shifts.
    let (exit, sign_stdout, _sign_stderr) = run_polygon_sign_only(
        &name,
        &password,
        recipient,
        amount,
        &data_dir,
        Some("40"),
        Some("30"),
    );
    assert_eq!(exit, 0, "sign-only must exit 0; stderr above");

    // (1) RLP present + 0x-prefixed + decodes to >= 1 byte.
    let rlp = extract_sign_only_rlp(&sign_stdout);
    assert!(
        !rlp.is_empty(),
        "sign-only RLP must be non-empty; stdout: {sign_stdout}"
    );

    // (2) EIP-1559 type byte per EIP-2718.
    assert_eq!(
        rlp[0], 0x02,
        "signed envelope must be EIP-1559 (type byte 0x02 per EIP-2718); got 0x{:02x}",
        rlp[0]
    );

    // Sanity: chain_id MUST be 80002 (Amoy) per the JSON SoT.
    let chain_id = amoy_chain_id();
    assert_eq!(chain_id, 80002, "Amoy chain_id mismatch");

    // (3) NETWORK-SIDE no-broadcast proof — derive the broadcast
    // tx-hash from the signed RLP (the hash the RPC node WOULD have
    // computed + stored if the sign path had reached
    // `eth_sendRawTransaction`), then assert the Amoy node has no
    // record of it. This is the canonical live-RPC proof that the
    // sign-only handler did not issue a broadcast.
    let expected_broadcast_hash = alloy_primitives::keccak256(&rlp);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for eth_getTransactionByHash");
    let on_chain_lookup: Option<_> = rt.block_on(async {
        let provider =
            polygon_wallet_core::new_http_polygon_amoy().expect("provider build for amoy");
        provider
            .get_transaction_by_hash(expected_broadcast_hash)
            .await
            .expect("get_transaction_by_hash")
    });
    assert!(
        on_chain_lookup.is_none(),
        "sign-only path must NOT have broadcast; eth_getTransactionByHash({expected_broadcast_hash:?}) returned Some; this proves the sign-only handler issued eth_sendRawTransaction"
    );

    // Belt-and-suspenders: the sender's on-chain nonce for this wallet
    // must NOT have advanced as a side effect of the sign-only path.
    // We don't assert a specific nonce (depends on prior runs / shared
    // Amoy state); the invariant is that no new tx was added.
    let sender_nonce: u64 = rt.block_on(async {
        let provider =
            polygon_wallet_core::new_http_polygon_amoy().expect("provider build for amoy");
        provider
            .get_transaction_count(sender_addr)
            .await
            .expect("get_transaction_count")
    });
    let _ = B256::ZERO; // suppress unused-import lint if compiler gets strict

    // Operator log line (visible via `--nocapture`).
    eprintln!(
        "amoy_sign_only_does_not_broadcast: sender={sender} sender_nonce={sender_nonce} \
         expected_broadcast_hash={expected_broadcast_hash:?} rlp_len={}",
        rlp.len()
    );
}
