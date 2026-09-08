//! V10 — SLIP-44 + BIP-32 path derivation (CLI-driven, Phase 7 §Task 7.11).
//!
//! Plan §V10: TRON's SLIP-44 coin type is `195`, so the canonical receive
//! path is `m/44'/195'/0'/0/0`. The shipped CLI derives this via
//! `tron address new --mnemonic <phrase> [--index N | --path <bip32>]`
//! (handler: `crates/tron/src/handlers/address.rs::new`).
//!
//! Asserted offline invariants (no in-process library calls — Phase 7):
//! - Default `--index 0` emits path `m/44'/195'/0'/0/0` (SLIP-44 195).
//! - `--index 0` and `--path m/44'/195'/0'/0/0` produce the same address.
//! - Sibling indices give different addresses (BIP-32 branch isolation).
//! - Different coin types (`m/44'/0'/0'/0/0` BTC vs `m/44'/195'/0'/0/0` TRON)
//!   give different addresses (key isolation across coin types).
//! - KAT capture: the canonical BIP-39 phrase
//!   `abandon abandon ... abandon about` (11 × "abandon" + "about") at the
//!   default path emits a pinned T-address. Downstream Vn tests
//!   (`v4_kat_parity_*`, `trc20_nile` rows) cross-check fixture addresses;
//!   if derivation silently regresses, this KAT breaks first.
//! - xpub export is deterministic per mnemonic + path.
//! - Drift detector: `tron address new --help` documents the `--path` flag.

mod common;

/// Pinned KAT: `common::CANONICAL_MNEMONIC` at `m/44'/195'/0'/0/0` MUST emit this
/// T-address. Captured from `tron address new --mnemonic ... --json` on
/// 2026-09-08; if derivation changes (e.g. seed-padding tweak, coin-type
/// flip, keccak step added) this KAT catches it before downstream tests
/// see flaky balances. The hex value is intentionally opaque here — the
/// offline base58check decoder in `v4_base58check.rs` already verified
/// the shape invariants (length 34, prefix `0x41`, valid checksum); this
/// Vn test only pins the byte sequence.
const CANONICAL_ABANDON_ABOUT_AT_DEFAULT_PATH: &str = "TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH";

/// Drive `tron address new` with the given args, return the parsed
/// `(address, path)` JSON pair. Panics on non-success exit or malformed
/// JSON — both are CLI contract violations the spike should surface loud.
fn derive_address(args: &[&str]) -> (String, String) {
    let assert = common::tron()
        .args(["address", "new", "--mnemonic", common::CANONICAL_MNEMONIC])
        .args(args)
        .args(["--json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("tron address new --json must emit JSON");
    let address = v
        .get("address")
        .and_then(|a| a.as_str())
        .expect("`--json` output must include `address` string")
        .to_string();
    let path = v
        .get("path")
        .and_then(|p| p.as_str())
        .expect("`--json` output must include `path` string")
        .to_string();
    (address, path)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. SLIP-44 coin type — default path must be `m/44'/195'/0'/0/0`.
// ─────────────────────────────────────────────────────────────────────────────

/// Default `--index 0` (the BIP-44 account-0 receive child) must emit the
/// path `m/44'/195'/0'/0/0` — coin type `195` is TRON's SLIP-44
/// registration. A CLI change that flipped the coin type (e.g. to `0`
/// / Bitcoin) would break mainnet interop — wallets would still sign
/// correctly but every T-address would shift and downstream tools
/// (TronGrid, exchanges) would fail to recognize the wallet.
#[test]
fn v10_default_index_uses_tron_slip44_path() {
    let (addr, path) = derive_address(&["--index", "0"]);
    assert_eq!(
        path,
        common::TRON_SLIP44_PATH,
        "default --index 0 must use SLIP-44 coin type 195; got {path:?}"
    );
    assert!(
        addr.starts_with('T'),
        "default-path address must be a T-address; got {addr:?}"
    );
    assert_eq!(addr.len(), 34);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. `--index 0` and explicit `--path m/44'/195'/0'/0/0` are equivalent.
// ─────────────────────────────────────────────────────────────────────────────

/// Per `crates/tron/src/cli.rs::AddressAction::New`: "BIP-44 address index;
/// ignored when `--path` is given." So `--index 0` and `--path
/// m/44'/195'/0'/0/0` MUST yield byte-equal addresses (same derivation
/// step). Catches a regression where `--index` and `--path` take
/// divergent code paths through `DerivationPath::parse`.
#[test]
fn v10_index_zero_and_explicit_default_path_match() {
    let (index_addr, index_path) = derive_address(&["--index", "0"]);
    let (path_addr, path_path) = derive_address(&["--path", common::TRON_SLIP44_PATH]);
    assert_eq!(index_path, common::TRON_SLIP44_PATH);
    assert_eq!(path_path, common::TRON_SLIP44_PATH);
    assert_eq!(
        index_addr, path_addr,
        "--index 0 and --path m/44'/195'/0'/0/0 must derive the same address; \
         got {index_addr:?} vs {path_addr:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Sibling indices — different children of the same branch give
//    different addresses (BIP-32 isolation).
// ─────────────────────────────────────────────────────────────────────────────

/// BIP-32 sibling derivation: `m/44'/195'/0'/0/0` and `m/44'/195'/0'/0/1`
/// MUST differ. A regression that fixed the address (e.g. cached the
/// first derivation, or skipped the final hardened/non-hardened step)
/// would collapse both indices into one address — caught here.
#[test]
fn v10_sibling_index_produces_different_address() {
    let (addr0, _) = derive_address(&["--index", "0"]);
    let (addr1, path1) = derive_address(&["--index", "1"]);
    assert_ne!(
        addr0, addr1,
        "sibling indices must derive different addresses; both yielded {addr0:?}"
    );
    assert_eq!(path1, "m/44'/195'/0'/0/1");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Coin-type isolation — BTC path vs TRON path must differ.
// ─────────────────────────────────────────────────────────────────────────────

/// SLIP-44 coin types isolate one chain's keyspace from another's at the
/// BIP-32 root. `m/44'/0'/0'/0/0` (Bitcoin) and `m/44'/195'/0'/0/0` (TRON)
/// MUST derive different addresses from the same mnemonic — even though
/// both share the secp256k1 curve and BIP-39 seed, the coin-type
/// hardened step separates the keyspaces.
#[test]
fn v10_different_coin_type_produces_different_address() {
    let (btc_addr, _) = derive_address(&["--path", common::BITCOIN_SLIP44_PATH]);
    let (tron_addr, _) = derive_address(&["--path", common::TRON_SLIP44_PATH]);
    assert_ne!(
        btc_addr, tron_addr,
        "BTC (coin 0) and TRON (coin 195) paths must derive different addresses; \
         both yielded {btc_addr:?} — SLIP-44 isolation broken"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. KAT — canonical `abandon ×11 about` at the default path.
// ─────────────────────────────────────────────────────────────────────────────

/// KAT capture: the canonical BIP-39 phrase at `m/44'/195'/0'/0/0` MUST
/// emit the pinned address below. The CLI output was captured on
/// 2026-09-08 from the shipped binary; if derivation changes
/// (e.g. seed-padding, hardened-step flip), this fails before any
/// downstream Vn test sees a flaky address. The KAT is anchored to a
/// deterministic phrase (no fixture coupling) so the spike can run
/// fully offline.
#[test]
fn v10_kat_canonical_abandon_about_at_default_path_is_pinned() {
    let (addr, path) = derive_address(&["--index", "0"]);
    assert_eq!(path, common::TRON_SLIP44_PATH);
    assert_eq!(
        addr, CANONICAL_ABANDON_ABOUT_AT_DEFAULT_PATH,
        "canonical BIP-39 phrase at default TRON path emitted {addr:?}; \
         expected the pinned KAT {CANONICAL_ABANDON_ABOUT_AT_DEFAULT_PATH:?}. \
         This usually means derivation changed (seed padding, hardened-step, \
         coin-type, or keccak chain). Investigate before re-pinning."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. xpub determinism — same mnemonic + path, same xpub.
// ─────────────────────────────────────────────────────────────────────────────

/// `tron address xpub --wallet-id <id> --password <pw>` exports the
/// extended public key for a stored wallet. xpub derivation is BIP-32
/// deterministic; running twice on the same wallet + path MUST yield
/// byte-equal output. Drives the xpub path through the wallet
/// import→xpub flow (the `tron address xpub` subcommand requires a
/// stored wallet per the CLI surface).
#[test]
fn v10_xpub_export_is_deterministic() {
    // wallet_id is captured from `wallet import --json` output below.
    let dir = tempfile::tempdir().expect("tempdir must succeed");
    common::set_test_data_dir(dir.path().to_path_buf());

    // Step 1: import the canonical phrase as a Nile-testnet wallet.
    let import = common::tron()
        .args([
            "wallet",
            "import",
            "--mnemonic",
            common::CANONICAL_MNEMONIC,
            "--network",
            common::NILE_NETWORK,
            "--json",
        ])
        .env("TRON_PASSWORD", common::V10_PASSWORD)
        .assert()
        .success();
    let import_stdout = String::from_utf8_lossy(&import.get_output().stdout);
    let import_json: serde_json::Value =
        serde_json::from_str(import_stdout.trim()).expect("wallet import --json must emit JSON");
    let wallet_id = import_json
        .get("wallet_id")
        .and_then(|w| w.as_str())
        .expect("wallet import --json must include `wallet_id`")
        .to_string();
    assert_eq!(
        wallet_id.len(),
        32,
        "wallet_id must be 32 hex chars; got {wallet_id:?} (len={})",
        wallet_id.len()
    );

    // Step 2: export xpub twice at the account path; both runs MUST
    // produce byte-equal output. Determinism is the property that makes
    // xpub usable as a watch-only credential (a recipient can re-derive
    // every future address offline).
    let xpub_first = common::tron()
        .args([
            "address",
            "xpub",
            "--wallet-id",
            &wallet_id,
            "--path",
            common::TRON_XPUB_PATH,
            "--json",
        ])
        .env("TRON_PASSWORD", common::V10_PASSWORD)
        .assert()
        .success();
    let xpub_second = common::tron()
        .args([
            "address",
            "xpub",
            "--wallet-id",
            &wallet_id,
            "--path",
            common::TRON_XPUB_PATH,
            "--json",
        ])
        .env("TRON_PASSWORD", common::V10_PASSWORD)
        .assert()
        .success();

    let first_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&xpub_first.get_output().stdout))
            .expect("xpub first run --json must parse");
    let second_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&xpub_second.get_output().stdout))
            .expect("xpub second run --json must parse");

    let first_key = first_json
        .get("xpub")
        .and_then(|x| x.as_str())
        .expect("xpub JSON must include `xpub` string");
    let second_key = second_json
        .get("xpub")
        .and_then(|x| x.as_str())
        .expect("xpub JSON must include `xpub` string");

    assert_eq!(
        first_key, second_key,
        "xpub export must be deterministic across runs; got {first_key:?} vs {second_key:?}"
    );
    // Sanity: a real TRON xpub starts with `xpub` (Bitcoin-style
    // serialization per `crates/tron-wallet-core/src/keys/xpub.rs`).
    assert!(
        first_key.starts_with("xpub"),
        "xpub must start with `xpub` magic; got {first_key:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Drift detector — `tron address new --help` documents `--path`.
// ─────────────────────────────────────────────────────────────────────────────

/// `tron address new` must expose `--path` (the BIP-32 override flag the
/// Plan §V10 surface requires). Anchored on the `--help` banner rather
/// than raw substring search so `--path` cannot appear in boilerplate
/// (env-var descriptions, footer examples) without tripping this. If a
/// future refactor removes `--path` (e.g. drops full-path overrides),
/// the KAT + sibling-index tests above may silently pass via `--index`
/// only — this drift detector fails loud and forces a test-body
/// migration.
#[test]
fn v10_address_new_help_documents_path_flag() {
    let assert = common::tron()
        .args(["address", "new", "--help"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    // Look for the `--path` token on a line by itself (clap renders
    // long flags as `--<flag> <VALUE_NAME>`). A substring scan is
    // sufficient here because `--path` is unambiguous: no other flag
    // starts with `--path`.
    let has_path_flag = stdout.lines().any(|l| l.trim_start().starts_with("--path"));
    assert!(
        has_path_flag,
        "shipped `tron address new --help` must document `--path` (Plan §V10 \
         surface); got help:\n{stdout}"
    );
}
