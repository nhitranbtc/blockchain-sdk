//! Shared helpers for CLI-driven spike tests.
//!
//! All Vn tests spawn the `tron` CLI binary via assert_cmd and assert on
//! stdout/stderr/exit-code. The CLI surface lives in
//! `rust-wallet-app/crates/tron/src/cli.rs` (plan §Phase 6).
//!
//! Per plan §Phase 7 (REVISED 2026-09-07), the spike is **tests-only** — there
//! is no `spikes/tron-v1/src/` library. Every assertion exercises the shipped
//! CLI, so drift between spike impl and shipped impl surfaces as a test failure.
//!
//! Conventions inherited from the prior `#[ignore]`-gated live tests:
//! - Loud-RED panic when live env-var is missing (never silent `return`).
//! - `--json` flag (when present) yields machine-parseable output.
//! - Diagnostics on STDERR; data on STDOUT (plan §Task 5.8).
//!
//! **Per-test isolation** via thread-local `TEST_DATA_DIR`: each test runs
//! on its own OS thread under cargo's default harness, so a per-thread
//! `OnceCell` is the right scope. `set_test_data_dir` records the
//! isolation dir for this thread; `tron()` injects it as `TRON_DATA_DIR`
//! + `XDG_DATA_HOME` on every spawned subprocess, so parallel tests
//!   never stomp each other's wallet/config state.

use std::cell::OnceCell;
use std::path::PathBuf;

use assert_cmd::Command;

thread_local! {
    static TEST_DATA_DIR: OnceCell<PathBuf> = const { OnceCell::new() };
}

/// Record the test-thread's isolated data dir. Subsequent `tron()` calls
/// on this thread inject `TRON_DATA_DIR` + `XDG_DATA_HOME` pointing at
/// this path. Panics if the thread already set one — caller owns the
/// setup, and a second set is a logic bug, not a runtime condition.
#[allow(dead_code)] // shared helper — used by `cli_coverage.rs`; per-test-file builds warn
pub fn set_test_data_dir(path: PathBuf) {
    TEST_DATA_DIR.with(|cell| {
        cell.set(path)
            .expect("set_test_data_dir called twice on the same test thread");
    });
}

/// Wrapper around `assert_cmd::Command::cargo_bin("tron")`. Returns an
/// `assert_cmd::Command` so the `.assert()` method (from `OutputAssertExt`)
/// is in scope on call sites. If the calling thread previously called
/// `set_test_data_dir`, this command carries `TRON_DATA_DIR` +
/// `XDG_DATA_HOME` env vars on the spawned subprocess — the shipped
/// `tron` CLI reads `TRON_DATA_DIR` via clap (`env = "TRON_DATA_DIR"`),
/// and `directories::ProjectDirs` falls back to `XDG_DATA_HOME`.
#[allow(dead_code)] // shared helper — used by every gated test; per-test-file builds warn otherwise
pub fn tron() -> Command {
    let bin = assert_cmd::cargo::cargo_bin("tron");
    let mut cmd = Command::new(bin);
    TEST_DATA_DIR.with(|cell| {
        if let Some(path) = cell.get() {
            cmd.env("TRON_DATA_DIR", path);
            cmd.env("XDG_DATA_HOME", path);
        }
    });
    cmd
}

/// Assert that an env-var gate is set; panic loudly with the missing-var list
/// when absent (L29 + Phase 7 convention; never silent skip).
///
/// Used by gated live tests: V5 (Nile energy), V6 (Nile chain-id), V7-live
/// (SPKI pin verify), V9-live (USDT decimals), V11 (mainnet self-send).
#[allow(dead_code)] // shared helper — used by gated rows across trc20_local, trc20_nile, use_case_alpha, v6_nile, v9_token_registry; per-test-file builds warn otherwise
pub fn require_env(vars: &[&str]) {
    let missing: Vec<&str> = vars
        .iter()
        .copied()
        .filter(|v| std::env::var(v).is_err())
        .collect();
    assert!(
        missing.is_empty(),
        "GATED TEST: missing env vars: {missing:?}. Set them and re-run.",
    );
}

/// Load the bundled `crates/tron-wallet-core/tokens/nile.json` fixture. Same
/// resolution as `trc20_nile.rs::load_nile_tokens` — duplicated here so each
/// test file stays self-contained for cargo's per-test-file build graph.
#[allow(dead_code)]
pub fn load_nile_fixture() -> serde_json::Value {
    let candidates = [
        "crates/tron-wallet-core/tokens/nile.json",
        "../crates/tron-wallet-core/tokens/nile.json",
        "../../crates/tron-wallet-core/tokens/nile.json",
    ];
    for path in candidates {
        if let Ok(s) = std::fs::read_to_string(path) {
            return serde_json::from_str(&s).expect("nile.json must be valid JSON");
        }
    }
    panic!(
        "GATED: cannot locate crates/tron-wallet-core/tokens/nile.json from cwd={}",
        std::env::current_dir().unwrap().display()
    );
}

/// Resolve Nile sender mnemonic. Env `TRON_NILE_MNEMONIC` first, else fixture.
/// Mirrors the core mirror test at `crates/tron-wallet-core/tests/trc20_nile.rs`
/// which uses the bundled `test.sender-tr20.mnemonic` directly.
#[allow(dead_code)]
pub fn nile_sender_mnemonic() -> String {
    if let Ok(m) = std::env::var("TRON_NILE_MNEMONIC") {
        return m;
    }
    load_nile_fixture()
        .pointer("/test/sender-tr20/mnemonic")
        .and_then(|v| v.as_str())
        .expect("tokens/nile.json must have test.sender-tr20.mnemonic")
        .to_string()
}

/// Resolve Nile recipient address. Env `TRON_NILE_RECIPIENT_ADDRESS` first,
/// else fixture.
#[allow(dead_code)]
pub fn nile_recipient_address() -> String {
    if let Ok(a) = std::env::var("TRON_NILE_RECIPIENT_ADDRESS") {
        return a;
    }
    load_nile_fixture()
        .pointer("/test/recipient-tr20/address")
        .and_then(|v| v.as_str())
        .expect("tokens/nile.json must have test.recipient-tr20.address")
        .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Typed registry loaders — companion to the raw `load_nile_fixture` helper
// above. These use `include_str!` + `serde::Deserialize` + `OnceLock` so the
// JSON is embedded at compile time and parsed exactly once. Test files
// include `mod common;` and call `common::nile_usdt()`, `common::nile_recipient()`,
// etc., instead of carrying their own copies of the struct definitions.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct NileToken {
    #[allow(dead_code)]
    symbol: String,
    address: String,
    #[allow(dead_code)]
    decimals: u8,
}

#[derive(serde::Deserialize)]
struct NileTest {
    owner_address: String,
    #[allow(dead_code)]
    recipient_address: String,
    approval_spender_address: String,
    #[allow(dead_code)]
    spki_pin_hex: String,
}

#[derive(serde::Deserialize)]
struct NileConfig {
    tokens: Vec<NileToken>,
    test: NileTest,
}

fn nile_config() -> &'static NileConfig {
    use std::sync::OnceLock;
    static CFG: OnceLock<NileConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../crates/tron-wallet-core/tokens/nile.json"
        ))
        .expect("crates/tron-wallet-core/tokens/nile.json must parse")
    })
}

/// USDT contract address for the Nile testnet, from `tokens[0].address`.
#[allow(dead_code)]
pub fn nile_usdt() -> &'static str {
    &nile_config().tokens[0].address
}

/// Recipient T-address from the Nile test fixtures.
#[allow(dead_code)]
pub fn nile_recipient() -> &'static str {
    &nile_config().test.recipient_address
}

/// Owner T-address from the Nile test fixtures.
#[allow(dead_code)]
pub fn nile_owner() -> &'static str {
    &nile_config().test.owner_address
}

/// Approval spender T-address from the Nile test fixtures.
#[allow(dead_code)]
pub fn nile_spender() -> &'static str {
    &nile_config().test.approval_spender_address
}

// ─── SPKI pin derivation (live TLS handshake + fixture cross-check) ─────────
//
// Compute the SPKI pin for `host:port` from a fresh TLS handshake. We shell
// to `openssl s_client` because adding `rustls` as a spike dev-dep just to
// read one cert is overkill, and `openssl` is available on every CI image
// the spike runs on. The pin format matches the library's
// `SpkiPinnedVerifier::leaf_spki_digest`: SHA-256 of the full
// SubjectPublicKeyInfo DER blob, lowercase hex (RFC 7469).
//
// If the returned pin does NOT match the bundled fixture's `spki_pin_hex`,
// TronGrid has rotated its leaf cert — the caller fails LOUDLY so the
// operator knows to refresh the fixture (rather than silently passing
// against a stale pin baked into the binary).

use std::io::Write;
use std::process::{Command as StdCommand, Stdio};

use sha2::{Digest, Sha256};
use x509_parser::prelude::FromDer;

/// Derive the SPKI pin for a given `host:port` from a live TLS handshake.
/// Returns lowercase hex (64 chars), or panics if the handshake fails.
#[allow(dead_code)] // shared helper — used by trc20_nile; per-test-file builds warn
pub fn live_spki_pin(host: &str, port: u16) -> String {
    // 1. Pull the leaf cert in DER via openssl s_client. `</dev/null`
    //    closes stdin so openssl exits after the handshake instead of
    //    waiting for input. `2>/dev/null` discards the s_client status
    //    chatter — only stdout (the cert blob) matters.
    let mut s_client = StdCommand::new("openssl")
        .args([
            "s_client",
            "-connect",
            &format!("{host}:{port}"),
            "-servername",
            host,
            "-showcerts",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("openssl s_client must be on PATH for live SPKI derivation");

    // 2. Pipe the PEM output through `openssl x509 -outform DER` to get the
    //    binary DER blob. openssl prints the FIRST cert from the chain
    //    (the leaf) by default.
    let x509 = StdCommand::new("openssl")
        .args(["x509", "-outform", "DER"])
        .stdin(s_client.stdout.expect("piped stdout"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("openssl x509 must be on PATH for live SPKI derivation");
    let output = x509.wait_with_output().expect("openssl x509 must complete");
    // Reap the s_client child now that x509 has consumed its stdout pipe to EOF.
    let _ = s_client.wait();

    if !output.status.success() {
        let _ = writeln!(
            std::io::stderr(),
            "openssl x509 stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!(
            "openssl x509 failed for {host}:{port} (exit={:?}); cannot derive SPKI pin",
            output.status.code()
        );
    }
    let der = output.stdout;
    assert!(
        !der.is_empty(),
        "openssl returned empty DER for {host}:{port}"
    );

    // 3. Parse the X.509 cert and hash the full SubjectPublicKeyInfo DER
    //    (algorithm identifier + public-key BIT STRING) — same byte range
    //    `tbs_certificate.public_key().raw` exposes per the library.
    let (_, cert) = x509_parser::certificate::X509Certificate::from_der(&der)
        .expect("openssl DER must parse as X.509");
    let spki_der = cert.tbs_certificate.public_key().raw;
    let digest: [u8; 32] = Sha256::digest(spki_der).into();
    hex::encode(digest)
}

/// Resolve the expected SPKI pin from the bundled `tokens/nile.json` fixture.
/// Returns `None` if the field is absent (older fixture) — callers should
/// then either skip the cross-check or fail-closed.
#[allow(dead_code)]
pub fn fixture_spki_pin() -> Option<String> {
    let raw = &nile_config().test.spki_pin_hex;
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_ascii_lowercase())
    }
}

/// Derive the live SPKI pin for the default Nile RPC, then assert it matches
/// the bundled fixture. Returns the pin on success, panics loudly on cert
/// rotation. This is what gated live tests use to surface cert rotation as
/// a fail-loud instead of a silent skip.
#[allow(dead_code)]
pub fn assert_live_spki_pin() -> String {
    let host = nile_rpc_host();
    // Port follows the URL scheme: https → 443, http → 80. Nile ships
    // over TLS so this is 443 today; if a future fixture flips to
    // `http://` for a local TronBox, the port follows automatically.
    let port: u16 = if nile_rpc_url().starts_with("https://") {
        443
    } else {
        80
    };
    let live = live_spki_pin(host, port);
    if let Some(expected) = fixture_spki_pin() {
        assert_eq!(
            live, expected,
            "SPKI pin drift: live handshake for {host}:{port} yields {live}, \
             but fixture pins {expected}. TronGrid rotated its leaf cert — \
             refresh crates/tron-wallet-core/tokens/nile.json \
             `test.spki_pin_hex` (or unset `TRON_NILE_SPKI_PIN_OVERRIDE`)."
        );
    } else {
        eprintln!(
            "[spki] WARNING: fixture missing `test.spki_pin_hex`; \
             using live pin {live} without cross-check"
        );
    }
    live
}

// ─── network.json: per-network RPC URLs ──────────────────────────────────────

#[derive(serde::Deserialize)]
struct NetworkConfig {
    #[allow(dead_code)]
    mainnet: NetworkEntry,
    #[allow(dead_code)]
    shasta: NetworkEntry,
    nile: NetworkEntry,
    #[allow(dead_code)]
    local: NetworkEntry,
}

#[derive(serde::Deserialize)]
struct NetworkEntry {
    #[allow(dead_code)]
    rpc_url: String,
}

fn network_config() -> &'static NetworkConfig {
    use std::sync::OnceLock;
    static CFG: OnceLock<NetworkConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../crates/tron-wallet-core/tokens/network.json"
        ))
        .expect("crates/tron-wallet-core/tokens/network.json must parse")
    })
}

/// RPC base URL for the Nile testnet, sourced from `network.json`.
#[allow(dead_code)]
pub fn nile_rpc_url() -> &'static str {
    &network_config().nile.rpc_url
}

/// Host portion of the Nile RPC URL (e.g. `nile.trongrid.io`), derived from
/// `network.json` so the SPKI pin flow + `pinned://` URL builders never
/// duplicate the host literal. Strips `https://`/`http://` scheme and any
/// trailing path. Port is left out — `assert_live_spki_pin` derives it
/// from the URL scheme (https → 443, http → 80).
#[allow(dead_code)]
pub fn nile_rpc_host() -> &'static str {
    use std::sync::OnceLock;
    static HOST: OnceLock<String> = OnceLock::new();
    HOST.get_or_init(|| {
        let url = nile_rpc_url();
        let after_scheme = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);
        after_scheme
            .split('/')
            .next()
            .unwrap_or(after_scheme)
            .to_string()
    })
}

/// Build a `pinned://<pin>@<host>` URL for the Nile testnet. Pin is supplied
/// by the caller (live-derived or test-supplied via `TRON_NILE_SPKI_PIN`).
/// Host comes from `network.json` via `nile_rpc_host()` — single source of
/// truth, so a network rename touches one file.
#[allow(dead_code)]
pub fn nile_pinned_url(pin: &str) -> String {
    format!("pinned://{pin}@{}", nile_rpc_host())
}

// ─── Shared test configuration and canonical fixtures ─────────────────────────

#[allow(dead_code)] // shared helpers — per-test-file builds warn otherwise
pub const RUN_TRON_NILE: &str = "RUN_TRON_NILE";
#[allow(dead_code)]
pub const RUN_TRON_LOCAL: &str = "RUN_TRON_LOCAL";
#[allow(dead_code)]
pub const RUN_TRON_MAINNET: &str = "RUN_TRON_MAINNET";
#[allow(dead_code)]
pub const TRON_NILE_PRIVATE_KEY: &str = "TRON_NILE_PRIVATE_KEY";
#[allow(dead_code)]
pub const TRON_MAINNET_OPERATOR_WALLET: &str = "TRON_MAINNET_OPERATOR_WALLET";

#[allow(dead_code)]
pub fn nile_network() -> &'static str {
    "nile"
}

#[allow(dead_code)]
pub fn mainnet_network() -> &'static str {
    "mainnet"
}

#[allow(dead_code)]
pub fn shasta_network() -> &'static str {
    "shasta"
}

#[allow(dead_code)]
pub fn local_network() -> &'static str {
    "local"
}

#[allow(dead_code)]
pub fn mainnet_rpc_url() -> &'static str {
    &network_config().mainnet.rpc_url
}

#[allow(dead_code)]
pub fn shasta_rpc_url() -> &'static str {
    &network_config().shasta.rpc_url
}

#[allow(dead_code)]
pub fn local_rpc_url() -> &'static str {
    &network_config().local.rpc_url
}

#[allow(dead_code)]
pub fn tronbox_rpc_url() -> &'static str {
    "http://127.0.0.1:9090"
}

#[allow(dead_code)]
pub fn closed_port_rpc_url() -> &'static str {
    "http://127.0.0.1:9999"
}

#[allow(dead_code)]
pub fn mainnet_owner() -> &'static str {
    &mainnet_config().test.owner_address
}

#[derive(serde::Deserialize)]
struct MainnetConfig {
    test: MainnetTest,
}

#[derive(serde::Deserialize)]
struct MainnetTest {
    owner_address: String,
}

#[allow(dead_code)]
fn mainnet_config() -> &'static MainnetConfig {
    use std::sync::OnceLock;
    static CFG: OnceLock<MainnetConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../crates/tron-wallet-core/tokens/mainnet.json"
        ))
        .expect("crates/tron-wallet-core/tokens/mainnet.json must parse")
    })
}

#[allow(dead_code)]
pub fn canonical_mnemonic() -> &'static str {
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
}

#[allow(dead_code)]
pub fn test_password() -> &'static str {
    "test-pw"
}

#[allow(dead_code)]
pub fn v10_password() -> &'static str {
    "v10-kat-pass"
}

#[allow(dead_code)]
pub fn default_fee_limit_sun() -> &'static str {
    "130000000"
}

#[allow(dead_code)]
pub fn speedup_fee_limit_sun() -> &'static str {
    "260000000"
}

#[allow(dead_code)]
pub fn tx_wait_timeout_secs() -> &'static str {
    "120"
}

#[allow(dead_code)]
pub fn tx_wait_short_timeout_secs() -> &'static str {
    "5s"
}

#[allow(dead_code)]
pub fn poll_interval_secs() -> &'static str {
    "1s"
}

#[allow(dead_code)]
pub fn transport_error_budget_secs() -> u64 {
    30
}

#[allow(dead_code)]
pub fn one_usdt_display_amount() -> &'static str {
    "1"
}

#[allow(dead_code)]
pub fn one_usdt_raw_amount() -> &'static str {
    "1000000"
}

#[allow(dead_code)]
pub fn usdt_decimals() -> u64 {
    6
}

#[allow(dead_code)]
pub fn nile_recipient_t_addr() -> &'static str {
    "TG7jQ7eGsns6nmQNfcKNgZKyKBFkx7CvXr"
}

#[allow(dead_code)]
pub fn secp256k1_generator_pubkey_hex() -> &'static str {
    "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\
     483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8"
}

#[allow(dead_code)]
pub fn bs58_alphabet() -> &'static str {
    "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
}

#[allow(dead_code)]
pub fn u256_max_decimal() -> &'static str {
    "115792089237316195423570985008687907853269984665640564039457584007913129639935"
}

#[allow(dead_code)]
pub fn tron_slip44_path() -> &'static str {
    "m/44'/195'/0'/0/0"
}

#[allow(dead_code)]
pub fn bitcoin_slip44_path() -> &'static str {
    "m/44'/0'/0'/0/0"
}

#[allow(dead_code)]
pub fn tron_xpub_path() -> &'static str {
    "m/44'/195'/0'"
}

#[allow(dead_code)]
pub fn trc20_calldata_len() -> usize {
    68
}

#[allow(dead_code)]
pub fn transfer_selector() -> &'static [u8; 4] {
    &[0xa9, 0x05, 0x9c, 0xbb]
}

#[allow(dead_code)]
pub fn approve_selector() -> &'static [u8; 4] {
    &[0x09, 0x5e, 0xa7, 0xb3]
}

#[allow(dead_code)]
pub fn balance_of_selector() -> &'static [u8; 4] {
    &[0x70, 0xa0, 0x82, 0x31]
}

#[allow(dead_code)]
pub fn wrong_spki_pin() -> &'static str {
    "0000000000000000000000000000000000000000000000000000000000000000"
}

#[allow(dead_code)]
pub fn unknown_txid() -> &'static str {
    "0000000000000000000000000000000000000000000000000000000000000001"
}

#[allow(dead_code)]
pub fn unconfirmed_txid() -> &'static str {
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
}

#[allow(dead_code)]
pub fn offline_raw_transaction() -> &'static str {
    r#"{
    "ref_block_bytes": "abcd",
    "ref_block_hash": "0123456789abcdef0123456789abcdef01234567",
    "timestamp": 1700000000000,
    "fee_limit": 130000000,
    "expiration": 1700000600000,
    "contract": [{
        "type": "TransferContract",
        "parameter": {
            "type_url": "type.googleapis.com/protocol.TransferContract",
            "value": {
                "amount": 1000000,
                "owner_address": "0x4100000000000000000000000000000000000000",
                "to_address": "0x4111111111111111111111111111111111111111"
            }
        }
    }]
}"#
}
