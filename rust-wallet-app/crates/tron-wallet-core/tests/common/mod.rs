//! Shared helpers + constants for `tron-wallet-core` integration tests
//! AND the spike tests (`spikes/tron-v1/tests/*.rs`).
//!
//! **Single source of truth** for both crates:
//! - Core tests (`crates/tron-wallet-core/tests/*.rs`) `mod common;` and
//!   exercise `tron-wallet-core` directly via the library API.
//! - Spike tests (`spikes/tron-v1/tests/*.rs`) `#[path =
//!   "../../../../crates/tron-wallet-core/tests/common/mod.rs"] mod common;`
//!   and exercise the shipped `tron` CLI binary via `assert_cmd`.
//!
//! CLI-only helpers (`tron`, `set_test_data_dir`, `live_spki_pin`) use
//! `assert_cmd` + the `tron` binary — both declared as core dev-deps so
//! the common module compiles in either crate's test binary. Core tests
//! ignore the CLI helpers (dead-code allowed).
//!
//! Conventions:
//! - `#[allow(dead_code)]` on every helper — per-test-file builds warn
//!   for helpers the calling test does not reference.
//! - `OnceLock<T>` for expensive loaders (test_addresses, nile.json
//!   fixture, network.json) — JSON embedded via `include_str!` + parsed
//!   exactly once.
//! - Loud-RED panic when env-gate missing (L29 + Plan §Conventions).

#![allow(dead_code, unused_imports)]

use std::cell::OnceCell;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command as StdCommand, Stdio};
use std::sync::OnceLock;

use sha2::{Digest, Sha256};
use x509_parser::prelude::FromDer;

use tron_wallet_core::config::Network;
use tron_wallet_core::keys::{Language, Mnemonic};

// ─────────────────────────────────────────────────────────────────────────────
// CLI test plumbing (spike-only; core tests ignore via #![allow(dead_code)]).
// ─────────────────────────────────────────────────────────────────────────────

thread_local! {
    static TEST_DATA_DIR: OnceCell<PathBuf> = const { OnceCell::new() };
}

/// Record the test-thread's isolated data dir. Subsequent `tron()` calls
/// on this thread inject `TRON_DATA_DIR` + `XDG_DATA_HOME` pointing at
/// this path. Panics if the thread already set one — caller owns the
/// setup, and a second set is a logic bug, not a runtime condition.
pub fn set_test_data_dir(path: PathBuf) {
    TEST_DATA_DIR.with(|cell| {
        cell.set(path)
            .expect("set_test_data_dir called twice on the same test thread");
    });
}

/// Wrapper around `assert_cmd::Command::cargo_bin("tron")`. If the
/// calling thread previously called `set_test_data_dir`, this command
/// carries `TRON_DATA_DIR` + `XDG_DATA_HOME` env vars on the spawned
/// subprocess — the shipped `tron` CLI reads `TRON_DATA_DIR` via clap
/// (`env = "TRON_DATA_DIR"`), and `directories::ProjectDirs` falls back
/// to `XDG_DATA_HOME`.
pub fn tron() -> assert_cmd::Command {
    let bin = assert_cmd::cargo::cargo_bin("tron");
    let mut cmd = assert_cmd::Command::new(bin);
    TEST_DATA_DIR.with(|cell| {
        if let Some(path) = cell.get() {
            cmd.env("TRON_DATA_DIR", path);
            cmd.env("XDG_DATA_HOME", path);
        }
    });
    cmd
}

/// Assert that an env-var gate is set; panic loudly with the missing-var
/// list when absent (L29 + Phase 7 convention; never silent skip).
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

// ─────────────────────────────────────────────────────────────────────────────
// SPKI pin derivation (live TLS handshake).
// ─────────────────────────────────────────────────────────────────────────────

/// Derive the SPKI pin for a given `host:port` from a live TLS handshake.
/// Returns lowercase hex (64 chars). Panics on handshake failure.
pub fn live_spki_pin(host: &str, port: u16) -> String {
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

    let x509 = StdCommand::new("openssl")
        .args(["x509", "-outform", "DER"])
        .stdin(s_client.stdout.take().expect("piped stdout"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("openssl x509 must be on PATH for live SPKI derivation");
    let output = x509.wait_with_output().expect("openssl x509 must complete");
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

    let (_, cert) = x509_parser::certificate::X509Certificate::from_der(&der)
        .expect("openssl DER must parse as X.509");
    let spki_der = cert.tbs_certificate.public_key().raw;
    let digest: [u8; 32] = Sha256::digest(spki_der).into();
    hex::encode(digest)
}

/// Resolve the expected SPKI pin from the bundled `tokens/nile.json` fixture.
/// SPKI pin source — derived from a live TLS handshake (no JSON fixture).
/// Equivalent to `assert_live_spki_pin()` but without the fixture cross-check
/// (the fixture no longer carries `spki_pin_hex`).
pub fn fixture_spki_pin() -> String {
    assert_live_spki_pin()
}

/// Derive the SPKI pin for the active Nile RPC endpoint from a live TLS
/// handshake. Returns lowercase hex (64 chars). Panics if the handshake
/// fails.
pub fn assert_live_spki_pin() -> String {
    let host = nile_rpc_host();
    let port: u16 = if nile_rpc_url().starts_with("https://") {
        443
    } else {
        80
    };
    live_spki_pin(host, port)
}

// ─────────────────────────────────────────────────────────────────────────────
// Canonical BIP-39 fixture phrase (all-zero entropy) + derivation paths
// ─────────────────────────────────────────────────────────────────────────────

/// BIP-39 vector mnemonic for all-zero entropy.
pub const CANONICAL_PHRASE: &str = "abandon abandon abandon abandon abandon abandon \
     abandon abandon abandon abandon abandon about";

/// All-zero mnemonic — used as `PHRASE` alias in wallet persistence tests.
pub const CANONICAL_MNEMONIC: &str = CANONICAL_PHRASE;

/// SLIP-44 TRON (coin 195) derivation path.
pub const TRON_PATH: &str = "m/44'/195'/0'/0/0";

/// Spike alias for `TRON_PATH` (CLI flag form).
pub const TRON_SLIP44_PATH: &str = TRON_PATH;

/// SLIP-44 TRON account-level path for SLIP-0132 xpub export.
pub const TRON_XPUB_PATH: &str = "m/44'/195'/0'";

/// SLIP-44 Bitcoin (coin 0) — for cross-chain tests.
pub const BITCOIN_SLIP44_PATH: &str = "m/44'/0'/0'/0/0";

/// SLIP-44 Ethereum (coin 60) derivation path.
pub const ETHEREUM_PATH: &str = "m/44'/60'/0'/0/0";

#[allow(dead_code)]
pub fn canonical_mnemonic() -> Mnemonic {
    Mnemonic::from_phrase(CANONICAL_PHRASE, Language::English)
        .expect("canonical BIP-39 phrase must parse")
}

// ─────────────────────────────────────────────────────────────────────────────
// Bundled token-registry accessors — single source of truth across tests
// ─────────────────────────────────────────────────────────────────────────────

/// `&'static TestAddresses` for the bundled mainnet fixtures.
pub fn mainnet_test_addresses() -> &'static tron_wallet_core::tokens::TestAddresses {
    static CELL: OnceLock<&'static tron_wallet_core::tokens::TestAddresses> = OnceLock::new();
    CELL.get_or_init(|| {
        tron_wallet_core::tokens::test_addresses(Network::Mainnet)
            .expect("mainnet test fixtures must be present")
    })
}

/// `&'static TestAddresses` for the bundled Nile fixtures.
pub fn nile_test_addresses() -> &'static tron_wallet_core::tokens::TestAddresses {
    static CELL: OnceLock<&'static tron_wallet_core::tokens::TestAddresses> = OnceLock::new();
    CELL.get_or_init(|| {
        tron_wallet_core::tokens::test_addresses(Network::Nile)
            .expect("Nile test fixtures must be present")
    })
}

/// Bundled mainnet USDT-TRC20 contract address (`tokens/mainnet.json`).
pub fn mainnet_usdt_address() -> &'static str {
    static CELL: OnceLock<&'static tron_wallet_core::tokens::Token> = OnceLock::new();
    CELL.get_or_init(|| {
        tron_wallet_core::tokens::by_symbol(Network::Mainnet, "USDT")
            .expect("mainnet USDT must be in the bundle")
    })
    .address
    .as_str()
}

/// Bundled Nile USDT-TRC20 contract address (`tokens/nile.json`).
pub fn nile_usdt_address() -> &'static str {
    static CELL: OnceLock<&'static tron_wallet_core::tokens::Token> = OnceLock::new();
    CELL.get_or_init(|| {
        tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")
            .expect("Nile USDT must be in the bundle")
    })
    .address
    .as_str()
}

// ─────────────────────────────────────────────────────────────────────────────
// Network config (RPC URLs from bundled `tokens/network.json`)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct NetworkConfig {
    mainnet: NetworkEntry,
    #[allow(dead_code)]
    shasta: NetworkEntry,
    nile: NetworkEntry,
    #[allow(dead_code)]
    local: NetworkEntry,
}

#[derive(serde::Deserialize)]
struct NetworkEntry {
    rpc_url: String,
}

fn network_config() -> &'static NetworkConfig {
    static CFG: OnceLock<NetworkConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        serde_json::from_str(include_str!("../../tokens/network.json"))
            .expect("crates/tron-wallet-core/tokens/network.json must parse")
    })
}

pub fn nile_rpc_url() -> &'static str {
    &network_config().nile.rpc_url
}

pub fn mainnet_rpc_url() -> &'static str {
    &network_config().mainnet.rpc_url
}

pub fn shasta_rpc_url() -> &'static str {
    &network_config().shasta.rpc_url
}

pub fn local_rpc_url() -> &'static str {
    &network_config().local.rpc_url
}

/// Host portion of the Nile RPC URL (e.g. `nile.trongrid.io`).
pub fn nile_rpc_host() -> &'static str {
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

/// Build `pinned://<pin>@<host>` URL for the Nile testnet.
pub fn nile_pinned_url(pin: &str) -> String {
    format!("pinned://{pin}@{}", nile_rpc_host())
}

// ─────────────────────────────────────────────────────────────────────────────
// Bundled Nile test fixture (mnemonic + address pairs for sender / recipient)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct TestWallet {
    mnemonic: String,
    address: String,
}

#[derive(serde::Deserialize)]
struct NileFixtureTest {
    #[serde(rename = "sender-tr20")]
    sender_tr20: TestWallet,
    #[serde(rename = "recipient-tr20")]
    recipient_tr20: TestWallet,
    /// Optional SPKI pin (lowercase hex, 32 bytes decoded) for
    /// `nile.trongrid.io`'s TLS leaf certificate. Phase 3 §3.7 — kept
    /// optional because the canonical pin is now derived at runtime via
    /// `get_spki()` (live TLS handshake) or supplied via `TRON_NILE_SPKI_PIN`
    /// env var. Bundled fixtures intentionally omit it so a TronGrid cert
    /// rotation does not require a JSON edit.
    #[serde(rename = "spki_pin_hex", default)]
    #[allow(dead_code)]
    spki_pin_hex: Option<String>,
}

#[derive(serde::Deserialize)]
struct NileFixture {
    test: NileFixtureTest,
}

fn nile_fixture() -> &'static NileFixture {
    static F: OnceLock<NileFixture> = OnceLock::new();
    F.get_or_init(|| {
        serde_json::from_str(include_str!("../../tokens/nile.json"))
            .expect("crates/tron-wallet-core/tokens/nile.json must parse as NileFixture")
    })
}

/// Bundled Nile sender mnemonic (random 12-word BIP-39, no real value).
/// Source: `tokens/nile.json::test.sender-tr20.mnemonic`.
pub fn nile_sender_mnemonic() -> &'static str {
    &nile_fixture().test.sender_tr20.mnemonic
}

/// Bundled Nile sender T-address.
pub fn nile_sender_address() -> &'static str {
    &nile_fixture().test.sender_tr20.address
}

/// Bundled Nile recipient T-address.
pub fn nile_recipient_address() -> &'static str {
    &nile_fixture().test.recipient_tr20.address
}

/// Bundled Nile recipient mnemonic.
pub fn nile_recipient_mnemonic() -> &'static str {
    &nile_fixture().test.recipient_tr20.mnemonic
}

/// Bundled Nile SPKI pin (lowercase hex, 32 bytes). Env-var override
/// `TRON_NILE_SPKI_PIN` wins; otherwise live-derived via `get_spki()`.
pub fn nile_spki_pin() -> [u8; 32] {
    if let Ok(hex_str) = std::env::var("TRON_NILE_SPKI_PIN") {
        return hex_to_pin(&hex_str);
    }
    if let Some(pin_hex) = &nile_fixture().test.spki_pin_hex {
        return hex_to_pin(pin_hex);
    }
    get_spki()
}

/// Hex-string → 32-byte pin helper.
fn hex_to_pin(hex_str: &str) -> [u8; 32] {
    let bytes = hex::decode(hex_str).expect("SPKI pin must be valid hex");
    assert_eq!(
        bytes.len(),
        32,
        "SPKI pin must be 32 bytes (64 hex chars); got {}",
        bytes.len()
    );
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

/// Resolve the SPKI pin for the active Nile RPC endpoint from a live TLS
/// handshake (preferred) or `TRON_NILE_SPKI_PIN` env-var override. Returns
/// lowercase-hex-decoded 32-byte pin. This is the canonical helper for
/// test functions that need the SPKI pin — `nile_spki_pin()` delegates
/// here.
pub fn get_spki() -> [u8; 32] {
    if let Ok(hex_str) = std::env::var("TRON_NILE_SPKI_PIN") {
        return hex_to_pin(&hex_str);
    }
    hex_to_pin(&assert_live_spki_pin())
}

// ─────────────────────────────────────────────────────────────────────────────
// Typed registry loaders (companion to `nile_test_addresses` / `nile_usdt_address`)
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
}

#[derive(serde::Deserialize)]
struct NileConfig {
    tokens: Vec<NileToken>,
    test: NileTest,
}

fn nile_config() -> &'static NileConfig {
    static CFG: OnceLock<NileConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        serde_json::from_str(include_str!("../../tokens/nile.json"))
            .expect("crates/tron-wallet-core/tokens/nile.json must parse")
    })
}

/// USDT contract address for the Nile testnet, from `tokens[0].address`.
pub fn nile_usdt() -> &'static str {
    &nile_config().tokens[0].address
}

/// Recipient T-address from the Nile test fixtures.
pub fn nile_recipient() -> &'static str {
    &nile_config().test.recipient_address
}

/// Owner T-address from the Nile test fixtures.
pub fn nile_owner() -> &'static str {
    &nile_config().test.owner_address
}

/// Approval spender T-address from the Nile test fixtures.
pub fn nile_spender() -> &'static str {
    &nile_config().test.approval_spender_address
}

// ─────────────────────────────────────────────────────────────────────────────
// Mainnet config (typed loader)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct MainnetConfig {
    test: MainnetTest,
}

#[derive(serde::Deserialize)]
struct MainnetTest {
    owner_address: String,
}

fn mainnet_config() -> &'static MainnetConfig {
    static CFG: OnceLock<MainnetConfig> = OnceLock::new();
    CFG.get_or_init(|| {
        serde_json::from_str(include_str!("../../tokens/mainnet.json"))
            .expect("crates/tron-wallet-core/tokens/mainnet.json must parse")
    })
}

/// Owner T-address from the mainnet test fixtures.
pub fn mainnet_owner() -> &'static str {
    &mainnet_config().test.owner_address
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants used across multiple test files
// ─────────────────────────────────────────────────────────────────────────────

/// 1 USDT-TRC20 in 6-decimal base units (1 × 10^6).
pub const ONE_USDT_RAW: u64 = 1_000_000;

/// 1 TRX in SUN (1 × 10^6).
pub const ONE_TRX_SUN: u64 = 1_000_000;

/// 1 USDT as a display string `"1"`.
pub const ONE_USDT_DISPLAY_AMOUNT: &str = "1";

/// 1 USDT as raw SUN/base-units string `"1000000"`.
pub const ONE_USDT_RAW_AMOUNT: &str = "1000000";

/// Default fee_limit per Plan §Task 5.x — 130 TRX in SUN.
pub const DEFAULT_FEE_LIMIT_SUN: &str = "130000000";

/// Speedup fee_limit per Plan §Task 5.x — 260 TRX in SUN (2× baseline).
pub const SPEEDUP_FEE_LIMIT_SUN: &str = "260000000";

/// USDT-TRC20 decimals per TronGrid published values.
pub const USDT_DECIMALS: u64 = 6;

/// TRC-20 `transfer(address,uint256)` calldata length: 4 + 32 + 32.
pub const TRC20_CALLDATA_LEN: usize = 68;

/// TRC-20 `transfer(address,uint256)` selector (keccak256 first 4 bytes).
pub const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

/// TRC-20 `approve(address,uint256)` selector.
pub const APPROVE_SELECTOR: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];

/// TRC-20 `balanceOf(address)` selector.
pub const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

/// Confirmation-poll interval in seconds.
pub const POLL_INTERVAL_SECS: u64 = 3;

/// Confirmation-poll interval — CLI string form (`--poll-interval 3`).
pub const POLL_INTERVAL_SECS_STR: &str = "3";

/// Confirmation-poll deadline in seconds.
pub const TX_WAIT_TIMEOUT_SECS: u64 = 120;

/// Confirmation-poll deadline in seconds — CLI string form (`--timeout 120`).
pub const TX_WAIT_TIMEOUT_SECS_STR: &str = "120";

/// Short confirmation-poll timeout (`"5s"` form, used by some spike tests).
pub const TX_WAIT_SHORT_TIMEOUT_SECS: &str = "5s";

/// Hard ceiling for transport-error round-trip (row_4 in nile tests).
pub const TRANSPORT_ERROR_BUDGET_SECS: u64 = 30;

/// base58 alphabet (Bitcoin/Tron standard).
pub const BS58_ALPHABET: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// `u256::MAX` as decimal string (for insufficient-balance tests).
pub const U256_MAX_DECIMAL: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

/// Wrong SPKI pin (all zeros, 64 hex chars).
pub const WRONG_SPKI_PIN: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Sentinel txid used for "unknown transaction" tests.
pub const UNKNOWN_TXID: &str = "0000000000000000000000000000000000000000000000000000000000000001";

/// Sentinel txid used for "unconfirmed transaction" tests.
pub const UNCONFIRMED_TXID: &str =
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

/// Sample offline raw transaction (used by offline CLI tests).
pub const OFFLINE_RAW_TRANSACTION: &str = r#"{
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
}"#;

// ─────────────────────────────────────────────────────────────────────────────
// Env-gate constants + passwords
// ─────────────────────────────────────────────────────────────────────────────

pub const RUN_TRON_NILE: &str = "RUN_TRON_NILE";
pub const RUN_TRON_LOCAL: &str = "RUN_TRON_LOCAL";
pub const RUN_TRON_MAINNET: &str = "RUN_TRON_MAINNET";
pub const TRON_NILE_PRIVATE_KEY: &str = "TRON_NILE_PRIVATE_KEY";
pub const TRON_MAINNET_OPERATOR_WALLET: &str = "TRON_MAINNET_OPERATOR_WALLET";

/// Network name constants (used as CLI args).
pub const NILE_NETWORK: &str = "nile";
pub const MAINNET_NETWORK: &str = "mainnet";
pub const SHASTA_NETWORK: &str = "shasta";
pub const LOCAL_NETWORK: &str = "local";

/// Generic test password for `tron wallet create` CLI tests.
pub const TEST_PASSWORD: &str = "test-pw";

/// Stronger password for V10 wallet persistence tests.
pub const V10_PASSWORD: &str = "v10-kat-pass";

/// TronBox default RPC URL (local testnet).
pub const TRONBOX_RPC_URL: &str = "http://127.0.0.1:9090";

/// Closed-port RPC URL (used by network-failure recovery tests).
pub const CLOSED_PORT_RPC_URL: &str = "http://127.0.0.1:9999";

// ─────────────────────────────────────────────────────────────────────────────
// Keypair helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build the canonical sender keypair from the bundled Nile mnemonic.
pub fn nile_sender_keypair() -> Result<
    (
        Mnemonic,
        tron_wallet_core::keys::DerivationPath,
        zeroize::Zeroizing<[u8; 32]>,
    ),
    Box<dyn std::error::Error>,
> {
    use tron_wallet_core::keys::{derive_keypair, DerivationPath};
    let mnemonic = Mnemonic::from_phrase(nile_sender_mnemonic(), Language::English)?;
    let path: DerivationPath = TRON_PATH.parse()?;
    let keypair = derive_keypair(&mnemonic, "", &path)?;
    Ok((mnemonic, path, keypair.secret_bytes().clone()))
}
