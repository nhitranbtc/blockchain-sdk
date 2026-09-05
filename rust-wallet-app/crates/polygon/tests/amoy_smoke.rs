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
// Load `${CARGO_MANIFEST_DIR}/tokens/amoy.json` once and parse it. Per
// 2026-09-02 drift note: this file is the committed SoT for ALL Amoy
// config (network + test-harness). Rust rejects top-level expressions in
// test files, so the loader runs lazily from each helper.
fn ensure_tokens_loaded() {
    use std::sync::OnceLock;
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        let path = format!("{}/tokens/amoy.json", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!("failed to read {path}: {e} — tokens/amoy.json is the committed Amoy SoT")
        });
        let parsed: serde_json::Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));
        AMOY_TOKENS_JSON
            .set(parsed)
            .expect("AMOY_TOKENS_JSON OnceLock set twice");
    });
}

static AMOY_TOKENS_JSON: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Pull a JSON string field out of the loaded Amoy config (panics on
/// missing/wrong-type so tests fail loudly on config drift).
fn amoy_json_str(key: &str) -> String {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or_else(|| panic!("missing string field `{key}` in tokens/amoy.json"))
        .to_string()
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
/// operator to pre-fund this address on Amoy before running. Config
/// source-of-truth lives in `polygon/tokens/amoy.json` (loaded via
/// `ensure_tokens_loaded()`), key `test_harness.amoy_funded_pk_hex`.
/// Per 2026-09-02 drift note: no `.env` files anymore.
///
/// **Callers MUST** hold `PK_LOCK` while reading + immediately
/// `std::env::remove_var("AMOY_FUNDED_PK_HEX")` after capture (the
/// PK-using test below does this in a scoped block — see
/// `amoy_wallet_import_via_pk_file`).
///
/// Default EIP-55 address (for the Anvil-#0 test vector shipped in
/// `tokens/amoy.json`): `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`.
fn amoy_funded_pk_hex() -> String {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get("test_harness")
        .and_then(|t| t.get("amoy_funded_pk_hex"))
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| {
                panic!(
                    "missing test_harness.amoy_funded_pk_hex in tokens/amoy.json — add the Anvil-#0 test vector"
                )
            })
        .to_string()
}

/// Read the shared Amoy wallet password from `tokens/amoy.json:test_harness.amoy_wallet_password`.
/// Used as `POLYGON_PASSWORD` env for every subprocess — must match the password
/// the pre-existing wallets in `amoy_wallet_data_dir()` were encrypted with
/// (sister to `amoy_erc20_send.rs::amoy_wallet_password`).
fn amoy_wallet_password() -> String {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get("test_harness")
        .and_then(|t| t.get("amoy_wallet_password"))
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| {
            panic!("missing `test_harness.amoy_wallet_password` in tokens/amoy.json")
        })
        .to_string()
}

/// Read the native-POL sender wallet name from
/// `tokens/amoy.json:test_harness.amoy_native_sender`.
fn amoy_native_sender_name() -> String {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get("test_harness")
        .and_then(|t| t.get("amoy_native_sender"))
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| panic!("missing `test_harness.amoy_native_sender` in tokens/amoy.json"))
        .to_string()
}

/// Read the native-POL recipient wallet name from
/// `tokens/amoy.json:test_harness.amoy_native_recipient`.
#[allow(dead_code)] // reserved for future native-POL round-trip test
fn amoy_native_recipient_name() -> String {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    v.get("test_harness")
        .and_then(|t| t.get("amoy_native_recipient"))
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| {
            panic!("missing `test_harness.amoy_native_recipient` in tokens/amoy.json")
        })
        .to_string()
}

/// Resolve the configured wallet data directory (relative or absolute).
/// Relative paths resolve against the repo root so the test works
/// regardless of cwd.
fn amoy_wallet_data_dir() -> std::path::PathBuf {
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    let raw = v
        .get("test_harness")
        .and_then(|t| t.get("amoy_wallet_data_dir"))
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| {
            panic!("missing `test_harness.amoy_wallet_data_dir` in tokens/amoy.json")
        });
    let p = std::path::PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest
            .parent()
            .and_then(|x| x.parent())
            .and_then(|x| x.parent())
            .map(|root| root.join(&p))
            .unwrap_or(p)
    }
}

/// Invoke `polygon` as a subprocess with hermetic env.
fn run_polygon(args: &[&str], data_dir: &std::path::Path) -> std::process::Output {
    Command::new(polygon_bin())
        .args(args)
        .arg("--data-dir")
        .arg(data_dir)
        .env("POLYGON_PASSWORD", amoy_wallet_password())
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
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON
        .get()
        .expect("AMOY_TOKENS_JSON set by ensure_tokens_loaded");
    let opt = v
        .get("test_harness")
        .and_then(|t| t.get("run_polygon_amoy"))
        .and_then(|s| s.as_str());
    if opt != Some("1") {
        panic!(
            "tokens/amoy.json test_harness.run_polygon_amoy must be \"1\" for live test runs; \
             current = {opt:?}"
        );
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
            "amoy-native-sender",
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

/// Story 5 (sister to PK-file path) — `polygon wallet send` against
/// the operator-funded `amoy-native-sender` wallet (pre-existing in
/// `amoy_wallet_data_dir`, configured in `tokens/amoy.json`). Sends
/// 0.001 POL to a deterministic sink address; asserts the CLI exits 0
/// and stdout confirms the tx hash. `network amoy` default per
/// cli.rs:165.
///
/// Uses the configured native-sender wallet (NOT the Anvil-#0
/// placeholder PK from `tokens/amoy.json:test_harness.amoy_funded_pk_hex`)
/// — the Anvil-#0 address has no real POL on Amoy and previously
/// failed with `insufficient funds`. The configured sender is the
/// operator-funded wallet this test infrastructure was designed around
/// (sister to `amoy_erc20_send.rs`'s native-POL test). `POLYGON_PASSWORD`
/// env comes from `amoy_wallet_password()` so it decrypts the
/// pre-existing wallet blob in `amoy_wallet_data_dir`.
///
/// Amount lowered from 0.01 → 0.001 POL after the live Amoy gas-price
/// spike (Amoy oracle returned ~2.6M gwei × 67.2M gas ≈ 0.18 POL per
/// send, eating most of the sender's dust balance). 0.001 POL value
/// keeps the total tx cost under the sender's funded balance while
/// still exercising the full send-sign-broadcast flow.
#[test]
#[ignore]
fn amoy_send_native_after_pk_wallet_import() {
    require_run_polygon_amoy();
    let data_dir = amoy_wallet_data_dir();
    let sender_name = amoy_native_sender_name();
    let out = run_polygon(
        &[
            "wallet",
            "send",
            "--name",
            &sender_name,
            "--to",
            "0x0000000000000000000000000000000000000042",
            "--amount",
            "0.001", // 0.001 POL; CLI `--unit pol` default (cli.rs:260) — handler still receives wei after main.rs conversion (main.rs:660)
            "--network",
            "amoy",
        ],
        &data_dir,
    );
    assert!(
        out.status.success(),
        "wallet send 0.001 POL failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("tx") || stdout.contains("0x"),
        "send stdout should confirm transaction; got: {stdout}"
    );
}

// ── Sanity check (tokens/amoy.json SoT verification, NOT #[ignore]) ───────
//
// Proves the tokens/amoy.json load chain end-to-end WITHOUT hitting live
// RPC. Lives alongside the 5 #[ignore] amoy tests so it runs on plain
// `cargo test -p polygon --test amoy_smoke` (no `--ignored`, no live
// RPC, no operator setup beyond having `tokens/amoy.json` populated —
// CI parity).
//
// Pass criterion: each field below matches the value in `tokens/amoy.json`.
// If any value drifts, the test fails loudly — preventing silent config
// drift across the integration suite.
#[test]
fn amoy_tokens_load() {
    ensure_tokens_loaded();

    // RPC URL — comes from `tokens/amoy.json` (NOT a Rust literal).
    // The test source itself has no RPC URL hardcoded; if the JSON is
    // missing or malformed, the panic message points to the JSON path.
    let rpc = amoy_json_str("rpc_url");
    assert_eq!(
        rpc, "https://polygon-amoy-bor-rpc.publicnode.com",
        "rpc_url from tokens/amoy.json must match plan §Network Configuration Amoy RPC (Q4 drift)"
    );

    // Chain ID + USDC address — same pattern, JSON is the source-of-truth.
    // `chain_id` is a JSON number (80002), not a string.
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON.get().expect("set");
    let chain_id = v
        .get("chain_id")
        .and_then(|x| x.as_u64())
        .unwrap_or_else(|| panic!("missing numeric field `chain_id` in tokens/amoy.json"));
    assert_eq!(chain_id, 80002, "chain_id must be 80002 (Amoy)");

    // Pull from tokens[] entry by symbol for USDC assertion.
    ensure_tokens_loaded();
    let v = AMOY_TOKENS_JSON.get().expect("set");
    let usdc_addr = v
        .get("tokens")
        .and_then(|t| t.as_array())
        .and_then(|a| {
            a.iter()
                .find(|t| t.get("symbol").and_then(|s| s.as_str()) == Some("USDC"))
        })
        .and_then(|t| t.get("address"))
        .and_then(|a| a.as_str())
        .unwrap_or_else(|| panic!("tokens[].address for USDC missing in tokens/amoy.json"));
    assert_eq!(
        usdc_addr, "0x8B0180f2101c8260d49339abfEe87927412494B4",
        "tokens[].address (USDC) must match plan §Network Configuration USDC contract"
    );

    // PK — proves `amoy_funded_pk_hex()` reads from JSON (no embedded literal).
    // Uses the fn (not direct JSON access) so the call path is exercised
    // exactly the way the live tests use it.
    let pk = amoy_funded_pk_hex();
    assert_eq!(
        pk, "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        "test_harness.amoy_funded_pk_hex from tokens/amoy.json must match the Anvil-#0 test vector"
    );

    // Opt-in guard — must read "1" if JSON is populated; the live tests
    // use this exact field to gate themselves per L29.
    require_run_polygon_amoy(); // panics on != "1"
}
