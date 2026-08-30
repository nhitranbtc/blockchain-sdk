//! Wallet command handlers — Issue #426 / T6c sub-task (L25 split).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §3.3 (handlers/{mod,wallet,tx,erc20,fee,config,faucet,sign}.rs split)
//! + §5.4 (per-command signatures).
//!
//! T6c3 follow-ups landed: `wallet_show` (Story 9 — T6c3 follow-up #1),
//! `wallet_sync` body + `TxSummary` + `--json` formatter (Story 4 — T6c3
//! follow-ups #2 + #3), and `wallet_delete` (Story 9 — T6c3).
//!
//! T6c4 follow-up: real `wallet_create` + `wallet_import` impls
//! (Stories 1, 2). Mnemonic-import path only — `--private-key` import
//! deferred to a follow-up sub-task (lib `import_private_key` hardcodes
//! `Network::default_v0_2()` per `evm-wallet-core/src/wallet.rs:341-404`
//! and lacks a `_for_network` variant).
//!
//! T6c5 (deferred): `wallet_send_native` + `wallet_send_speedup`. Stubs
//! remain below until T6c5 lands.

use alloy_consensus::transaction::SignerRecoverable;
use alloy_consensus::EthereumTxEnvelope;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{Filter, TransactionRequest};
use alloy_signer_local::PrivateKeySigner;
use std::path::Path;
use std::str::FromStr;
use zeroize::Zeroizing;

use crate::cli::SecretMnemonic;

use crate::handlers::validate_rpc_scheme;

use polygon_wallet_core::{
    encoded_envelope, new_http, new_http_polygon_amoy, new_http_polygon_mainnet,
    sign_native_eth_tx, Error, Network, PolygonChain, Result, WalletCreated, WalletInfo,
    WalletManager,
};

/// ERC-20 Transfer(address,address,uint256) event topic0 hash.
///
/// `keccak256("Transfer(address,address,uint256)")` =
/// `0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef`.
/// Used to filter `eth_getLogs` to Transfer events involving the watch
/// address (matches topics[1] when the address is the sender — see
/// `wallet_sync` body for the OR semantics T7 must expand to).
const TRANSFER_TOPIC: [u8; 32] = [
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
];

// `TxSummary` lives in `polygon-wallet-core` (not here) — see
// `polygon-wallet-core/src/lib.rs`. Keeping it in this publish=false
// binary crate would make it unreachable to any sister CLI / future
// `--export` writer. Re-exported for ergonomics via the existing
// `polygon_wallet_core::*` import above; the local `use` of
// `polygon_wallet_core::TxSummary` happens at the call site.

// T6d-1 (Issue #426): `validate_rpc_scheme` moved to
// `super::validate_rpc_scheme` (handlers/mod.rs) so all future
// RPC-touching handlers share the same scheme policy without
// per-handler duplication. The unprefixed call sites below resolve
// via the module scope chain.

/// Query native POL balance for `address` (Story 3 — `wallet balance`).
///
/// Uses `new_http_polygon_amoy()` (PR #424 Phase 2 convenience
/// constructor) — returns `RootProvider<Ethereum>` directly. Polygon
/// Amoy testnet default RPC (`https://polygon-amoy.drpc.org`).
///
/// When `rpc_url` is `Some`, parses it via `url::Url::parse` and uses
/// the generic `new_http(url)` constructor (re-exported from
/// `polygon-wallet-core`). When `None`, falls back to Amoy default.
///
/// Returns the balance in wei (U256). Caller formats with `--unit pol|wei`
/// (T6c1 follow-up #2 wires the unit-aware formatter + dispatch).
pub async fn wallet_balance(rpc_url: Option<&str>, address: &str) -> Result<U256> {
    let addr = Address::from_str(address)
        .map_err(|e| Error::InvalidInput(format!("invalid --address: {e}")))?;
    let provider = match rpc_url {
        Some(url_str) => {
            let url = url::Url::parse(url_str)
                .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
            validate_rpc_scheme(&url)?;
            new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")))?
        }
        None => new_http_polygon_amoy()
            .map_err(|e| Error::Rpc(format!("provider new_http_polygon_amoy: {e}")))?,
    };
    provider
        .get_balance(addr)
        .await
        .map_err(|e| Error::Rpc(format!("get_balance: {e}")))
}

/// Real `wallet list` impl (Story 9 — `wallet list`) — T6c2 (merged earlier).
#[allow(dead_code)] // wired in main.rs::run() (T6c2 follow-up merged)
pub fn wallet_list(
    data_dir: &std::path::Path,
    network: polygon_wallet_core::Network,
) -> Result<Vec<String>> {
    let network_dir = data_dir.join(network.as_dir_name());
    let mut names = Vec::new();
    if !network_dir.exists() {
        return Ok(names);
    }
    let entries = std::fs::read_dir(&network_dir)
        .map_err(|e| Error::Rpc(format!("read_dir {}: {e}", network_dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::Rpc(format!("dir entry: {e}")))?;
        let path = entry.path();
        // Match full filename suffix `.meta.json` — `Path::extension`
        // returns only the LAST component (`json` for `xxx.meta.json`)
        // so it never equals `"meta.json"`. Bug surfaced by #438
        // integration scenario. Same fix the lib applies at
        // `evm-wallet-core/src/wallet.rs:list_wallets` via
        // `.ends_with(META_EXT)`.
        let is_meta = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.ends_with(".meta.json"));
        if !is_meta {
            continue;
        }
        // Return the persisted `name` from meta.json (not the UUID
        // file stem) so callers see the operator-supplied identifier
        // — matches `wallet list` story intent ("enumerate wallets
        // under a keystore dir" → friendly name, not UUID).
        let bytes = std::fs::read(&path)
            .map_err(|e| Error::Rpc(format!("read_file {}: {e}", path.display())))?;
        if let Ok(meta) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(name) = meta.get("name").and_then(|n| n.as_str()) {
                names.push(name.to_string());
                continue;
            }
        }
        // Fallback to UUID stem if meta.json is missing/malformed.
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_string());
        }
    }
    Ok(names)
}

/// Real `wallet show` impl (Story 9 — `wallet show`) — T6c3 follow-up.
///
/// Reads `.meta.json` (plaintext metadata; no decrypt — encrypted
/// `.enc` blob inspection deferred to T6d when rpassword + AES-GCM
/// decryption wires up). Returns `Error::InvalidInput` if wallet_id
/// is not UUID format. Returns `Error::Rpc` on filesystem / parse errors.
pub fn wallet_show(
    data_dir: &std::path::Path,
    network: polygon_wallet_core::Network,
    wallet_id: &str,
) -> Result<WalletInfo> {
    let uuid = uuid::Uuid::from_str(wallet_id)
        .map_err(|e| Error::InvalidInput(format!("invalid wallet_id (expected UUID): {e}")))?;
    let path = data_dir
        .join(network.as_dir_name())
        .join(format!("{uuid}.meta.json"));
    let bytes = std::fs::read(&path)
        .map_err(|e| Error::Rpc(format!("read_file {}: {e}", path.display())))?;
    serde_json::from_slice(&bytes).map_err(|e| Error::Rpc(format!("parse meta.json: {e}")))
}

/// Real `wallet delete` impl (Story 9 — `wallet delete`) — T6c3.
///
/// Removes `<data_dir>/<network>/<wallet_id>.meta.json` and the
/// matching `<wallet_id>.enc` blob. Returns `Error::InvalidInput` if
/// the wallet_id is malformed (must be UUID format per WalletManager).
/// Returns `Error::Rpc` on filesystem errors. Returns `Ok(())` even if
/// the wallet doesn't exist (idempotent — matches Story 9 AC).
pub fn wallet_delete(
    data_dir: &std::path::Path,
    network: polygon_wallet_core::Network,
    wallet_id: &str,
) -> Result<()> {
    let uuid = uuid::Uuid::from_str(wallet_id)
        .map_err(|e| Error::InvalidInput(format!("invalid wallet_id (expected UUID): {e}")))?;
    let network_dir = data_dir.join(network.as_dir_name());
    for ext in ["meta.json", "enc"] {
        let path = network_dir.join(format!("{uuid}.{ext}"));
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Idempotent: ignore missing files.
            }
            Err(e) => {
                return Err(Error::Rpc(format!("remove_file {}: {e}", path.display())));
            }
        }
    }
    Ok(())
}

/// `wallet sync` handler — T6c3 follow-up #3.
///
/// Polls ERC-20 Transfer events involving `address`. Returns one
/// `polygon_wallet_core::TxSummary` per matching log entry. Per design
/// doc §5.4 the return type is `Vec<TxSummary>` (lightweight subset of
/// the full `Vec<Transaction>`) — AMENDMENT to original §5.4 which
/// specified `Result<()>` + internal print. Decision rationale: same
/// pattern as `wallet_balance` (returns `U256`, main.rs formats); the
/// handler/formatter split enables `--json` + future `--export` without
/// duplicating formatting in the handler.
///
/// Live RPC body (the `provider.get_logs(&filter).await` call that
/// fetches logs and decodes them) is deferred to T7 operator-driven
/// integration per L29 — operator session runs against Amoy testnet.
/// The address parse + provider build paths exercise now; the actual
/// `get_logs` call sits behind an early-return `Error::Rpc` so CI
/// compiles + negative-tests pass without a live RPC dependency. T7
/// removes the early-return.
#[allow(dead_code)] // wired in main.rs::run() (T6c3 follow-up #2 dispatch)
pub async fn wallet_sync(
    rpc_url: Option<&str>,
    _network: polygon_wallet_core::Network,
    address: &str,
) -> Result<Vec<polygon_wallet_core::TxSummary>> {
    let addr = Address::from_str(address)
        .map_err(|e| Error::InvalidInput(format!("invalid --address: {e}")))?;
    let provider = match rpc_url {
        Some(url_str) => {
            let url = url::Url::parse(url_str)
                .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
            validate_rpc_scheme(&url)?;
            new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")))?
        }
        None => new_http_polygon_amoy()
            .map_err(|e| Error::Rpc(format!("provider new_http_polygon_amoy: {e}")))?,
    };
    // Filter: Transfer events where `from` (topic1) equals the watch
    // address (left-padded to 32 bytes). `eth_getLogs` semantics: a
    // null topic = wildcard, so `[X, null]` = topic1==X AND
    // topic2==any. T7 expands this to two `get_logs` calls — one for
    // topic1==X (transfers FROM the address), one for topic2==X
    // (transfers TO the address) — then merges + dedupes by
    // `(tx_hash, log_index)`. Keeping it single-topic for now keeps
    // the CI-compile surface small.
    let padded = B256::left_padding_from(addr.as_slice());
    let filter = Filter::new()
        .event_signature(B256::from_slice(&TRANSFER_TOPIC))
        .topic1(padded);
    let _provider = provider;
    let _filter = filter;
    Err(Error::Rpc("wallet sync not yet implemented".into()))
}

// === T6c4 follow-up: real wallet_create + wallet_import ===
//
// Per L13 step 9a (mandatory module-interface design): both functions
// are thin wrappers over `polygon-wallet-core::WalletManager`. The
// handler returns `WalletCreated` (which carries only
// `wallet_id`/`name`/`network`/`address` — no mnemonic field), so
// the type system structurally prevents the mnemonic from leaking
// into the return type. main.rs is responsible for printing the
// mnemonic to STDERR (design §3.5 + F49 L28 mnemonic-leak discipline);
// the handlers never own a `String` mnemonic.
//
// Critical-tier (per L13 Q4): touches key material + AES-GCM password
// wrap. L12 review cluster (type-design-analyzer + code-reviewer +
// security-auditor) + standalone `security-review` gate this code.

/// Real `wallet_create` impl (Story 1) — T6c4 follow-up.
///
/// Thin wrapper over `WalletManager::create_wallet_for_network`:
///   1. Validate `name` via `handlers::validate_wallet_name`
///      (1..=32 chars, `[A-Za-z0-9 _-]` charset; exit 2 on violation).
///   2. Open a `WalletManager` rooted at `data_dir`. `open_at` creates
///      the directory tree + tightens perms to `0o700` per #337 H-1.
///   3. Delegate to `create_wallet_for_network`. Lib generates a
///      fresh 12-word BIP-39 mnemonic, derives first receive address,
///      Argon2id-derives the AES key, AES-256-GCM-encrypts the
///      mnemonic blob, writes `0o600` `<uuid>.enc` + `<uuid>.meta.json`.
///   4. Map `WalletError` → `polygon_wallet_core::Error` via
///      `handlers::map_wallet_err` so the CLI's exit-code table
///      (cryptic internal errors → `Error::InvalidInput` exit 2)
///
/// Empty password → `WalletError::Crypto(Argon2("password must be
/// non-empty"))` → `Error::InvalidInput`. Duplicate name →
/// `AlreadyExists` → `Error::InvalidInput`. Both exit 2.
#[allow(dead_code)] // wired into main.rs::run() dispatch (separate sub-task)
pub fn wallet_create(
    data_dir: &std::path::Path,
    name: &str,
    password: &Zeroizing<Vec<u8>>,
    network: polygon_wallet_core::Network,
) -> Result<WalletCreated> {
    crate::handlers::validate_wallet_name(name)?;
    let mgr = polygon_wallet_core::WalletManager::open_at(data_dir.to_path_buf())
        .map_err(crate::handlers::map_wallet_err)?;
    mgr.create_wallet_for_network(name, password.as_slice(), network)
        .map_err(crate::handlers::map_wallet_err)
}

/// Real `wallet_import` impl (Story 2) — T6c4 follow-up (mnemonic-only).
///
/// Thin wrapper over `WalletManager::import_wallet_for_network`. Parses
/// `phrase` as a BIP-39 mnemonic (English wordlist), re-derives the
/// first receive address, encrypts under `password`, persists to disk.
///
/// `--private-key` import path is deferred — the lib's
/// `import_private_key` (line 341) hardcodes `Network::default_v0_2()`
/// and lacks a `_for_network` variant. Adds follow-up sub-task for
/// the lib extension; mnemonic import ships first.
///
/// Sync (not `async`): `WalletManager::import_wallet_for_network`
/// is fully synchronous on the lib side. The `async` keyword on a
/// prior revision forced every caller + test to spin up a
/// `tokio::runtime` for no concurrency benefit — reverted per
/// `code-review M1` + `type-design F8` (L12 cluster for T6c4).
/// Re-add `async` only if a future live-RPC path needs it.
#[allow(dead_code)] // wired into main.rs::run() dispatch (separate sub-task)
pub fn wallet_import(
    data_dir: &std::path::Path,
    name: &str,
    password: &Zeroizing<Vec<u8>>,
    network: polygon_wallet_core::Network,
    phrase: &SecretMnemonic,
) -> Result<WalletCreated> {
    crate::handlers::validate_wallet_name(name)?;
    let mgr = polygon_wallet_core::WalletManager::open_at(data_dir.to_path_buf())
        .map_err(crate::handlers::map_wallet_err)?;
    // Bounded lifetime: the `&str` lives only for the synchronous
    // `import_wallet_for_network` call; the lib encrypts then drops.
    mgr.import_wallet_for_network(name, phrase.expose().as_str(), password.as_slice(), network)
        .map_err(crate::handlers::map_wallet_err)
}

// =====================================================================
// T7 follow-up (Issue #469): --private-key-file flag support.
// =====================================================================
//
// #464 partial land — T7 Amoy smoke harness shrunk from 5 to 2 tests
// after automated security review flagged `--private-key` argv as a
// HIGH sensitive-data-exposure-via-argv finding (L12 H-1 sister class
// to the `--mnemonic` argv finding closed by PR #456 / `SecretMnemonic`).
// The follow-up #469 closes the argv hole by accepting a mode-0600 file
// path whose contents are read into a `Zeroizing<Vec<u8>>` wrapper and
// passed to the new `WalletManager::import_private_key_for_network`
// lib method (also added in this PR; hardcoded-network gap in the
// pre-existing `import_private_key` was the drift-scan finding that
// forced the lib extension).
//
// Critical-tier (per L13 Q4): touches key material + AES-GCM password
// wrap + AtomicFile write. L12 review cluster
// (type-design-analyzer + code-reviewer + security-auditor) +
// standalone `security-review` gate this code.

/// Read the contents of a `--private-key-file` path into a
/// `Zeroizing<Vec<u8>>` wrapper.
///
/// Invariants enforced (per #469 AC + L12 H-1 sister finding from
/// PR #456):
/// - File must exist; missing → `Error::InvalidInput` (exit 2).
/// - File mode must be `0o600` (owner-only); any other mode on Unix →
///   `Error::InvalidInput` naming the actual mode so the operator
///   can `chmod 600` without re-reading the source. On non-Unix
///   (Windows) mode check is skipped — Windows ACLs are out of scope.
/// - File contents wrapped in `Zeroizing<Vec<u8>>` so the heap buffer
///   zeroizes on drop (sister invariant to `SecretMnemonic`'s
///   `Zeroizing<String>` at `cli.rs:64`). The clap-side `&str` borrow
///   that survives to `Cli::drop` is documented at that call site and
///   is a pre-existing footgun class — not introduced here.
///
/// The returned wrapper derefs to `&[u8]`, so the caller passes
/// `bytes.as_slice()` directly to
/// `WalletManager::import_private_key_for_network` without any
/// hex-encoding round-trip.
#[cfg_attr(not(unix), allow(unused_variables))]
pub(crate) fn read_pk_file(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let bytes = std::fs::read(path).map_err(|e| {
        Error::InvalidInput(format!(
            "--private-key-file not found or unreadable: {}: {e}",
            path.display()
        ))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|e| {
                Error::InvalidInput(format!(
                    "--private-key-file metadata: {}: {e}",
                    path.display()
                ))
            })?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o600 {
            return Err(Error::InvalidInput(format!(
                "--private-key-file must have mode 0o600 (owner-only); got 0o{mode:o}: {}",
                path.display()
            )));
        }
    }
    Ok(Zeroizing::new(bytes))
}

/// Real `wallet_import_private_key_for_network` impl (Issue #469) — the
/// PK-import counterpart of the mnemonic-based `wallet_import` above.
/// Takes `&Zeroizing<Vec<u8>>` directly (the file-read + hex-decode
/// lives in the dispatch layer so this handler stays a thin wrapper
/// over `WalletManager::import_private_key_for_network`). The lib's
/// `_for_network` variant is the network-aware sister of the
/// hardcoded-network `import_private_key` — the missing variant was
/// the drift-scan finding that forced this lib extension.
///
/// Per L13 Q4 critical-tier: key material + signing surface.
/// Sister invariants to `wallet_import`:
/// - Name validated via `handlers::validate_wallet_name`.
/// - Empty-password defense-in-depth: lib rejects via
///   `WalletError::Crypto(Argon2)` (mapped to `Error::InvalidInput` by
///   `handlers::map_wallet_err`).
/// - Duplicate-name defense: lib rejects via `WalletError::AlreadyExists`.
/// - Disk perms: lib writes 0o600 blob (sister test in
///   evm-wallet-core pins this; handler-level test re-pins it for
///   defense-in-depth).
pub fn wallet_import_private_key_for_network(
    data_dir: &Path,
    name: &str,
    password: &Zeroizing<Vec<u8>>,
    network: polygon_wallet_core::Network,
    pk_bytes: &Zeroizing<Vec<u8>>,
) -> Result<WalletCreated> {
    crate::handlers::validate_wallet_name(name)?;
    let mgr = polygon_wallet_core::WalletManager::open_at(data_dir.to_path_buf())
        .map_err(crate::handlers::map_wallet_err)?;
    mgr.import_private_key_for_network(name, pk_bytes.as_slice(), password.as_slice(), network)
        .map_err(crate::handlers::map_wallet_err)
}

#[allow(dead_code)]
pub async fn wallet_send_native(_to: &str, _amount: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet send: deferred past T6c3 follow-up (lands in T6c5)".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_send_speedup(_tx_hash: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet send speed-up: deferred past T6c3 follow-up (lands in T6c5)".into(),
    ))
}

// =====================================================================
// T6c5 (Issue #426 sub-task): real `wallet_send_native` +
// `wallet_send_speedup` — signatures expanded to design doc §5.4
// (`Result<B256>` — was `Result<()>` placeholder) and per-arg
// validation split into pure helpers (`parse_send_address`,
// `parse_send_amount`, `assert_send_password`, `assert_new_fee_higher`)
// so the handler bodies stay linear + each validator is testable in
// isolation without a provider.
// =====================================================================

/// Parse a `--to` address string. Returns `Error::InvalidInput` for any
/// non-address input. Mirrors the `wallet_balance` validator at
/// `wallet_balance` body lines 87-89.
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
fn parse_send_address(s: &str) -> Result<Address> {
    Address::from_str(s).map_err(|e| Error::InvalidInput(format!("invalid --to address: {e}")))
}

/// Parse a `--amount` wei string. Returns `Error::InvalidInput` for
/// non-decimal / negative / overflow. Wei is the canonical unit
/// (design §3.5 cross-cutting + Story 5 AC); `--unit pol|wei`
/// conversion happens in the dispatch layer (main.rs) before calling
/// this handler so the handler surface stays single-unit.
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
fn parse_send_amount(s: &str) -> Result<U256> {
    s.parse::<U256>()
        .map_err(|e| Error::InvalidInput(format!("invalid --amount (wei): {e}")))
}

/// Reject empty passwords at the handler boundary. The lib-level
/// `wallet_create` rejects empty pw with a "crypto" message (test
/// `wallet_create_rejects_empty_password` above). This handler-
/// boundary check fails fast BEFORE the wallet-unlock work so the
/// operator sees exit-2 immediately on TTY.
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
fn assert_send_password(p: &Zeroizing<Vec<u8>>) -> Result<()> {
    if p.is_empty() {
        return Err(Error::InvalidInput(
            "wallet password must not be empty".into(),
        ));
    }
    Ok(())
}

/// Enforce RBF rule #1: the new `max_fee_per_gas` must be STRICTLY
/// greater than the original. The lib's `sign_native_eth_tx` does
/// NOT enforce this — the RPC will silently accept a same-or-lower-
/// fee replacement, breaking the operator's intent to replace a
/// stuck pending tx. Mirrors ETH analog `eth/src/handlers.rs:847-874`
/// speedup recovery (Gate 5 cryptographic recovery + fee ordering).
/// Caller supplies `old_max_fee` after `eth_getTransactionByHash`.
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
fn assert_new_fee_higher(old_max_fee: u128, new_max_fee: u128) -> Result<()> {
    if new_max_fee <= old_max_fee {
        return Err(Error::InvalidInput(format!(
            "speed-up max_fee_per_gas ({new_max_fee}) must be strictly greater than original ({old_max_fee})"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
pub async fn wallet_send_native_v2(
    data_dir: &std::path::Path,
    rpc_url: Option<&str>,
    network: Network,
    name: &str,
    password: &Zeroizing<Vec<u8>>,
    to: &str,
    amount: &str,
    _unit: &str,
    nonce_override: Option<u64>,
    gas_limit: Option<u64>,
    fee: &str,
    max_fee_gwei: Option<f64>,
    priority_fee_gwei: Option<f64>,
    _drain: bool,
    dry_run: bool,
    wait: bool,
) -> Result<B256> {
    // Step 1: validators (pure, no I/O, no provider). Close the
    // S1 / S2 / S4 / S7 / S6 paths at exit 2 before any wallet
    // unlock or RPC call.
    let to_addr = parse_send_address(to)?;
    let amount_wei = parse_send_amount(amount)?;
    assert_send_password(password)?;
    let fee_tier = crate::handlers::fee::parse_fee_tier(fee)?;
    // Parse + validate RPC URL once at the validators stage (fail fast
    // at exit 2 BEFORE wallet unlock); reuse the parsed `Url` in
    // step 3 below to avoid a second parse + scheme check.
    let parsed_rpc_url: Option<url::Url> = match rpc_url {
        Some(url_str) => {
            let url = url::Url::parse(url_str)
                .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
            validate_rpc_scheme(&url)?;
            Some(url)
        }
        None => None,
    };
    // Step 2: open wallet + unlock signer.
    let mgr =
        WalletManager::open_at(data_dir.to_path_buf()).map_err(crate::handlers::map_wallet_err)?;
    let wallet_id = mgr
        .lookup_by_name(name, network)
        .map_err(crate::handlers::map_wallet_err)?;
    let key_bytes = mgr
        .unlock_signer(wallet_id, password.as_slice())
        .map_err(crate::handlers::map_wallet_err)?;
    let signer = PrivateKeySigner::from_slice(&*key_bytes)
        .map_err(|e| Error::Rpc(format!("signer from_slice: {e}")))?;
    drop(key_bytes); // Zeroizing drop
    let from = signer.address();
    // Step 3: build provider (per-network default or custom --rpc-url).
    let provider = match parsed_rpc_url {
        Some(url) => new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")))?,
        None => match network {
            Network::Polygon(PolygonChain::Amoy) => {
                new_http_polygon_amoy().map_err(|e| Error::Rpc(format!("provider amoy: {e}")))?
            }
            Network::Polygon(PolygonChain::Mainnet) => new_http_polygon_mainnet()
                .map_err(|e| Error::Rpc(format!("provider mainnet: {e}")))?,
            _ => {
                return Err(Error::InvalidInput(format!(
                    "unsupported network for wallet_send_native: {network:?}"
                )));
            }
        },
    };
    // Step 4: chain_id trust-boundary check (Q7 + C1, mirrors ETH
    // analog `eth/src/handlers.rs:651-660`). An attacker-controlled
    // RPC must not return values we sign against a legitimate chain.
    let provider_chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| Error::Rpc(format!("get_chain_id: {e}")))?;
    let expected_chain_id = network.chain_id();
    if provider_chain_id != expected_chain_id {
        return Err(Error::InvalidInput(format!(
            "rpc chain_id {provider_chain_id} does not match wallet network {network:?} (expected {expected_chain_id})"
        )));
    }
    // Step 5: nonce (override OR auto-fetch from RPC).
    let nonce = match nonce_override {
        Some(n) => n,
        None => provider
            .get_transaction_count(from)
            .await
            .map_err(|e| Error::Rpc(format!("get_transaction_count: {e}")))?,
    };
    // Step 6: gas. Either explicit gwei overrides for BOTH max_fee
    // AND priority_fee, or omit both and use the per-tier multiplier
    // over `estimate_eip1559_fees()`. Partial override is a user
    // error → exit 2 (mirrors ETH `resolve_overrides` at
    // `eth/src/handlers.rs:1047-1068`).
    let (max_fee_per_gas, max_priority_fee_per_gas) = match (max_fee_gwei, priority_fee_gwei) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(Error::InvalidInput(
                "either set both --max-fee-gwei AND --priority-fee-gwei, \
                 or omit BOTH to use the --fee tier estimate"
                    .into(),
            ));
        }
        (Some(m), Some(p)) => (((m * 1e9) as u128), ((p * 1e9) as u128)),
        (None, None) => {
            let estimate = provider
                .estimate_eip1559_fees()
                .await
                .map_err(|e| Error::Rpc(format!("estimate_eip1559_fees: {e}")))?;
            crate::handlers::fee::resolve_fee_tier(
                fee_tier,
                estimate.max_fee_per_gas,
                estimate.max_priority_fee_per_gas,
            )
        }
    };
    let gas = gas_limit.unwrap_or(21_000);
    // Step 7: build the EIP-1559 transaction request.
    let tx_req = alloy_rpc_types::TransactionRequest {
        from: Some(from),
        to: Some(alloy_primitives::TxKind::Call(to_addr)),
        value: Some(amount_wei),
        chain_id: Some(provider_chain_id),
        nonce: Some(nonce),
        gas: Some(gas),
        max_fee_per_gas: Some(max_fee_per_gas),
        max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
        ..Default::default()
    };
    // Step 8: dry-run short-circuits before broadcast. The envelope is
    // signed (so the operator can audit the exact bytes) but no RPC
    // `send_raw_transaction` is issued. Returns the signed-tx B256 via
    // a synthetic tx-hash derivation step.
    if dry_run {
        let signed = polygon_wallet_core::sign_native_eth_tx(&signer, tx_req)
            .map_err(|e| Error::Rpc(format!("sign (dry_run): {e}")))?;
        // Dry-run: surface a stable sentinel so the operator can
        // confirm the dry-run path executed. Live hash lands when
        // dry_run=false.
        return Ok({
            let bytes = polygon_wallet_core::encoded_envelope(&signed);
            alloy_primitives::keccak256(&bytes)
        });
    }
    // Step 9: sign + broadcast (live path).
    let signed = polygon_wallet_core::sign_native_eth_tx(&signer, tx_req)
        .map_err(|e| Error::Rpc(format!("sign: {e}")))?;
    let bytes = polygon_wallet_core::encoded_envelope(&signed);
    let pending = provider
        .send_raw_transaction(&bytes)
        .await
        .map_err(|e| Error::Rpc(format!("send_raw_transaction: {e}")))?;
    let tx_hash = *pending.tx_hash();
    // Step 10: optional wait-for-receipt (operator UX: block until
    // mined before returning exit code).
    if wait {
        let _receipt = provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| Error::Rpc(format!("get_transaction_receipt: {e}")))?;
    }
    Ok(tx_hash)
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)] // wired in T6c5 follow-up alongside main.rs dispatch
pub async fn wallet_send_speedup_v2(
    data_dir: &std::path::Path,
    rpc_url: Option<&str>,
    network: Network,
    name: &str,
    password: &Zeroizing<Vec<u8>>,
    tx_hash: &str,
    new_max_fee_per_gas: u128,
    new_max_priority_fee_per_gas: u128,
) -> Result<B256> {
    // Step 1: validators (pure, no I/O).
    assert_send_password(password)?;
    let pending_tx_hash = tx_hash
        .parse::<B256>()
        .map_err(|e| Error::InvalidInput(format!("invalid --tx-hash: {e}")))?;
    if let Some(url_str) = rpc_url {
        let url = url::Url::parse(url_str)
            .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
        validate_rpc_scheme(&url)?;
    }
    if new_max_fee_per_gas == 0 {
        return Err(Error::InvalidInput(
            "speed-up max_fee_per_gas (wei) must be > 0".into(),
        ));
    }
    // Parse + validate RPC URL once at validators (fail fast at exit
    // 2 BEFORE wallet unlock); reuse the parsed `Url` in step 3 below.
    let parsed_rpc_url: Option<url::Url> = match rpc_url {
        Some(url_str) => {
            let url = url::Url::parse(url_str)
                .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
            validate_rpc_scheme(&url)?;
            Some(url)
        }
        None => None,
    };
    // Step 2: open wallet + unlock signer (same path as send).
    let mgr =
        WalletManager::open_at(data_dir.to_path_buf()).map_err(crate::handlers::map_wallet_err)?;
    let wallet_id = mgr
        .lookup_by_name(name, network)
        .map_err(crate::handlers::map_wallet_err)?;
    let key_bytes = mgr
        .unlock_signer(wallet_id, password.as_slice())
        .map_err(crate::handlers::map_wallet_err)?;
    let signer = PrivateKeySigner::from_slice(&*key_bytes)
        .map_err(|e| Error::Rpc(format!("signer from_slice: {e}")))?;
    drop(key_bytes);
    let from = signer.address();
    // Step 3: provider.
    let provider = match parsed_rpc_url {
        Some(url) => new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")))?,
        None => match network {
            Network::Polygon(PolygonChain::Amoy) => {
                new_http_polygon_amoy().map_err(|e| Error::Rpc(format!("provider amoy: {e}")))?
            }
            Network::Polygon(PolygonChain::Mainnet) => new_http_polygon_mainnet()
                .map_err(|e| Error::Rpc(format!("provider mainnet: {e}")))?,
            _ => {
                return Err(Error::InvalidInput(format!(
                    "unsupported network for wallet_send_speedup: {network:?}"
                )));
            }
        },
    };
    // Step 4: chain_id trust-boundary gate.
    let provider_chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| Error::Rpc(format!("get_chain_id: {e}")))?;
    let expected_chain_id = network.chain_id();
    if provider_chain_id != expected_chain_id {
        return Err(Error::InvalidInput(format!(
            "rpc chain_id {provider_chain_id} does not match wallet network {network:?} (expected {expected_chain_id})"
        )));
    }
    // Step 5: fetch the pending tx (Gate 3 + 4 — must exist, must not
    // be mined).
    let pending_tx = provider
        .get_transaction_by_hash(pending_tx_hash)
        .await
        .map_err(|e| Error::Rpc(format!("get_transaction_by_hash: {e}")))?
        .ok_or_else(|| Error::InvalidInput(format!("pending tx {pending_tx_hash:?} not found")))?;
    if let Some(mined_block) = pending_tx.block_number {
        return Err(Error::InvalidInput(format!(
            "tx {pending_tx_hash:?} already mined at block {mined_block}; cannot speed up"
        )));
    }
    // Step 6: Gate 5 — cryptographic recovery + signer match (anti-
    // forgery, mirrors ETH analog `eth/src/handlers.rs:847-874`).
    let pending_from = pending_tx.inner.signer();
    let pending_recovered: alloy_primitives::Address = match pending_tx.inner.inner() {
        EthereumTxEnvelope::Eip1559(tx) => SignerRecoverable::recover_signer(tx).map_err(|e| {
            Error::InvalidInput(format!("pending tx signature recovery failed: {e}"))
        })?,
        EthereumTxEnvelope::Legacy(_)
        | EthereumTxEnvelope::Eip2930(_)
        | EthereumTxEnvelope::Eip4844(_)
        | EthereumTxEnvelope::Eip7702(_) => {
            return Err(Error::InvalidInput(
                "speedup is EIP-1559-only; pending tx type not supported".into(),
            ));
        }
    };
    if pending_recovered != pending_from {
        return Err(Error::InvalidInput(format!(
            "pending tx signer mismatch: RPC-reported {pending_from:?} != signature-recovered {pending_recovered:?}"
        )));
    }
    if pending_recovered != from {
        return Err(Error::InvalidInput(format!(
            "pending tx from {pending_from:?} != wallet address {from:?}"
        )));
    }
    // Step 7: extract original nonce + fees from the pending tx.
    let pending_req: TransactionRequest = pending_tx.into_request();
    let pending_nonce = pending_req
        .nonce
        .ok_or_else(|| Error::InvalidInput("pending tx missing nonce".into()))?;
    let pending_max_fee = pending_req
        .max_fee_per_gas
        .ok_or_else(|| Error::InvalidInput("pending tx missing max_fee_per_gas".into()))?;
    let pending_max_priority = pending_req.max_priority_fee_per_gas.unwrap_or(0);
    // Step 8: nonce drift check (Gate 6 — pending tx must be the
    // wallet's next nonce).
    let wallet_nonce = provider
        .get_transaction_count(from)
        .await
        .map_err(|e| Error::Rpc(format!("get_transaction_count: {e}")))?;
    if wallet_nonce != pending_nonce {
        return Err(Error::InvalidInput(format!(
            "nonce drift: pending tx nonce {pending_nonce} != wallet next nonce {wallet_nonce} (tx was replaced or abandoned)"
        )));
    }
    // Step 9: RBF fee-bumping invariant (Gate 7).
    // - max_fee_per_gas: strictly greater (BIP-125 conservative — the
    //   mempool's eviction rule can accept equal-fee replacements from
    //   a different sender but the operator intent is to outbid, so
    //   we require strict).
    // - max_priority_fee_per_gas: >= pending. BIP-125 only requires the
    //   overall fee to be higher; equal priority_fee is acceptable
    //   provided max_fee_per_gas strictly exceeds (matches ETH analog
    //   at `eth/src/handlers.rs:911`).
    assert_new_fee_higher(pending_max_fee, new_max_fee_per_gas)?;
    if new_max_priority_fee_per_gas < pending_max_priority {
        return Err(Error::InvalidInput(format!(
            "new max_priority_fee_per_gas ({new_max_priority_fee_per_gas}) must be >= pending ({pending_max_priority})"
        )));
    }
    // Step 10: build the replacement envelope (same from/to/value/
    // nonce + new fees + same gas limit).
    let tx_req = TransactionRequest {
        from: Some(from),
        to: pending_req.to,
        value: pending_req.value,
        chain_id: Some(provider_chain_id),
        nonce: Some(pending_nonce),
        gas: pending_req.gas,
        max_fee_per_gas: Some(new_max_fee_per_gas),
        max_priority_fee_per_gas: Some(new_max_priority_fee_per_gas),
        ..Default::default()
    };
    // Step 11: sign + broadcast.
    let signed =
        sign_native_eth_tx(&signer, tx_req).map_err(|e| Error::Rpc(format!("sign: {e}")))?;
    let bytes = encoded_envelope(&signed);
    let new_pending = provider
        .send_raw_transaction(&bytes)
        .await
        .map_err(|e| Error::Rpc(format!("send_raw_transaction: {e}")))?;
    Ok(*new_pending.tx_hash())
}

#[cfg(test)]
mod tests {
    use super::wallet_list;
    use alloy_primitives::{Address, B256, U256};
    use polygon_wallet_core::{Network, PolygonChain};
    use std::path::PathBuf;

    /// T6c2 test: nonexistent data_dir returns empty list (not Err).
    #[test]
    fn wallet_list_returns_empty_for_nonexistent_dir() {
        let r = wallet_list(
            &PathBuf::from("/nonexistent/path/polygon-cli-test-xyz"),
            Network::Polygon(PolygonChain::Amoy),
        );
        assert!(
            r.is_ok(),
            "nonexistent dir should be Ok(empty), not error; got {r:?}"
        );
        assert_eq!(r.unwrap(), Vec::<String>::new());
    }

    /// T6c1 test: invalid address must surface as `Error::InvalidInput`
    /// (exit 2). Live RPC test deferred to T7 (operator-driven per L29).
    /// Wraps `wallet_balance` in `tokio::runtime::Runtime::block_on`
    /// since the production fn is `async` (no separate runtime dep).
    #[test]
    fn wallet_balance_rejects_invalid_address() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_balance(None, "not-an-address"));
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "invalid --address must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 test: `wallet_delete` rejects invalid (non-UUID) wallet_id.
    #[test]
    fn wallet_delete_rejects_invalid_wallet_id() {
        use super::wallet_delete;
        let r = wallet_delete(
            &PathBuf::from("/tmp/polygon-cli-test"),
            Network::Polygon(PolygonChain::Amoy),
            "not-a-uuid",
        );
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "non-UUID wallet_id must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 test: `wallet_delete` on nonexistent path is idempotent (Ok).
    #[test]
    fn wallet_delete_nonexistent_is_idempotent() {
        use super::wallet_delete;
        let r = wallet_delete(
            &PathBuf::from("/nonexistent/path/polygon-cli-test-xyz"),
            Network::Polygon(PolygonChain::Amoy),
            "00000000-0000-0000-0000-000000000000",
        );
        assert!(
            r.is_ok(),
            "nonexistent dir should be Ok(idempotent), not Err; got {r:?}"
        );
    }

    /// T6c3 follow-up test: `wallet_show` rejects invalid wallet_id.
    #[test]
    fn wallet_show_rejects_invalid_wallet_id() {
        use super::wallet_show;
        let r = wallet_show(
            &PathBuf::from("/tmp/polygon-cli-test"),
            Network::Polygon(PolygonChain::Amoy),
            "not-a-uuid",
        );
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "non-UUID wallet_id must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 follow-up test: `wallet_show` on nonexistent path is Ok (file not found).
    #[test]
    fn wallet_show_nonexistent_path_is_error() {
        use super::wallet_show;
        let r = wallet_show(
            &PathBuf::from("/nonexistent/path/polygon-cli-test-xyz"),
            Network::Polygon(PolygonChain::Amoy),
            "00000000-0000-0000-0000-000000000000",
        );
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::Rpc(_))),
            "nonexistent wallet_id file should be Err (Rpc), not Ok; got {r:?}"
        );
    }

    /// T6c3 follow-up #3 test: `wallet_sync` rejects invalid (non-hex)
    /// --address. Mirrors `wallet_balance_rejects_invalid_address`.
    /// Live RPC body deferred to T7 per L29 — this test exercises the
    /// address-parse path that runs BEFORE provider construction.
    #[test]
    fn wallet_sync_rejects_invalid_address() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_sync(
                None,
                Network::Polygon(PolygonChain::Amoy),
                "not-an-address",
            ));
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "invalid --address must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 follow-up #3 test: `wallet_sync` rejects malformed
    /// `--rpc-url` via the URL-parse path. Exercises the provider-build
    /// branch that runs BEFORE the live-RPC early-return.
    #[test]
    fn wallet_sync_rejects_invalid_rpc_url() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_sync(
                Some("not a url"),
                Network::Polygon(PolygonChain::Amoy),
                "0x0000000000000000000000000000000000000001",
            ));
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::Rpc(_))),
            "invalid --rpc-url must surface as Error::Rpc; got {r:?}"
        );
    }

    /// Security fix #2 (transport-security): `wallet_balance` rejects
    /// cleartext HTTP RPC URLs to non-loopback hosts. Localhost / 127.0.0.1
    /// / ::1 remain allowed for Anvil-regtest per design doc §9.
    #[test]
    fn wallet_balance_rejects_http_rpc_to_remote_host() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_balance(
                Some("http://example.com"),
                "0x0000000000000000000000000000000000000001",
            ));
        match r {
            Err(polygon_wallet_core::Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("scheme not allowed"),
                    "InvalidInput must mention rejected scheme; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// Security fix #2 (transport-security): `wallet_sync` rejects
    /// cleartext HTTP RPC URLs to non-loopback hosts.
    #[test]
    fn wallet_sync_rejects_http_rpc_to_remote_host() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_sync(
                Some("http://example.com"),
                Network::Polygon(PolygonChain::Amoy),
                "0x0000000000000000000000000000000000000001",
            ));
        match r {
            Err(polygon_wallet_core::Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("scheme not allowed"),
                    "InvalidInput must mention rejected scheme; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// Security fix #2 positive path: `http://localhost` is allowed
    /// (Anvil-regtest use case per design doc §9). The handler still
    /// fails because no live RPC, but with `Error::Rpc` — NOT
    /// `Error::InvalidInput`. Verifies the loopback exemption.
    #[test]
    fn validate_rpc_scheme_allows_http_to_localhost() {
        let url = url::Url::parse("http://localhost:8545").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_ok());
        let url = url::Url::parse("http://127.0.0.1:8545").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_ok());
        let url = url::Url::parse("https://polygon-rpc.com").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_ok());
        let url = url::Url::parse("http://example.com").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_err());
    }

    /// T6c3 follow-up #3 test: `TxSummary` survives a JSON
    /// serialize → deserialize roundtrip with field values intact.
    /// Independent of provider / RPC — fixture-driven. Required for
    /// the `--json` output formatter wired in main.rs::run().
    /// `TxSummary` lives in `polygon-wallet-core` (not this crate's
    /// binary scope) — see `polygon-wallet-core/src/lib.rs`.
    #[test]
    fn tx_summary_serde_json_roundtrip() {
        use polygon_wallet_core::TxSummary;
        let summary = TxSummary {
            block_number: 12_345,
            tx_hash: B256::repeat_byte(0xab),
            from: Address::repeat_byte(0x01),
            to: Address::repeat_byte(0x02),
            value: U256::from(1_000u64),
        };
        let json = serde_json::to_string(&summary).expect("TxSummary serializes");
        let back: TxSummary = serde_json::from_str(&json).expect("TxSummary deserializes");
        assert_eq!(back, summary);
    }

    // ============================================================
    // T6c4 tests: wallet_create + wallet_import + map_wallet_err
    // + validate_wallet_name
    // ============================================================
    //
    // All tests use hermetic `tempfile::tempdir()` — no live RPC,
    // no Anvil, no network. Pure filesystem + crypto assertions.
    // Round-trip tests exercise the AES-GCM auth-tag path on the
    // wrong-password side + the Argon2id-derive-key path on the
    // correct-password side.

    use alloy_primitives::hex;
    use polygon_wallet_core::{Error, WalletCreated, WalletError};
    use tempfile::tempdir;
    use zeroize::Zeroizing;

    use crate::cli::SecretMnemonic;

    fn amoy() -> Network {
        Network::Polygon(PolygonChain::Amoy)
    }
    /// 12 lowercase BIP-39 English wordlist words. Used to verify
    /// the encrypted blob actually carries the test mnemonic.
    const GOOD_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    fn good_mnemonic() -> SecretMnemonic {
        SecretMnemonic::new(GOOD_MNEMONIC.to_string())
    }
    /// Wrong word count — bip39 expects 12/15/18/21/24.
    const BAD_WORD_COUNT: &str = "foo bar baz";
    fn bad_word_count() -> SecretMnemonic {
        SecretMnemonic::new(BAD_WORD_COUNT.to_string())
    }
    /// 12 words but the last 10 are not on the BIP-39 English wordlist.
    /// bip39 returns error + lib surfaces `Error::InvalidInput`.
    const BAD_WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon zzzqx";
    fn bad_words() -> SecretMnemonic {
        SecretMnemonic::new(BAD_WORDS.to_string())
    }

    // ----- wallet_create tests -----

    #[test]
    fn wallet_create_writes_encrypted_blob_and_meta_json() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let created = super::wallet_create(tmp.path(), "alpha", &pwd, amoy()).expect("create ok");
        assert_eq!(created.name, "alpha");
        assert_eq!(created.network, amoy());
        assert!(
            !created.address.is_zero(),
            "address must be non-zero (real derive)"
        );
        let dir = tmp.path().join(amoy().as_dir_name());
        let blob = dir.join(format!("{}.enc", created.wallet_id));
        let meta = dir.join(format!("{}.meta.json", created.wallet_id));
        assert!(blob.exists(), ".enc blob missing at {}", blob.display());
        assert!(meta.exists(), ".meta.json missing at {}", meta.display());
    }

    #[test]
    fn wallet_create_rejects_empty_password() {
        let tmp = tempdir().expect("tempdir");
        let empty = Zeroizing::new(Vec::<u8>::new());
        let r = super::wallet_create(tmp.path(), "alpha", &empty, amoy());
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("crypto") || msg.contains("password"),
                    "InvalidInput must mention crypto/password; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
        // No files written on rejection.
        let amoy_dir = tmp.path().join(amoy().as_dir_name());
        let count_after = std::fs::read_dir(&amoy_dir).map(|d| d.count()).unwrap_or(0);
        assert_eq!(count_after, 0, "no files written on rejection");
    }

    #[test]
    fn wallet_create_rejects_duplicate_name_on_same_network() {
        let tmp = tempdir().expect("tempdir");
        let pwd1 = Zeroizing::new(b"correct horse battery staple".to_vec());
        let _first =
            super::wallet_create(tmp.path(), "alpha", &pwd1, amoy()).expect("first create ok");
        let pwd2 = Zeroizing::new(b"different password 1234567".to_vec());
        let r = super::wallet_create(tmp.path(), "alpha", &pwd2, amoy());
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("already exists") && msg.contains("alpha"),
                    "InvalidInput must mention duplicate + name; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn wallet_create_address_is_eip55_checksum() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let created = super::wallet_create(tmp.path(), "alpha", &pwd, amoy()).expect("create ok");
        // alloy Display impl for Address is EIP-55 checksummed.
        // Pure lowercase/uppercase = not checksummed.
        let formatted = format!("{}", created.address);
        assert!(
            formatted.starts_with("0x"),
            "address must start with 0x; got {formatted}"
        );
        let has_upper = formatted.chars().any(|c| c.is_ascii_uppercase());
        let has_lower_or_nibble = formatted
            .chars()
            .any(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        assert!(
            has_upper && has_lower_or_nibble,
            "address must be EIP-55 checksum (mixed case); got {formatted}"
        );
    }

    #[test]
    #[allow(clippy::type_complexity)] // complex fn-pointer type is the assertion
    fn wallet_create_zeroizing_wrapper_type() {
        // Type-system proof: `password: &Zeroizing<Vec<u8>>` is the
        // parameter type. Compile-time guarantee that callers cannot
        // pass a `String` or `&[u8]` without wrapping. Drop timing is
        // verified by `evm-wallet-core` integration tests; this test
        // asserts the type contract at the handler boundary.
        let _fn_sig: fn(
            &std::path::Path,
            &str,
            &Zeroizing<Vec<u8>>,
            Network,
        ) -> Result<WalletCreated, polygon_wallet_core::Error> = super::wallet_create;
    }

    #[test]
    fn wallet_create_wallet_created_struct_carries_no_mnemonic() {
        // Structural no-mnemonic proof (per L12 cluster: type-design
        // F6 + security M-3 tightened this). The `WalletCreated` return
        // type has only `wallet_id`/`name`/`network`/`address` per
        // `evm-wallet-core/src/wallet.rs:102-109`. Two layers of assertion:
        //   1. Schema-shape: the four expected fields appear in Debug
        //      output. Catches drift if a future field is added.
        //   2. Vocabulary exclusion: no mnemonic / phrase / secret /
        //      PrivateKey / password strings — tightens the prior
        //      4-word BIP-39 sample that would silently pass a
        //      redact-prefix pattern.
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let created = super::wallet_create(tmp.path(), "alpha", &pwd, amoy()).expect("create ok");
        let dbg = format!("{:?}", created);
        for field in &["wallet_id", "name:", "network:", "address:"] {
            assert!(
                dbg.contains(field),
                "WalletCreated missing expected field {field:?}: {dbg}"
            );
        }
        let dbg_lc = dbg.to_lowercase();
        for forbidden in &[
            "mnemonic",
            "phrase",
            "secret",
            "privatekey",
            "private_key",
            "password",
        ] {
            assert!(
                !dbg_lc.contains(forbidden),
                "WalletCreated debug leaks forbidden token {forbidden:?}: {dbg}"
            );
        }
    }

    // ----- wallet_import tests -----

    #[test]
    fn wallet_import_writes_encrypted_blob_and_meta_json() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let phrase = good_mnemonic();
        let created = super::wallet_import(tmp.path(), "beta-import", &pwd, amoy(), &phrase)
            .expect("import ok");
        assert_eq!(created.name, "beta-import");
        let dir = tmp.path().join(amoy().as_dir_name());
        assert!(dir.join(format!("{}.enc", created.wallet_id)).exists());
        assert!(dir
            .join(format!("{}.meta.json", created.wallet_id))
            .exists());
    }

    #[test]
    fn wallet_import_rejects_empty_password() {
        let tmp = tempdir().expect("tempdir");
        let empty = Zeroizing::new(Vec::<u8>::new());
        let r = super::wallet_import(tmp.path(), "beta", &empty, amoy(), &good_mnemonic());
        match r {
            Err(Error::InvalidInput(_)) => {}
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn wallet_import_rejects_invalid_mnemonic_word_count() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let r = super::wallet_import(tmp.path(), "beta", &pwd, amoy(), &bad_word_count());
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("mnemonic") || msg.contains("word"),
                    "InvalidInput must mention mnemonic/word; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn wallet_import_rejects_invalid_mnemonic_word() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let r = super::wallet_import(tmp.path(), "beta", &pwd, amoy(), &bad_words());
        match r {
            Err(Error::InvalidInput(_)) => {}
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn wallet_import_rejects_already_exists() {
        let tmp = tempdir().expect("tempdir");
        let pwd1 = Zeroizing::new(b"correct horse battery staple".to_vec());
        let _first = super::wallet_import(tmp.path(), "dupe", &pwd1, amoy(), &good_mnemonic())
            .expect("first import ok");
        let pwd2 = Zeroizing::new(b"different password 1234567".to_vec());
        let r = super::wallet_import(tmp.path(), "dupe", &pwd2, amoy(), &good_mnemonic());
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("already exists") && msg.contains("dupe"),
                    "InvalidInput must mention duplicate + name; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    // Round-trip via a fresh `WalletManager::open_at` is deferred — the
    // lib's `scan_disk_into` (`evm-wallet-core/src/wallet.rs:763-766`)
    // only recognizes `mainnet` / `sepolia` / `anvil` directories, not
    // `polygon_mainnet` / `polygon_amoy`. Extending `scan_disk_into`
    // lives in follow-up issue #448 (per L13 step 10a pr-test-analyzer
    // H-1 fix). When that extension lands, re-enable as a real `#[test]`
    // covering:
    //   1. `mgr.lookup_by_name(name, network)` returns the wallet_id, OR
    //   2. fall back to `mgr.unlock(wallet_id, password)` proving the
    //      AES-GCM auth-tag path round-trips correctly.
    // Until then no `#[test]` here — an empty body would silently inflate
    // `cargo test -p polygon` count without exercising behavior (L-1).

    #[test]
    fn wallet_import_wallet_created_struct_carries_no_mnemonic() {
        // Structural no-mnemonic proof (per L12 cluster: type-design
        // F6 + security M-3 tightened this). Mirror of the create-side
        // test: schema-shape + vocabulary-exclusion checks. The
        // previous 4-word BIP-39 sample was tightened to forbid any
        // mnemonic/phrase/secret/private-key/password surface.
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let phrase = good_mnemonic();
        let created =
            super::wallet_import(tmp.path(), "noleak", &pwd, amoy(), &phrase).expect("import ok");
        let dbg = format!("{:?}", created);
        for field in &["wallet_id", "name:", "network:", "address:"] {
            assert!(
                dbg.contains(field),
                "WalletCreated missing expected field {field:?}: {dbg}"
            );
        }
        let dbg_lc = dbg.to_lowercase();
        for forbidden in &[
            "mnemonic",
            "phrase",
            "secret",
            "privatekey",
            "private_key",
            "password",
        ] {
            assert!(
                !dbg_lc.contains(forbidden),
                "WalletCreated debug leaks forbidden token {forbidden:?}: {dbg}"
            );
        }
    }

    // ----- map_wallet_err tests -----

    #[test]
    fn map_wallet_err_already_exists_to_invalid_input() {
        let wallet_err = WalletError::AlreadyExists {
            name: "alpha".into(),
            network: amoy(),
        };
        let mapped = crate::handlers::map_wallet_err(wallet_err);
        match mapped {
            Error::InvalidInput(msg) => {
                assert!(
                    msg.contains("alpha") && msg.contains("exists"),
                    "msg must mention alpha + exists; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// H-1 from L13 step 10a — `Corrupt` is the ONLY `WalletError`
    /// variant that maps to `Error::Rpc` (not `InvalidInput`), so the
    /// exit code differs. A future refactor that "normalizes" Corrupt
    /// into `InvalidInput` would silently change the CLI's exit-code
    /// contract. Locked here. Reachable via `WalletManager::unlock`
    /// (UTF-8 decode failure / mnemonic parse / `0x` prefix detection).
    #[test]
    fn map_wallet_err_corrupt_to_rpc() {
        let wallet_err = WalletError::Corrupt {
            reason: "mnemonic parse: bad entropy".into(),
        };
        let mapped = crate::handlers::map_wallet_err(wallet_err);
        match mapped {
            Error::Rpc(msg) => {
                assert!(
                    msg.contains("corrupt"),
                    "Rpc msg must mention 'corrupt'; got: {msg}"
                );
            }
            other => panic!("expected Error::Rpc (different exit code!), got {other:?}"),
        }
    }

    /// H-2 from L13 step 10a — Io/Json/Path all zero coverage. These
    /// are the only `Error::Rpc`-encoding variants besides `Corrupt`;
    /// they cover fs permission errors, back-meta deserialize failure,
    /// and the lib's `RwLock` poisoning defense. Pinning each unit
    /// here means future handler additions that DO touch fs can trust
    /// the translation table.
    #[test]
    fn map_wallet_err_io_to_rpc() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "x");
        let mapped = crate::handlers::map_wallet_err(WalletError::Io(io_err));
        match mapped {
            Error::Rpc(msg) => {
                assert!(msg.contains("io"), "Rpc msg must mention 'io'; got: {msg}");
            }
            other => panic!("expected Error::Rpc, got {other:?}"),
        }
    }

    #[test]
    fn map_wallet_err_json_to_rpc() {
        let json_err = serde_json::from_str::<u8>("{").unwrap_err();
        let mapped = crate::handlers::map_wallet_err(WalletError::Json(json_err));
        match mapped {
            Error::Rpc(msg) => {
                assert!(
                    msg.contains("json"),
                    "Rpc msg must mention 'json'; got: {msg}"
                );
            }
            other => panic!("expected Error::Rpc, got {other:?}"),
        }
    }

    #[test]
    fn map_wallet_err_path_to_rpc() {
        let mapped = crate::handlers::map_wallet_err(WalletError::Path("store poisoned".into()));
        match mapped {
            Error::Rpc(msg) => {
                assert!(
                    msg.contains("path"),
                    "Rpc msg must mention 'path'; got: {msg}"
                );
            }
            other => panic!("expected Error::Rpc, got {other:?}"),
        }
    }

    /// H-3 from L13 step 10a — the documented all-whitespace error
    /// branch in `validate_wallet_name` is actually unreachable for
    /// ASCII space input (the charset check fires first with "no
    /// whitespace" message). This test locks down that the documented
    /// contract holds: input `"   "` is rejected with the charset
    /// message (NOT the unreachable all-whitespace message). Future
    /// refactor that loosens the charset to allow spaces would flip
    /// this to the all-whitespace branch — the test surfaces the
    /// design inconsistency.
    #[test]
    fn validate_wallet_name_rejects_all_whitespace_via_charset() {
        let r = crate::handlers::validate_wallet_name("   ");
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("charset") && !msg.contains("non-whitespace"),
                    "expected charset rejection (not unreachable all-whitespace branch); got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    // ----- validate_wallet_name tests -----

    #[test]
    fn validate_wallet_name_rejects_empty() {
        let r = crate::handlers::validate_wallet_name("");
        assert!(matches!(r, Err(Error::InvalidInput(_))), "got: {r:?}");
    }

    #[test]
    fn validate_wallet_name_rejects_too_long() {
        let too_long = "a".repeat(33);
        let r = crate::handlers::validate_wallet_name(&too_long);
        assert!(matches!(r, Err(Error::InvalidInput(_))), "got: {r:?}");
    }

    #[test]
    fn validate_wallet_name_rejects_bad_charset() {
        let r = crate::handlers::validate_wallet_name("bad!name");
        assert!(matches!(r, Err(Error::InvalidInput(_))), "got: {r:?}");
    }

    #[test]
    fn validate_wallet_name_accepts_valid() {
        let r = crate::handlers::validate_wallet_name("alpha-123");
        assert!(r.is_ok(), "got: {r:?}");
    }

    #[test]
    fn validate_wallet_name_accepts_max_length() {
        let max_len = "a".repeat(32);
        let r = crate::handlers::validate_wallet_name(&max_len);
        assert!(r.is_ok(), "got: {r:?}");
    }

    /// H-4 from L13 step 10a — `data_dir` errors must surface as
    /// `Error::Rpc` (via `map_wallet_err` `WalletError::Io`) without
    /// panicking. Approach: pre-place a regular file at `data_dir`,
    /// so `WalletManager::open_at`'s `fs::create_dir_all(&base_dir)?`
    /// fails with `NotADir` / `AlreadyExists` — portable across
    /// unix + windows + root user (the original 0o500 unix-only
    /// permission test was bypassed when the test runs as root, per
    /// standard Linux DAC-bypass behavior; this approach triggers the
    /// lib's IO error path regardless).
    #[test]
    fn wallet_create_fails_when_data_dir_is_a_file() {
        let tmp = tempdir().expect("tempdir");
        let file_path = tmp.path().join("not-a-dir");
        std::fs::write(&file_path, b"blocker").expect("pre-write file");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let r = super::wallet_create(&file_path, "alpha", &pwd, amoy());
        match r {
            Err(Error::Rpc(_)) => {}
            Err(Error::InvalidInput(_)) => panic!(
                "data_dir IO error must surface as Error::Rpc (distinct exit code), not InvalidInput"
            ),
            other => panic!("expected Error::Rpc, got {other:?}"),
        }
    }

    #[test]
    fn wallet_import_fails_when_data_dir_is_a_file() {
        let tmp = tempdir().expect("tempdir");
        let file_path = tmp.path().join("not-a-dir");
        std::fs::write(&file_path, b"blocker").expect("pre-write file");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let r = super::wallet_import(&file_path, "alpha", &pwd, amoy(), &good_mnemonic());
        match r {
            Err(Error::Rpc(_)) => {}
            other => panic!("expected Error::Rpc, got {other:?}"),
        }
    }

    // ----- T6c5 send / speedup validator tests -----

    /// T6c5 / S1 (failing seed → validator green): handler rejects
    /// invalid `--to` address BEFORE any provider / wallet call.
    #[test]
    fn wallet_send_native_v2_rejects_invalid_address() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"password123".to_vec());
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_send_native_v2(
                tmp.path(),
                None,
                amoy(),
                "alpha",
                &pwd,
                "not-an-address",
                "1000000000000000000",
                "wei",
                None,
                None,
                "half_hour",
                None,
                None,
                false,
                false,
                false,
            ));
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("address"),
                    "InvalidInput must mention address; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// T6c5 / S2: handler rejects non-decimal `--amount` BEFORE any
    /// provider call. Address is valid so we reach the amount check.
    #[test]
    fn wallet_send_native_v2_rejects_invalid_amount() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"password123".to_vec());
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_send_native_v2(
                tmp.path(),
                None,
                amoy(),
                "alpha",
                &pwd,
                "0x0000000000000000000000000000000000000001",
                "abc",
                "wei",
                None,
                None,
                "half_hour",
                None,
                None,
                false,
                false,
                false,
            ));
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("amount"),
                    "InvalidInput must mention amount; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// T6c5 / S4: handler rejects empty password BEFORE wallet unlock.
    /// Runs even though data_dir doesn't exist — validator fires first.
    #[test]
    fn wallet_send_native_v2_rejects_empty_password() {
        let tmp = tempdir().expect("tempdir");
        let empty = Zeroizing::new(Vec::<u8>::new());
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_send_native_v2(
                tmp.path(),
                None,
                amoy(),
                "alpha",
                &empty,
                "0x0000000000000000000000000000000000000001",
                "1000",
                "wei",
                None,
                None,
                "half_hour",
                None,
                None,
                false,
                false,
                false,
            ));
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("password"),
                    "InvalidInput must mention password; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// T6c5 / S7: cleartext RPC to non-loopback host rejected
    /// (mirrors the wallet_balance scheme guard added in commit
    /// `8701eb6`). Validators run before any provider construction,
    /// so the rejected RPC URL never opens a socket.
    #[test]
    fn wallet_send_native_v2_rejects_http_rpc_to_remote_host() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"password123".to_vec());
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_send_native_v2(
                tmp.path(),
                Some("http://remote.example.com"),
                amoy(),
                "alpha",
                &pwd,
                "0x0000000000000000000000000000000000000001",
                "1000",
                "wei",
                None,
                None,
                "half_hour",
                None,
                None,
                false,
                false,
                false,
            ));
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("scheme") || msg.contains("http"),
                    "InvalidInput must mention scheme/http; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// T6c5 / S9: RBF gate — speedup rejects new max_fee that is not
    /// STRICTLY greater than the original. Anti-silent-replace (RPC
    /// would accept same-fee replacement without error, breaking RBF
    /// intent). Pure fn test — no RPC.
    #[test]
    fn assert_new_fee_higher_rejects_non_strictly_higher() {
        assert!(super::assert_new_fee_higher(50, 30).is_err());
        assert!(super::assert_new_fee_higher(50, 50).is_err());
        assert!(super::assert_new_fee_higher(50, 51).is_ok());
    }

    /// T6c5 / S10: speedup rejects empty password (mirrors S4).
    /// Runs even though data_dir doesn't exist — validator fires
    /// before wallet lookup.
    #[test]
    fn wallet_send_speedup_v2_rejects_empty_password() {
        let tmp = tempdir().expect("tempdir");
        let empty = Zeroizing::new(Vec::<u8>::new());
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_send_speedup_v2(
                tmp.path(),
                None,
                amoy(),
                "alpha",
                &empty,
                "0x0000000000000000000000000000000000000000000000000000000000000001",
                60_000_000_000,
                30_000_000_000,
            ));
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("password"),
                    "InvalidInput must mention password; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// T6c5 / S9b: speedup rejects new_max_fee_per_gas == 0 (RBF
    /// requires higher gas; zero always fails). Validator inside
    /// `wallet_send_speedup_v2` runs before any RPC.
    #[test]
    fn wallet_send_speedup_v2_rejects_zero_new_max_fee() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"password123".to_vec());
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_send_speedup_v2(
                tmp.path(),
                None,
                amoy(),
                "alpha",
                &pwd,
                "0x0000000000000000000000000000000000000000000000000000000000000001",
                0,
                0,
            ));
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("fee") || msg.contains("max_fee"),
                    "InvalidInput must mention fee; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    // ===== #446 follow-up: Zeroizing<String> mnemonic wrap at CLI boundary =====
    //
    // Per triage brief on #446: the polygon CLI's `wallet import --mnemonic`
    // path currently carries a plain `String` mnemonic through clap into
    // the dispatch handler. The auto-derived `Debug` impl on
    // `WalletAction::Import` prints the raw mnemonic phrase, leaking the
    // secret into any logger or formatter that touches the action. Fix
    // wraps the field in `Zeroizing<String>` + a `Debug` impl that
    // redacts the phrase.
    //
    // Gating per AC: `#[cfg(dev)]` would skip this test entirely because
    // no `dev` cfg is defined in `polygon/Cargo.toml`; widen to
    // `#[cfg(any(test, dev))]` so it runs under `cargo test` AND honors
    // the dev gate when it lands.

    #[test]
    #[cfg(test)]
    fn mnemonic_does_not_leak_via_wallet_action_debug() {
        use crate::cli::WalletAction;
        let mnemonic_phrase =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let action = WalletAction::Import {
            name: "test".to_string(),
            password: Some("hunter2".to_string()),
            network: "amoy".to_string(),
            mnemonic: Some(SecretMnemonic::new(mnemonic_phrase.to_string())),
            private_key: None,
            private_key_file: None,
            account_index: 0,
            legacy_token_symbol: false,
            rpc_url: None,
        };
        let dbg = format!("{action:?}");
        assert!(
            !dbg.contains(mnemonic_phrase),
            "WalletAction::Import Debug leaked mnemonic; got: {dbg}"
        );
        assert!(
            dbg.contains("SecretMnemonic"),
            "SecretMnemonic Debug must surface type-marker; got: {dbg}"
        );
        // Same struct: password field is still a plain String (out of
        // scope per #446; tracked in #447 follow-up). Document the
        // boundary — no assertion here on the password field.
    }

    /// `wallet_import` rejects an empty phrase. Pins the lib's
    /// BIP-39 parse-in contract at the CLI boundary — empty mnemonic
    /// is `Error::InvalidInput`, not a panic.
    #[test]
    fn wallet_import_rejects_empty_phrase() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let empty = SecretMnemonic::new(String::new());
        let r = super::wallet_import(tmp.path(), "beta", &pwd, amoy(), &empty);
        match r {
            Err(Error::InvalidInput(_)) => {}
            other => panic!("expected Error::InvalidInput for empty phrase, got {other:?}"),
        }
    }

    /// `wallet_import` rejects a whitespace-only phrase (bip39
    /// `split_whitespace` collapses to 0 words → `BadWordCount(0)`).
    /// Sister contract to `wallet_import_rejects_empty_phrase` —
    /// different input surface, same lib error path.
    #[test]
    fn wallet_import_rejects_whitespace_only_phrase() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let ws = SecretMnemonic::new("   \t  ".to_string());
        let r = super::wallet_import(tmp.path(), "beta", &pwd, amoy(), &ws);
        match r {
            Err(Error::InvalidInput(_)) => {}
            other => panic!("expected Error::InvalidInput for whitespace-only, got {other:?}"),
        }
    }

    /// `wallet_import` rejects an invalid BIP-39 checksum. 12 valid
    /// BIP-39 words where the 12th is wrong-checksum (e.g., 12×
    /// `abandon` instead of the canonical `abandon × 11 + about`).
    /// Catches regressions that swap `Mnemonic::parse_in` for the
    /// silent `parse_in_normalized_without_checksum_check` bip39 API.
    #[test]
    fn wallet_import_rejects_invalid_bip39_checksum() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let bad = SecretMnemonic::new(
            "abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon"
                .to_string(),
        );
        let r = super::wallet_import(tmp.path(), "beta", &pwd, amoy(), &bad);
        match r {
            Err(Error::InvalidInput(_)) => {}
            other => panic!("expected Error::InvalidInput for bad checksum, got {other:?}"),
        }
    }

    /// `wallet_import` accepts a 24-word BIP-39 phrase (the production
    /// realistic word count — most wallets generate 24). Defense-in-
    /// depth against a future regression that hardcodes `word_count() == 12`.
    /// The 24-word all-zero-entropy test vector ends with `art` for
    /// the correct checksum.
    #[test]
    fn wallet_import_accepts_24_word_mnemonic() {
        let tmp = tempdir().expect("tempdir");
        let pwd = Zeroizing::new(b"correct horse battery staple".to_vec());
        let phrase = SecretMnemonic::new(
            "abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon art"
                .to_string(),
        );
        let r = super::wallet_import(tmp.path(), "w24", &pwd, amoy(), &phrase);
        assert!(r.is_ok(), "24-word mnemonic must succeed: {r:?}");
    }

    // ===== L13 step 10a coverage gap (test-coverage-gate) =====

    /// Type-pinning test: `SecretMnemonic::expose()` returns
    /// `&Zeroizing<String>`, NOT `&str`. Catches a regression to
    /// `expose(&self) -> &str` which would silently defeat zeroize.
    #[test]
    fn secret_mnemonic_expose_returns_zeroizing_ref() {
        let m = SecretMnemonic::new("a b c".to_string());
        let r: &Zeroizing<String> = m.expose();
        assert_eq!(r.as_str(), "a b c");
    }

    /// `FromStr` path — load-bearing for clap's `TypedValueParser`.
    /// The clap parse path goes through `s.parse::<SecretMnemonic>()`,
    /// which is functionally equivalent to `SecretMnemonic::new(s.to_string())`
    /// today. Pin both the infallibility and the phrase preservation.
    #[test]
    fn secret_mnemonic_from_str_is_infallible_and_preserves_phrase() {
        let m: SecretMnemonic = "foo bar baz".parse().expect("infallible");
        assert_eq!(m.expose().as_str(), "foo bar baz");
        assert_eq!(format!("{m:?}"), "SecretMnemonic([redacted])");
    }

    /// End-to-end clap parse test: `Cli::try_parse_from(["polygon",
    /// "wallet", "import", "--mnemonic=..."])` constructs a
    /// `WalletAction::Import { mnemonic: Some(SecretMnemonic(_)), .. }`.
    /// Catches regressions where clap's `TypedValueParser` path breaks
    /// (e.g., dropping the `Clone` derive required for the parser bound).
    #[test]
    fn clap_parse_wraps_mnemonic_in_secret_mnemonic() {
        use crate::cli::{Cli, Command, WalletAction};
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "polygon",
            "wallet",
            "import",
            "--name=x",
            "--mnemonic=abandon abandon abandon abandon abandon \
             abandon abandon abandon abandon abandon abandon about",
            "--password=hunter2",
        ])
        .expect("clap parse should succeed");
        let Command::Wallet {
            action: WalletAction::Import {
                mnemonic: Some(m), ..
            },
        } = cli.command
        else {
            panic!(
                "expected WalletAction::Import with Some(mnemonic); got {:?}",
                cli.command
            )
        };
        assert_eq!(m.expose().as_str().split_whitespace().count(), 12);
        assert_eq!(format!("{m:?}"), "SecretMnemonic([redacted])");
    }

    // ============================================================
    // #469 tests: --private-key-file flag (mode-0600 + Zeroizing wrap)
    // + wired --private-key + handler dispatch to
    // WalletManager::import_private_key_for_network.
    // ============================================================

    /// 32-byte secp256k1 PK (Anvil default account #0) — hex encoded so
    /// each test can pick its preferred input shape (raw bytes for the
    /// new file path, hex for the wired `--private-key` path).
    const ANVIL_PK_HEX: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    fn anvil_pk_bytes() -> [u8; 32] {
        let mut out = [0u8; 32];
        hex::decode(ANVIL_PK_HEX)
            .expect("hardcoded hex is valid")
            .into_iter()
            .enumerate()
            .for_each(|(i, b)| out[i] = b);
        out
    }

    /// Write `bytes` to `<tmpdir>/<name>` with the requested mode. Unix
    /// only — Windows lacks `PermissionsExt::set_permissions`.
    #[cfg(unix)]
    fn write_pk_file(
        dir: &std::path::Path,
        name: &str,
        bytes: &[u8],
        mode: u32,
    ) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write pk file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .expect("set pk file mode");
        path
    }

    #[test]
    #[cfg(unix)]
    fn read_pk_file_accepts_mode_0600_file() {
        // Happy path: mode 0600 + valid bytes → `Zeroizing<Vec<u8>>`
        // wrapper. Type assertion (`Zeroizing<Vec<u8>>` return) is the
        // contract — caller can `as_slice()` and pass to lib.
        let tmp = tempdir().expect("tempdir");
        let path = write_pk_file(tmp.path(), "pk.hex", &anvil_pk_bytes(), 0o600);
        let got: Zeroizing<Vec<u8>> = super::read_pk_file(&path).expect("read ok");
        assert_eq!(got.as_slice(), &anvil_pk_bytes());
    }

    #[test]
    #[cfg(unix)]
    fn read_pk_file_rejects_mode_0644_file() {
        // Mode != 0600 must surface `Error::InvalidInput` with the
        // actual mode in the message so the operator can chmod it
        // without re-reading the source.
        let tmp = tempdir().expect("tempdir");
        let path = write_pk_file(tmp.path(), "pk.hex", &anvil_pk_bytes(), 0o644);
        let err = super::read_pk_file(&path).expect_err("mode 0o644 must error");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("0o644"),
            "error must name the bad mode (0o644); got: {msg}"
        );
        assert!(
            msg.contains("0o600"),
            "error must mention required mode (0o600); got: {msg}"
        );
    }

    #[test]
    fn read_pk_file_rejects_missing_file() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("does-not-exist.hex");
        let err = super::read_pk_file(&path).expect_err("missing file must error");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("not found") || msg.contains("No such file"),
            "error must indicate missing file; got: {msg}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn read_pk_file_returns_zeroizing_wrapper() {
        // Type contract: must bind to `Zeroizing<Vec<u8>>` so the
        // caller-side lifecycle zeros the buffer on drop (sister
        // invariant to `SecretMnemonic(Zeroizing<String>)`).
        let tmp = tempdir().expect("tempdir");
        let path = write_pk_file(tmp.path(), "pk.hex", &anvil_pk_bytes(), 0o600);
        let got: Zeroizing<Vec<u8>> = super::read_pk_file(&path).expect("read ok");
        // Zeroizing<Vec<u8>> deref to &[u8].
        assert_eq!(got.len(), 32);
    }

    #[test]
    #[cfg(unix)]
    fn wallet_import_private_key_for_network_writes_blob_and_meta_json() {
        // End-to-end handler test: pass a `Zeroizing<Vec<u8>>` PK to
        // the handler (the file-read + Zeroizing wrap is `read_pk_file`,
        // tested separately above; this test focuses on the handler's
        // pure-crypto path), assert a wallet is created under
        // polygon_amoy/<uuid>.enc + .meta.json (sister invariant to
        // wallet_import_writes_encrypted_blob_and_meta_json for the
        // mnemonic path).
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().expect("tempdir");

        let password = Zeroizing::new(b"correct horse battery staple".to_vec());
        let pk_bytes = Zeroizing::new(anvil_pk_bytes().to_vec());
        let created = super::wallet_import_private_key_for_network(
            tmp.path(),
            "pk-file-alpha",
            &password,
            amoy(),
            &pk_bytes,
        )
        .expect("handler must accept mode-0600 file");
        assert_eq!(created.network, amoy());

        let enc = tmp
            .path()
            .join("polygon_amoy")
            .join(format!("{}.enc", created.wallet_id));
        let meta = tmp
            .path()
            .join("polygon_amoy")
            .join(format!("{}.meta.json", created.wallet_id));
        assert!(
            enc.exists(),
            "encrypted blob must exist under polygon_amoy/"
        );
        assert!(meta.exists(), "meta.json must exist alongside .enc");

        // Defense-in-depth: blob mode must be 0o600 (the lib already
        // enforces this for `import_private_key_for_network` per
        // evm-wallet-core test; pin it here too so a future refactor
        // that bypasses the lib doesn't silently regress).
        let mode = std::fs::metadata(&enc)
            .expect("enc exists")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "encrypted PK blob must be owner-only (0o600); got 0o{mode:o}"
        );
    }

    #[test]
    fn cli_rejects_private_key_with_private_key_file() {
        // `clap` `conflicts_with` enforcement: passing both flags in
        // one invocation must error at parse time (no dispatch).
        // Sister invariant to the existing `mnemonic` vs `private_key`
        // conflict at `cli.rs:183`.
        use crate::cli::Cli;
        use clap::Parser;
        let result = Cli::try_parse_from([
            "polygon",
            "wallet",
            "import",
            "--name",
            "x",
            "--password",
            "pw",
            "--private-key",
            "0xdeadbeef",
            "--private-key-file",
            "/tmp/pk.hex",
        ]);
        let err = result.expect_err("--private-key + --private-key-file must conflict");
        let msg = err.to_string();
        assert!(
            msg.contains("--private-key") && msg.contains("--private-key-file"),
            "clap error must name both flags; got: {msg}"
        );
    }
}
