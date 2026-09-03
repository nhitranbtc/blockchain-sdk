//! P9-T-import-pk live-RPC smoke (operator-driven per L29) — issue #527
//! sister-issue to #528 (PR #529 landed `--mnemonic-file`).
//!
//! **Opt in (CI-safe by default):**
//!   RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored
//!
//! **Scope:** operator-driven integration tests for the P9-T-import-pk
//! test cases (plan §P9-T-import-pk — `docs/superpowers/engineering/
//! 2026-09-02-polygon-amoy-test-plan.md`). Subset of the 21-case table;
//! hermetic cases (mode check, conflict class, missing/empty file) live
//! in `polygon/src/handlers/wallet.rs` test mod per PR #529 (commit
//! `916f010`); this file exercises the LIVE operator-driven surface:
//!   - case 1: PK file happy path
//!   - case 2: PK file via stdin heredoc (L54)
//!   - case 5: mnemonic file happy path (post-PR-#529)
//!   - case 7: mnemonic file multiline heredoc layout
//!   - case 14: argv mnemonic baseline (L12 H-1 documented caveat)
//!   - case 16: account-index non-zero produces different address
//!   - case 17: wallet list ↔ wallet show address cross-check
//!
//! **Operator pre-conditions** (per L29 + L54):
//! - `tokens/amoy.json` committed SoT present at
//!   `${CARGO_MANIFEST_DIR}/tokens/amoy.json`.
//! - `RUN_POLYGON_AMOY=1` env opt-in.
//! - `P9_TEST_PK_HEX` env var: 32-byte secp256k1 PK as bare hex
//!   (no `0x` prefix). Operator exports from MetaMask → Account → ⋮
//!   → Account details → Export private key.
//! - `P9_TEST_MNEMONIC` env var: 12- or 24-word BIP-39 mnemonic
//!   (single-line, space-separated). Operator exports from MetaMask
//!   → Settings → Security & privacy → Reveal Secret Recovery Phrase.
//! - `P9_WALLET_PASSWORD` env var: strong password used by both
//!   import paths (lib encrypts wallet blob with Argon2id + AES-GCM
//!   keyed by this password). Operator must supply — never persisted
//!   to git, never echoed in chat (L12 H-1 + L54).
//! - Operator-funded target addresses (Polygon faucet for native POL
//!   + Circle faucet for USDC). For balance tests only.
//! - Run in a private shell with cleared history
//!   (`history -c && set +o history`) so env-var secrets don't leak
//!   to shell history (L54).
//!
//! **PK capture defense-in-depth** (L54 sister pattern): `static
//! PK_LOCK: Mutex<()>` serializes tests that read env-var secrets so
//! `cargo test`'s parallel runner can't double-read the PK; each test
//! captures then immediately `std::env::remove_var("P9_TEST_PK_HEX")`
//! to drop the secret from process env (sister to
//! `amoy_wallet_import_via_pk_file` at `amoy_smoke.rs:235-243`).
//!
//! **TDD status:** RED by default — every operator-driven test carries
//! `#[ignore] + require_run_polygon_amoy()`. Hermetic sanity test
//! `amoy_p9_pk_import_test_harness_present` runs on plain
//! `cargo test -p polygon --test amoy_p9_pk_import` (no `--ignored`,
//! no live RPC). Sister to `amoy_tokens_load` at
//! `amoy_smoke.rs:355-405`.

#![cfg(test)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use alloy_primitives::hex;
use serde_json::Value;

// =====================================================================
// Shared helpers + fixtures (sister to amoy_smoke.rs:40-130)
// =====================================================================

/// Load `${CARGO_MANIFEST_DIR}/tokens/amoy.json` once and parse it.
/// Per 2026-09-02 drift note: this file is the committed SoT for ALL
/// Amoy config (network + test-harness). Rust rejects top-level
/// expressions in test files, so the loader runs lazily from each
/// helper via `OnceLock::get_or_init`.
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

/// Read the Anvil-#0 funded-PK hex from the committed `tokens/amoy.json`
/// SoT. Panics if unset (no embedded literal — config source-of-truth
/// = JSON `test_harness.amoy_funded_pk_hex`). EIP-55 addr
/// `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` documented as comment.
fn amoy_funded_pk_hex() -> String {
    ensure_tokens_loaded();
    AMOY_TOKENS_JSON
        .get()
        .and_then(|v| v.get("test_harness"))
        .and_then(|t| t.get("amoy_funded_pk_hex"))
        .and_then(|p| p.as_str())
        .map(String::from)
        .unwrap_or_else(|| panic!("tokens/amoy.json missing test_harness.amoy_funded_pk_hex"))
}

/// Read the env-var name that the operator uses to supply their
/// MetaMask-exported PK (32 raw bytes, hex, no `0x` prefix). The
/// JSON SoT holds the name; the actual PK value is operator-supplied
/// Read the canonical wallet data dir for P9 tests from the JSON
/// SoT. Used by the integration test's `--data-dir` flag (sandbox
/// away from the operator's real `~/.local/share/polygon/wallets`).
fn p9_test_wallet_data_dir() -> PathBuf {
    ensure_tokens_loaded();
    let s = AMOY_TOKENS_JSON
        .get()
        .and_then(|v| v.get("test_harness"))
        .and_then(|t| t.get("p9_test_wallet_data_dir"))
        .and_then(|p| p.as_str())
        .unwrap_or_else(|| panic!("tokens/amoy.json missing test_harness.p9_test_wallet_data_dir"));
    PathBuf::from(s)
}

/// Read the committed PK test vector (32 raw bytes, hex, no `0x`
/// prefix). Sister to `amoy_funded_pk_hex()` — the Anvil-#0 fixture
/// is the historical precedent for committing test vectors to
/// `tokens/amoy.json`; this is the P9 variant for MetaMask-exported
/// PK round-trip tests. Operator-supplied `P9_TEST_PK_HEX` env var
/// overrides this default (L54 + operator-driven run).
fn p9_test_pk_hex_test_vector() -> String {
    ensure_tokens_loaded();
    AMOY_TOKENS_JSON
        .get()
        .and_then(|v| v.get("test_harness"))
        .and_then(|t| t.get("p9_test_pk_hex_test_vector"))
        .and_then(|p| p.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            panic!("tokens/amoy.json missing test_harness.p9_test_pk_hex_test_vector")
        })
}

/// Read the committed mnemonic test vector (12/24-word BIP-39,
/// space-separated). Sister to `p9_test_pk_hex_test_vector`.
fn p9_test_mnemonic_test_vector() -> String {
    ensure_tokens_loaded();
    AMOY_TOKENS_JSON
        .get()
        .and_then(|v| v.get("test_harness"))
        .and_then(|t| t.get("p9_test_mnemonic_test_vector"))
        .and_then(|p| p.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            panic!("tokens/amoy.json missing test_harness.p9_test_mnemonic_test_vector")
        })
}

/// Read the committed wallet password test vector. Sister to
/// `p9_test_pk_hex_test_vector` + `p9_test_mnemonic_test_vector`.
/// **⚠ LEAK ACKNOWLEDGMENT:** this password unlocks the wallet
/// blob (Argon2id + AES-GCM keyed by this value) — anyone with
/// the committed PK + mnemonic + password has full signing
/// authority over the funded test address. Operator accepted this
/// trade-off 2026-09-03 to enable fully-self-sufficient live
/// tests without env-var secret materialization. Same precedent
/// as `amoy_funded_pk_hex` (Anvil-#0 fixture).
fn p9_test_wallet_password_test_vector() -> String {
    ensure_tokens_loaded();
    AMOY_TOKENS_JSON
        .get()
        .and_then(|v| v.get("test_harness"))
        .and_then(|t| t.get("p9_test_wallet_password_test_vector"))
        .and_then(|p| p.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            panic!("tokens/amoy.json missing test_harness.p9_test_wallet_password_test_vector")
        })
}

/// L29 opt-in guard — panics unless `RUN_POLYGON_AMOY=1`. Reads the env
/// via `std::env::var` so the JSON SoT (above) and the operator-supplied
/// override are both honored.
fn require_run_polygon_amoy() {
    let v = std::env::var("RUN_POLYGON_AMOY").unwrap_or_default();
    assert_eq!(
        v, "1",
        "RUN_POLYGON_AMOY must be '1' to run live Amoy tests (L29)"
    );
}

/// `polygon` binary path (compile-time cargo metadata; no env reading).
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

/// Sister to `write_pk_file` for mnemonic file.
#[cfg(unix)]
fn write_mnemonic_file(dir: &std::path::Path, name: &str, words: &str, mode: u32) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, words).expect("write mnemonic file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .expect("set mnemonic file mode");
    path
}

/// L54 defense-in-depth — captures the env-var secrets under this
/// mutex so parallel tests don't race-read them.
static PK_LOCK: Mutex<()> = Mutex::new(());

// =====================================================================
// Operator-driven helpers (env-reading for the L29 + L54 surface)
// =====================================================================

/// Capture `P9_TEST_PK_HEX` from env, then immediately `remove_var`
/// per L54 (read + immediate drop). Falls back to the committed
/// JSON test vector (`p9_test_pk_hex_test_vector`) when the env var
/// is unset — same Anvil-#0 pattern as `amoy_funded_pk_hex`. Caller
/// MUST hold `PK_LOCK` for the duration of the env-var use so
/// concurrent tests can't double-read. Env-var name read from JSON
/// SoT (`p9_test_pk_hex_env_var()`).
fn p9_capture_pk_hex() -> String {
    let _guard = PK_LOCK.lock().expect("PK_LOCK poisoned");
    let var = "P9_TEST_PK_HEX";
    let v = std::env::var(var)
        .ok()
        .or_else(|| {
            let tv = p9_test_pk_hex_test_vector();
            (!tv.is_empty()).then_some(tv)
        })
        .unwrap_or_else(|| {
            panic!("{var} must be set or p9_test_pk_hex_test_vector must be non-empty (L29)")
        });
    std::env::remove_var(var);
    v
}

/// Sister to `p9_capture_pk_hex` for the mnemonic.
fn p9_capture_mnemonic() -> String {
    let _guard = PK_LOCK.lock().expect("PK_LOCK poisoned");
    let var = "P9_TEST_MNEMONIC";
    let v = std::env::var(var)
        .ok()
        .or_else(|| {
            let tv = p9_test_mnemonic_test_vector();
            (!tv.is_empty()).then_some(tv)
        })
        .unwrap_or_else(|| {
            panic!("{var} must be set or p9_test_mnemonic_test_vector must be non-empty (L29)")
        });
    std::env::remove_var(var);
    v
}

/// Sister to `p9_capture_pk_hex` for the wallet password. Falls
/// back to the committed JSON test vector
/// (`p9_test_wallet_password_test_vector`) when the env var is
/// unset — operator accepted this leak surface 2026-09-03 (see
/// `p9_test_wallet_password_test_vector` doc-comment for the
/// security implications: anyone with PK + mnemonic + password
/// has full signing authority over the funded test address).
fn p9_capture_wallet_password() -> String {
    let _guard = PK_LOCK.lock().expect("PK_LOCK poisoned");
    let var = "P9_WALLET_PASSWORD";
    let v = std::env::var(var)
        .ok()
        .or_else(|| {
            let tv = p9_test_wallet_password_test_vector();
            (!tv.is_empty()).then_some(tv)
        })
        .unwrap_or_else(|| {
            panic!("{var} must be set or p9_test_wallet_password_test_vector must be non-empty")
        });
    std::env::remove_var(var);
    v
}

/// Spawn `polygon wallet import` with the given secret-supplying
/// strategy + assert exit code + capture stdout for downstream
/// assertions. Sister pattern to `amoy_wallet_import_via_pk_file`
/// at `amoy_smoke.rs:235-243`.
fn run_polygon_import(
    pk_path: &std::path::Path,
    network: &str,
    wallet_name: &str,
    data_dir: &std::path::Path,
    password: &str,
) -> (i32, String) {
    let mut cmd = Command::new(polygon_bin());
    cmd.env("POLYGON_PASSWORD", password)
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
        .stderr(Stdio::piped());
    let output = cmd.output().expect("spawn polygon wallet import");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit = output.status.code().unwrap_or(-1);
    if exit != 0 {
        eprintln!("polygon wallet import failed (exit {exit})\nstdout: {stdout}\nstderr: {stderr}");
    }
    (exit, stdout)
}

/// Sister to `run_polygon_import` for mnemonic file path.
fn run_polygon_import_mnemonic(
    mnemonic_path: &std::path::Path,
    network: &str,
    wallet_name: &str,
    data_dir: &std::path::Path,
    password: &str,
) -> (i32, String) {
    let mut cmd = Command::new(polygon_bin());
    cmd.env("POLYGON_PASSWORD", password)
        .arg("wallet")
        .arg("import")
        .arg("--mnemonic-file")
        .arg(mnemonic_path)
        .arg("--name")
        .arg(wallet_name)
        .arg("--network")
        .arg(network)
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .expect("spawn polygon wallet import --mnemonic-file");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit = output.status.code().unwrap_or(-1);
    if exit != 0 {
        eprintln!("polygon wallet import --mnemonic-file failed (exit {exit})\nstdout: {stdout}\nstderr: {stderr}");
    }
    (exit, stdout)
}

/// Sister to `run_polygon_import` for `--mnemonic` argv (legacy baseline).
fn run_polygon_import_mnemonic_argv(
    mnemonic: &str,
    network: &str,
    wallet_name: &str,
    data_dir: &std::path::Path,
    password: &str,
) -> (i32, String) {
    let mut cmd = Command::new(polygon_bin());
    cmd.env("POLYGON_PASSWORD", password)
        .arg("wallet")
        .arg("import")
        .arg("--mnemonic")
        .arg(mnemonic)
        .arg("--name")
        .arg(wallet_name)
        .arg("--network")
        .arg(network)
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .expect("spawn polygon wallet import --mnemonic");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit = output.status.code().unwrap_or(-1);
    if exit != 0 {
        eprintln!("polygon wallet import --mnemonic failed (exit {exit})\nstdout: {stdout}\nstderr: {stderr}");
    }
    (exit, stdout)
}

/// Extract the first `0x[0-9a-fA-F]{40}` substring from `s` (the
/// EIP-55 address polygon CLI prints in `wallet imported: ...` stdout).
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

/// Unique wallet name per test invocation — prevents
/// `WalletError::AlreadyExists` collision when the operator re-runs
/// the suite. Uses process PID + test name hash.
fn unique_wallet_name(test_name: &str) -> String {
    format!("p9-{}-{}", std::process::id(), test_name)
}

// =====================================================================
// Hermetic sanity (runs on plain `cargo test -p polygon --test amoy_p9_pk_import`)
// =====================================================================

#[test]
fn amoy_p9_pk_import_test_harness_present() {
    // Sister to `amoy_tokens_load` at `amoy_smoke.rs:355-405`. Proves
    // the SoT loader is wired correctly so the live tests below can
    // rely on `tokens/amoy.json` being parsed before `require_run_polygon_amoy`
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
    let pk_hex = amoy_funded_pk_hex();
    assert_eq!(
        pk_hex.len(),
        64,
        "amoy_funded_pk_hex must be 32 raw bytes (64 hex chars); got len={}",
        pk_hex.len()
    );
    let bin = polygon_bin();
    assert!(
        bin.exists(),
        "polygon binary must exist at {bin:?} (cargo build -p polygon first)"
    );
    // P9 test harness fields (issue #527 + PR #529) — canonical
    // data-dir documented in the JSON SoT. Operator supplies the
    // actual secret VALUES at run time (L12 H-1 + L54); only the
    // data-dir path lands in the committed JSON.
    let data_dir = p9_test_wallet_data_dir();
    assert!(
        data_dir.is_absolute(),
        "p9_test_wallet_data_dir must be absolute; got {data_dir:?}"
    );
    // Test vectors (Anvil-#0 precedent) — committed to JSON per
    // operator choice 2026-09-03 (same leak surface as
    // `amoy_funded_pk_hex`). Hermetic sanity asserts they parse +
    // are well-formed (hex chars only; word-count for mnemonic).
    let pk_tv = p9_test_pk_hex_test_vector();
    assert_eq!(
        pk_tv.len(),
        64,
        "p9_test_pk_hex_test_vector must be 32 raw bytes (64 hex chars); got len={}",
        pk_tv.len()
    );
    assert!(
        pk_tv.chars().all(|c| c.is_ascii_hexdigit()),
        "p9_test_pk_hex_test_vector must be hex-only; got: {pk_tv}"
    );
    let mn_tv = p9_test_mnemonic_test_vector();
    let word_count = mn_tv.split_whitespace().count();
    assert!(
        word_count == 12 || word_count == 15 || word_count == 18 || word_count == 21 || word_count == 24,
        "p9_test_mnemonic_test_vector must be a valid BIP-39 length (12/15/18/21/24); got {word_count} words"
    );
    // Password test vector (operator accepted leak 2026-09-03 to make
    // live tests self-sufficient). Hermetic sanity asserts non-empty.
    let pw_tv = p9_test_wallet_password_test_vector();
    assert!(
        !pw_tv.is_empty(),
        "p9_test_wallet_password_test_vector must be non-empty"
    );
}

// =====================================================================
// Live tests — all `#[ignore]` + `require_run_polygon_amoy()` per L29
// =====================================================================

/// Case 1 — PK happy path via mode-0600 file.
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored"]
fn p9_pk_file_happy_path() {
    require_run_polygon_amoy();
    let pk_hex = p9_capture_pk_hex();
    let password = p9_capture_wallet_password();
    let raw = hex::decode(&pk_hex).expect("P9_TEST_PK_HEX must be valid hex");
    assert_eq!(raw.len(), 32, "P9_TEST_PK_HEX must decode to 32 bytes");
    let tmp = tempfile::tempdir().expect("tempdir");
    let pk_path = write_pk_file(tmp.path(), "p9-pk.hex", &raw, 0o600);
    let name = unique_wallet_name("pk-file");
    let (exit, stdout) = run_polygon_import(&pk_path, "amoy", &name, tmp.path(), &password);
    assert_eq!(exit, 0, "wallet import must exit 0; stderr above");
    let addr = extract_eip55(&stdout).expect("stdout must contain 0x<eip55>");
    // Sanity: 42-char EIP-55 (0x + 40 hex).
    assert_eq!(addr.len(), 42);
}

/// Case 2 — PK via stdin heredoc (L54 — no /tmp artifact).
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored"]
fn p9_pk_file_stdin_heredoc() {
    require_run_polygon_amoy();
    let pk_hex = p9_capture_pk_hex();
    let password = p9_capture_wallet_password();
    let raw = hex::decode(&pk_hex).expect("P9_TEST_PK_HEX must be valid hex");
    assert_eq!(raw.len(), 32);
    let tmp = tempfile::tempdir().expect("tempdir");
    // Pipe the raw PK bytes to `polygon wallet import
    // --private-key-file /dev/stdin` via stdin. Sister to L54
    // pattern documented at `amoy_smoke.rs:235-243` but via
    // /dev/stdin instead of a tempfile (no /tmp artifact on disk
    // after the test — the kernel reclaims the pipe buffer).
    let mut cmd = Command::new(polygon_bin());
    cmd.env("POLYGON_PASSWORD", &password)
        .arg("wallet")
        .arg("import")
        .arg("--private-key-file")
        .arg("/dev/stdin")
        .arg("--name")
        .arg(unique_wallet_name("pk-stdin"))
        .arg("--network")
        .arg("amoy")
        .arg("--data-dir")
        .arg(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn polygon wallet import stdin");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("stdin pipe")
        .write_all(&raw)
        .expect("stdin write");
    let output = child.wait_with_output().expect("wait polygon");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdin heredoc import must exit 0"
    );
    let addr = extract_eip55(&stdout).expect("stdout must contain 0x<eip55>");
    assert_eq!(addr.len(), 42);
}

/// Case 5 — mnemonic happy path via `--mnemonic-file` (post-PR-#529).
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored"]
fn p9_mnemonic_file_happy_path_12_word() {
    require_run_polygon_amoy();
    let mnemonic = p9_capture_mnemonic();
    let password = p9_capture_wallet_password();
    let word_count = mnemonic.split_whitespace().count();
    assert!(
        word_count == 12
            || word_count == 15
            || word_count == 18
            || word_count == 21
            || word_count == 24,
        "P9_TEST_MNEMONIC must be a valid BIP-39 length (12/15/18/21/24); got {word_count} words"
    );
    let tmp = tempfile::tempdir().expect("tempdir");
    let mnemonic_path = write_mnemonic_file(tmp.path(), "p9-mnemonic.txt", &mnemonic, 0o600);
    let name = unique_wallet_name("mn-file");
    let (exit, stdout) =
        run_polygon_import_mnemonic(&mnemonic_path, "amoy", &name, tmp.path(), &password);
    assert_eq!(exit, 0, "wallet import --mnemonic-file must exit 0");
    let addr = extract_eip55(&stdout).expect("stdout must contain 0x<eip55>");
    assert_eq!(addr.len(), 42);
    // Best-effort cleanup: wipe the mnemonic file before tmpdir drops.
    let _ = std::fs::remove_file(&mnemonic_path);
}

/// Case 7 — multiline heredoc layout (inter-word newlines preserved).
/// Imports the same mnemonic as case 5 but with one word per line;
/// the resulting address must match the single-line case (the lib's
/// BIP-39 parser splits on Unicode whitespace, so inter-word newlines
/// are equivalent to spaces). Sister to the hermetic handler-side test
/// `read_mnemonic_file_accepts_multiline_heredoc_layout`.
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored"]
fn p9_mnemonic_file_multiline_heredoc_layout() {
    require_run_polygon_amoy();
    let mnemonic = p9_capture_mnemonic();
    let password = p9_capture_wallet_password();
    let multiline = mnemonic
        .split_whitespace()
        .map(|w| format!("{w}\n"))
        .collect::<String>();
    let tmp = tempfile::tempdir().expect("tempdir");
    let mnemonic_path =
        write_mnemonic_file(tmp.path(), "p9-mnemonic-multiline.txt", &multiline, 0o600);
    let name = unique_wallet_name("mn-multi");
    let (exit, stdout) =
        run_polygon_import_mnemonic(&mnemonic_path, "amoy", &name, tmp.path(), &password);
    assert_eq!(exit, 0, "multiline mnemonic file import must exit 0");
    let addr = extract_eip55(&stdout).expect("stdout must contain 0x<eip55>");
    assert_eq!(addr.len(), 42);
    let _ = std::fs::remove_file(&mnemonic_path);
}

/// Case 14 — argv mnemonic baseline (legacy pre-#528).
/// DOCUMENTS THE L12 H-1 CAVEAT — the mnemonic passes through argv,
/// visible via `/proc/<pid>/cmdline` for the polygon's full process
/// lifetime. Test passes when import succeeds; the L12 H-1 risk is
/// captured in the test name + ignore reason.
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored"]
fn p9_mnemonic_argv_baseline_l12_h1_caveat() {
    require_run_polygon_amoy();
    let mnemonic = p9_capture_mnemonic();
    let password = p9_capture_wallet_password();
    let tmp = tempfile::tempdir().expect("tempdir");
    let name = unique_wallet_name("mn-argv");
    let (exit, stdout) =
        run_polygon_import_mnemonic_argv(&mnemonic, "amoy", &name, tmp.path(), &password);
    assert_eq!(exit, 0, "argv mnemonic import must exit 0");
    let addr = extract_eip55(&stdout).expect("stdout must contain 0x<eip55>");
    assert_eq!(addr.len(), 42);
    // ⚠ L12 H-1: mnemonic was visible in polygon's argv for the
    // full process lifetime. Operator must treat this mnemonic as
    // burned — do not reuse. Test runner should also `unset
    // P9_TEST_MNEMONIC` before this test exits (caller's
    // responsibility, since `p9_capture_mnemonic` already removed
    // it from env via L54 remove_var).
}

/// Case 16 — `account-index` non-zero produces a different address
/// than the default index 0. Uses the same mnemonic but `--account-index 1`.
/// Asserts inequality (doesn't require deriving the expected address —
/// just that the lib honors the index). Sister to the BIP-44
/// derivation path documented in plan §P9-T-import-pk.
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored"]
fn p9_account_index_nonzero_changes_address() {
    require_run_polygon_amoy();
    let mnemonic = p9_capture_mnemonic();
    let password = p9_capture_wallet_password();
    let tmp = tempfile::tempdir().expect("tempdir");
    // First import: default index 0.
    let path_0 = write_mnemonic_file(tmp.path(), "p9-mn-idx0.txt", &mnemonic, 0o600);
    let name_0 = unique_wallet_name("idx-0");
    let (exit_0, stdout_0) =
        run_polygon_import_mnemonic_with_index(&path_0, "amoy", &name_0, 0, tmp.path(), &password);
    assert_eq!(exit_0, 0);
    let addr_0 = extract_eip55(&stdout_0).expect("addr_0");
    // Second import: index 1.
    let path_1 = write_mnemonic_file(tmp.path(), "p9-mn-idx1.txt", &mnemonic, 0o600);
    let name_1 = unique_wallet_name("idx-1");
    let (exit_1, stdout_1) =
        run_polygon_import_mnemonic_with_index(&path_1, "amoy", &name_1, 1, tmp.path(), &password);
    assert_eq!(exit_1, 0);
    let addr_1 = extract_eip55(&stdout_1).expect("addr_1");
    assert_ne!(
        addr_0, addr_1,
        "BIP-44 path m/44'/60'/0'/0/0 vs m/44'/60'/0'/0/1 must yield different addresses"
    );
    let _ = std::fs::remove_file(&path_0);
    let _ = std::fs::remove_file(&path_1);
}

/// Sister helper to `run_polygon_import_mnemonic` with `--account-index`.
fn run_polygon_import_mnemonic_with_index(
    mnemonic_path: &std::path::Path,
    network: &str,
    wallet_name: &str,
    account_index: u32,
    data_dir: &std::path::Path,
    password: &str,
) -> (i32, String) {
    let mut cmd = Command::new(polygon_bin());
    cmd.env("POLYGON_PASSWORD", password)
        .arg("wallet")
        .arg("import")
        .arg("--mnemonic-file")
        .arg(mnemonic_path)
        .arg("--name")
        .arg(wallet_name)
        .arg("--network")
        .arg(network)
        .arg("--account-index")
        .arg(account_index.to_string())
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .expect("spawn polygon wallet import --mnemonic-file --account-index");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit = output.status.code().unwrap_or(-1);
    if exit != 0 {
        eprintln!("import --account-index {account_index} failed (exit {exit})\nstdout: {stdout}\nstderr: {stderr}");
    }
    (exit, stdout)
}

/// Case 17 — address verification cross-check: `wallet list --json`
/// contains the imported name + `wallet show --name` returns an EIP-55
/// address. Both must succeed + the name must match. Operator's manual
/// cross-check against MetaMask is outside this test (the test asserts
/// the CLI contract; MetaMask is the operator's responsibility per
/// case 17's plan section).
#[test]
#[ignore = "operator-driven live Amoy RPC per L29 — run with: RUN_POLYGON_AMOY=1 cargo test -p polygon --test amoy_p9_pk_import -- --ignored"]
fn p9_address_verification_cross_check() {
    require_run_polygon_amoy();
    let mnemonic = p9_capture_mnemonic();
    let password = p9_capture_wallet_password();
    let tmp = tempfile::tempdir().expect("tempdir");
    let mnemonic_path = write_mnemonic_file(tmp.path(), "p9-mn-xcheck.txt", &mnemonic, 0o600);
    let name = unique_wallet_name("xcheck");
    let (exit, stdout) =
        run_polygon_import_mnemonic(&mnemonic_path, "amoy", &name, tmp.path(), &password);
    assert_eq!(exit, 0);
    let imported_addr = extract_eip55(&stdout).expect("imported stdout must contain 0x<eip55>");
    let _ = std::fs::remove_file(&mnemonic_path);

    // `wallet list --json` returns an array of wallet names. Assert the
    // imported name is present (sister to the live `amoy_smoke.rs`
    // wallet-list parsing).
    let list_out = Command::new(polygon_bin())
        .arg("wallet")
        .arg("list")
        .arg("--json")
        .arg("--network")
        .arg("amoy")
        .arg("--data-dir")
        .arg(tmp.path())
        .output()
        .expect("spawn wallet list --json");
    assert_eq!(list_out.status.code(), Some(0));
    let list_stdout = String::from_utf8_lossy(&list_out.stdout).into_owned();
    let parsed: Value = serde_json::from_str(&list_stdout).unwrap_or_else(|e| {
        panic!("wallet list --json must parse as JSON: {e}; raw: {list_stdout}")
    });
    let arr = parsed
        .as_array()
        .expect("wallet list --json must be an array");
    assert!(
        arr.iter().any(|v| v.as_str() == Some(&name)),
        "wallet list --json must include imported name {name}; got: {list_stdout}"
    );

    // `wallet show --name <imported> --json` must return the same
    // EIP-55 address we captured from the import stdout. The CLI
    // prints address only in `--json` mode (T6c3 follow-up deferred
    // the human-readable address printing to a later PR).
    let show_out = Command::new(polygon_bin())
        .env("POLYGON_PASSWORD", &password)
        .arg("wallet")
        .arg("show")
        .arg("--name")
        .arg(&name)
        .arg("--network")
        .arg("amoy")
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("--json")
        .output()
        .expect("spawn wallet show --json");
    assert_eq!(show_out.status.code(), Some(0), "wallet show must exit 0");
    let show_stdout = String::from_utf8_lossy(&show_out.stdout).into_owned();
    let parsed: Value = serde_json::from_str(&show_stdout).unwrap_or_else(|e| {
        panic!("wallet show --json must parse as JSON: {e}; raw: {show_stdout}")
    });
    let show_addr = parsed
        .get("address")
        .and_then(|a| a.as_str())
        .unwrap_or_else(|| {
            panic!("wallet show --json missing 'address' field; got: {show_stdout}")
        });
    assert_eq!(
        imported_addr.to_lowercase(),
        show_addr.to_lowercase(),
        "imported address must match wallet show --json address (case 17 cross-check)"
    );
}
