//! Per-subcommand handler fns for the `eth` CLI.
//!
//! `main.rs::run` parses CLI args via clap and dispatches each subcommand to
//! a fn in this module. Handlers return `Result<(), eth_wallet_core::Error>`
//! so all error→exit-code mapping flows through `Error::exit_code()` per
//! #297 M11 (the stable 0..=5 exit code table).
//!
//! Handlers that need RPC use `eth_wallet_core::new_http(url)` so the
//! library façade decides whether to use pinned vs default-TLS transport
//! (per #316 — currently default rustls TLS; SPKI pinning deferred per #330).
//!
//! ## Scope (per #337, split 2 sub-PRs)
//!
//! PR-A (this commit): wallet create/import/list/show/delete + wallet
//!   balance + tx get. These are sync or RPC-read-only — no signing, no
//!   broadcast. All Anvil-backed via spawned `alloy-node-bindings::Anvil`.
//!
//! PR-B (deferred): wallet send-native + wallet send-erc20 + tx list. These
//! need sign+broadcast+tx-history scan. Lives in a follow-up PR per L25.

use std::path::PathBuf;
use std::str::FromStr;

use alloy_network::Ethereum;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use alloy_transport_http::reqwest::Url;

use eth_wallet_core::error::{Error, Result};
use eth_wallet_core::wallet::{Network, WalletError, WalletManager};
use eth_wallet_core::{new_http, WalletCreated};

// ---------------------------------------------------------------------------
// Manager + provider helpers
// ---------------------------------------------------------------------------

/// Open a `WalletManager` honoring the CLI's `--data-dir` flag. Falls back
/// to the XDG default (`WalletManager::open()`) when `data_dir` is None.
pub fn open_manager(data_dir: Option<&PathBuf>) -> Result<WalletManager> {
    match data_dir {
        Some(p) => WalletManager::open_at(p.clone()).map_err(map_wallet_err),
        None => WalletManager::open().map_err(map_wallet_err),
    }
}

/// Open an RPC `RootProvider<Ethereum>` against `rpc_url`. Returns
/// `Error::InvalidInput` if the URL doesn't parse.
pub fn open_provider(rpc_url: &str) -> Result<RootProvider<Ethereum>> {
    let url: Url = rpc_url
        .parse()
        .map_err(|e| Error::InvalidInput(format!("rpc-url parse: {e}")))?;
    new_http(url)
}

/// Map `WalletError` → canonical `Error` so the CLI's `exit_code()` table
/// still applies. Category mapping per #297 M11:
/// - Io/Path → Rpc (3)
/// - Json/Corrupt → WalletCorrupt (5)
/// - Crypto/Mnemonic → InvalidMnemonic (2)
/// - PrivateKey → InvalidPrivateKey (2)
/// - NotFound → WalletNotFound (4)
/// - NotFoundByName → WalletNotFound + tracing event with the name
/// - AlreadyExists → WalletExists (4)
pub(crate) fn map_wallet_err(e: WalletError) -> Error {
    match e {
        WalletError::Io(_) | WalletError::Path(_) => Error::Rpc(format!("wallet: {e}")),
        WalletError::Json(_) | WalletError::Corrupt { .. } => Error::WalletCorrupt {
            path: "<unknown>".to_string(),
            reason: e.to_string(),
        },
        WalletError::Crypto(_) => Error::DecryptionFailed(e.to_string()),
        WalletError::Mnemonic(_) => Error::InvalidMnemonic(e.to_string()),
        WalletError::PrivateKey(_) => Error::InvalidPrivateKey(e.to_string()),
        WalletError::NotFound { wallet_id } => Error::WalletNotFound { wallet_id },
        WalletError::NotFoundByName { name, network } => Error::WalletNotFoundByName {
            name,
            network: format!("{network:?}").to_lowercase(),
        },
        WalletError::AlreadyExists { name, network } => Error::WalletExists {
            name,
            network: format!("{network:?}").to_lowercase(),
        },
    }
}

/// Validate a user-supplied wallet name against the L12 review M-3 rule:
/// 1..=32 chars, charset `[A-Za-z0-9 _-]`. Hand-rolled byte check (no
/// regex crate) — short, allocation-free on the happy path. Returns
/// `Error::InvalidInput` (exit 2) on violation so the operator sees the
/// exact failure reason in stderr.
pub(crate) fn validate_wallet_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidInput("wallet name must not be empty".into()));
    }
    if name.len() > 32 {
        return Err(Error::InvalidInput(format!(
            "wallet name must be 1..=32 chars, got {}",
            name.len()
        )));
    }
    for b in name.bytes() {
        if !matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b' ' | b'_' | b'-') {
            return Err(Error::InvalidInput(format!(
                "wallet name contains invalid char: {name:?} (allowed: A-Z a-z 0-9 space _ -)"
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Wallet create / import / list / show / delete
// ---------------------------------------------------------------------------

pub fn wallet_create(
    mgr: &WalletManager,
    name: &str,
    password: &str,
    network_str: &str,
) -> Result<WalletCreated> {
    validate_wallet_name(name)?;
    if password.is_empty() {
        return Err(Error::InvalidPassword("password must be non-empty".into()));
    }
    let network =
        Network::parse_cli(network_str).map_err(|e| Error::InvalidInput(e.to_string()))?;
    mgr.create_wallet_for_network(name, password.as_bytes(), network)
        .map_err(map_wallet_err)
}

pub fn wallet_import(
    mgr: &WalletManager,
    name: &str,
    password: &str,
    network_str: &str,
    mnemonic: Option<&str>,
    private_key: Option<&str>,
) -> Result<WalletCreated> {
    validate_wallet_name(name)?;
    if password.is_empty() {
        return Err(Error::InvalidPassword("password must be non-empty".into()));
    }
    match (mnemonic, private_key) {
        (Some(phrase), None) => {
            let network =
                Network::parse_cli(network_str).map_err(|e| Error::InvalidInput(e.to_string()))?;
            mgr.import_wallet_for_network(name, phrase, password.as_bytes(), network)
                .map_err(map_wallet_err)
        }
        (None, Some(pk)) => {
            // `import_private_key` doesn't take a network (always Sepolia).
            // Drift: CLI's `--network` flag is ignored when `--private-key`
            // is provided. PR-B can layer `import_private_key_for_network`.
            mgr.import_private_key(name, pk, password.as_bytes())
                .map_err(map_wallet_err)
        }
        (Some(_), Some(_)) | (None, None) => Err(Error::InvalidInput(
            "exactly one of --mnemonic or --private-key must be provided".into(),
        )),
    }
}

/// Print a one-line-per-wallet summary table to stdout.
pub fn wallet_list(mgr: &WalletManager) -> Result<()> {
    let infos = mgr.list_wallets().map_err(map_wallet_err)?;
    if infos.is_empty() {
        println!("(no wallets — use `eth wallet create` to start)");
        return Ok(());
    }
    println!(
        "{:<36}  {:<24}  {:<8}  {:<44}  DERIVATION",
        "WALLET_ID", "NAME", "NETWORK", "ADDRESS",
    );
    for w in infos {
        println!(
            "{:<36}  {:<24}  {:<8}  0x{:<42}  {}",
            w.wallet_id,
            w.name,
            format!("{:?}", w.network).to_lowercase(),
            format!("{:x}", w.address),
            w.derivation_path,
        );
    }
    Ok(())
}

/// Resolve a wallet by `--name` or `--id` (exactly one required) and print
/// its metadata. Network defaults to Sepolia when not supplied via the
/// caller — pass through from `wallet show --network` flag.
pub fn wallet_show(
    mgr: &WalletManager,
    name: Option<&str>,
    id: Option<&str>,
    network: &str,
) -> Result<()> {
    let net = Network::parse_cli(network).map_err(|e| Error::InvalidInput(e.to_string()))?;
    let infos = mgr.list_wallets().map_err(map_wallet_err)?;

    let info =
        match (name, id) {
            (Some(n), None) => infos.into_iter().find(|w| w.name == n).ok_or_else(|| {
                Error::WalletNotFoundByName {
                    name: n.to_string(),
                    network: format!("{net:?}").to_lowercase(),
                }
            })?,
            (None, Some(s)) => {
                let uuid = uuid::Uuid::parse_str(s)
                    .map_err(|e| Error::InvalidInput(format!("invalid wallet id: {e}")))?;
                infos
                    .into_iter()
                    .find(|w| w.wallet_id == uuid)
                    .ok_or(Error::InvalidInput("wallet id not found".into()))?
            }
            _ => {
                return Err(Error::InvalidInput(
                    "exactly one of --name or --id required".into(),
                ));
            }
        };
    if info.network != net {
        return Err(Error::InvalidInput(format!(
            "wallet '{}' lives on {:?}, not {:?}",
            info.name, info.network, net
        )));
    }
    println!("wallet_id:        {}", info.wallet_id);
    println!("name:             {}", info.name);
    println!("network:          {:?}", info.network);
    println!("address:          0x{:x}", info.address);
    println!("derivation_path:  {}", info.derivation_path);
    Ok(())
}

pub fn wallet_delete(
    mgr: &WalletManager,
    name: Option<&str>,
    id: Option<&str>,
    network: &str,
) -> Result<()> {
    let net = Network::parse_cli(network).map_err(|e| Error::InvalidInput(e.to_string()))?;
    let wallet_id = match (name, id) {
        (Some(n), None) => mgr.lookup_by_name(n, net).map_err(map_wallet_err)?,
        (None, Some(s)) => uuid::Uuid::parse_str(s)
            .map_err(|e| Error::InvalidInput(format!("invalid wallet id: {e}")))?,
        _ => {
            return Err(Error::InvalidInput(
                "exactly one of --name or --id required".into(),
            ));
        }
    };
    mgr.delete_wallet(wallet_id).map_err(map_wallet_err)?;
    println!("deleted wallet: {}", wallet_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// RPC handlers (Anvil provider)
// ---------------------------------------------------------------------------

/// Print the ETH balance of `address` in the requested unit. Default unit
/// is ETH (18 decimals). Caller provides the provider so PR-A can reuse
/// this fn for both Anvil (dev) and Sepolia (testnet).
///
/// When `token` is `Some`, print the ERC-20 `balanceOf(address)` for the
/// token contract instead — auto-detect decimals via
/// `erc20::query_decimals` unless `decimals_override` is supplied (Issue #356).
pub async fn wallet_balance(
    provider: &RootProvider<Ethereum>,
    address: &str,
    unit: Option<&str>,
    token: Option<&str>,
    decimals_override: Option<u8>,
    network: &str,
) -> Result<()> {
    let holder = Address::from_str(address)
        .map_err(|e| Error::InvalidInput(format!("invalid address: {e}")))?;
    match token {
        None => {
            let balance_wei = provider
                .get_balance(holder)
                .await
                .map_err(|e| Error::Rpc(format!("get_balance: {e}")))?;
            let unit = unit.unwrap_or("eth");
            let formatted = match unit.to_ascii_lowercase().as_str() {
                "wei" => format!("{} wei", balance_wei),
                "gwei" => format!("{} gwei", format_wei_as(balance_wei, 9)),
                "eth" => format!("{} ETH", format_wei_as(balance_wei, 18)),
                other => return Err(Error::InvalidInput(format!("unknown unit '{other}'"))),
            };
            println!("{formatted}");
            Ok(())
        }
        Some(token_str) => {
            let token_addr = Address::from_str(token_str)
                .map_err(|e| Error::InvalidInput(format!("invalid --token address: {e}")))?;
            // Resolve human-readable label + cached decimals (Issue #360 +
            // Issue #366). Order:
            //   1. bundled registry short-circuit (skip RPC for both symbol
            //      and decimals when the token is known);
            //   2. otherwise: decimals via `query_decimals` RPC, symbol via
            //      `query_symbol` RPC;
            //   3. symbol fallback to lowercase address on RPC failure;
            //      decimals error bubbles as `Error::AbiDecodeFailed` / `Rpc`
            //      so the lock-down tests added in PR #362 (Issue #366)
            //      still classify non-ERC-20 targets as exit 2 / 3.
            //
            // The network string from `--network` drives registry lookup;
            // unknown networks → Anvil chain_id 31337, no registry entries,
            // so both queries run and the RPC answer wins. The registry hit
            // also short-circuits `query_decimals` — Sepolia USDC is a
            // proxy whose `decimals()` reverts on the Anvil fork, so the
            // bundled `decimals=6` is the only reliable answer.
            let meta = resolve_token_metadata(provider, network, token_addr).await;
            let label = meta.label;
            // Precedence: explicit CLI `--decimals` > registry cache > RPC.
            let decimals = match decimals_override {
                Some(d) => d,
                None => match meta.decimals {
                    Some(d) => d,
                    None => eth_wallet_core::erc20::query_decimals(provider, token_addr).await?,
                },
            };
            let raw = eth_wallet_core::erc20::token_balance(provider, token_addr, holder).await?;
            // Token balance prints `<symbol> <scaled>` — `<symbol>` is the
            // human-readable label (or the lowercase address on fallback).
            // The `--unit` flag is rejected by clap when `--token` is set
            // (`conflicts_with`), so this branch never sees a `unit` hint.
            println!("{} {}", label, format_wei_as(raw, decimals));
            Ok(())
        }
    }
}

/// Iterate the bundled token registry for `network` + any `--token`
/// overrides, printing one line per token (Issue #358). Output format:
/// `<symbol> <scaled_balance> <token-addr>` (text mode) or a JSON array of
/// `{symbol, address, balance, decimals}` rows (`--json`).
///
/// Failure isolation (AC #4): per-token `token_balance` (and per-token
/// `query_decimals` for non-registry overrides) failures are logged to
/// stderr + skipped. When at least one token succeeded → exit 0. When ALL
/// tokens failed (e.g. unreachable RPC) → return the first error so the
/// dominant failure category wins (AC #7: exit 3 + stderr carries
/// `balanceOf` from the first `erc20::token_balance` call).
///
/// `--decimals <N>` (AC #6): when set, applies to every token in the batch
/// and skips per-token `decimals()` auto-detect. When unset, registry
/// entries use their cached `decimals`; non-registry overrides RPC-query.
pub async fn wallet_balance_all(
    provider: &RootProvider<Ethereum>,
    address: &str,
    network: &str,
    token_overrides: &[String],
    decimals_override: Option<u8>,
    json: bool,
) -> Result<()> {
    let holder = Address::from_str(address)
        .map_err(|e| Error::InvalidInput(format!("invalid address: {e}")))?;
    let chain_id = match eth_wallet_core::Network::parse_cli(network) {
        Ok(n) => n.chain_id(),
        // Unknown CLI network (e.g. local Anvil chain_id 31337) — registry
        // lookup is per-chain, so it returns the empty stub and the
        // token_overrides become the only entries.
        Err(_) => 31337,
    };

    /// Per-token entry built from `load_chain` + user-supplied `--token`.
    struct Entry {
        label: String,
        addr: Address,
        /// Cached decimals from the bundled registry. `None` means the
        /// address was not in the registry — caller will fall through to
        /// `query_decimals` RPC (or `--decimals` override).
        registry_decimals: Option<u8>,
    }
    let mut entries: Vec<Entry> = Vec::new();
    let registry = eth_wallet_core::load_chain(chain_id)?;
    for t in &registry {
        entries.push(Entry {
            label: t.symbol.clone(),
            addr: t.address,
            registry_decimals: Some(t.decimals),
        });
    }
    // AC #2: user-supplied --token overrides appended AFTER the registry
    // entries, in CLI order. Same address may appear twice (dedup is the
    // caller's responsibility; we iterate as-supplied to preserve order).
    for addr_str in token_overrides {
        let addr = Address::from_str(addr_str)
            .map_err(|e| Error::InvalidInput(format!("invalid --token address: {e}")))?;
        let (label, cached) = match eth_wallet_core::lookup_by_address(chain_id, addr) {
            Ok(Some(t)) => (t.symbol, Some(t.decimals)),
            _ => (format!("{addr:#x}"), None),
        };
        entries.push(Entry {
            label,
            addr,
            registry_decimals: cached,
        });
    }
    if entries.is_empty() {
        return Err(Error::InvalidInput(format!(
            "no tokens for chain_id={chain_id} (--all requires at least one bundled entry or --token)"
        )));
    }

    let mut json_rows: Vec<serde_json::Value> = Vec::new();
    let mut first_err: Option<Error> = None;
    let mut succeeded_any = false;

    for entry in &entries {
        // AC #6: --decimals override wins for every token.
        // Otherwise: registry cache → per-token `query_decimals` RPC.
        let decimals = match decimals_override {
            Some(d) => d,
            None => match entry.registry_decimals {
                Some(d) => d,
                None => match eth_wallet_core::erc20::query_decimals(provider, entry.addr).await {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("error: decimals for {} ({}): {e}", entry.label, entry.addr);
                        if first_err.is_none() {
                            first_err = Some(e);
                        }
                        continue;
                    }
                },
            },
        };
        match eth_wallet_core::erc20::token_balance(provider, entry.addr, holder).await {
            Ok(raw) => {
                let formatted = format_wei_as(raw, decimals);
                if json {
                    json_rows.push(serde_json::json!({
                        "symbol": entry.label,
                        // `Display` for `alloy_primitives::Address` writes
                        // EIP-55 checksum; `{:#x}` calls `LowerHex` (lowercase)
                        // which would break the AC #1 contract + any
                        // operator piping through `jq` + checksum tools.
                        "address": format!("{}", entry.addr),
                        "balance": formatted,
                        "decimals": decimals,
                    }));
                } else {
                    println!("{} {} {}", entry.label, formatted, entry.addr);
                }
                succeeded_any = true;
            }
            Err(e) => {
                eprintln!("error: balance for {} ({}): {e}", entry.label, entry.addr);
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_rows)
                .expect("serialize Vec<serde_json::Value> (constructed above)")
        );
    }

    if succeeded_any {
        Ok(())
    } else {
        // AC #7 + #4: when all per-token calls failed (unreachable RPC,
        // non-ERC-20 target, etc.), return the first error. Per-token
        // failures were already logged to stderr above so the operator
        // can see which subset failed.
        Err(first_err.expect("entries non-empty + all failed → first_err set"))
    }
}

/// Resolved ERC-20 metadata for `wallet balance --token` output (Issue #360 +
/// Issue #366).
///
/// - `label`: human-readable symbol from registry short-circuit, RPC
///   `query_symbol`, or lowercase address fallback (in that order). Any
///   registry/RPC error downgrades silently to the address fallback so
///   the balance line still prints.
/// - `decimals`: registry-cached `Token.decimals` when the registry hits,
///   otherwise `None` so the caller falls through to `query_decimals` RPC.
struct TokenMetadata {
    label: String,
    decimals: Option<u8>,
}

/// Resolve the human-readable label + cached decimals for an ERC-20 token
/// (Issue #360 + Issue #366). Order: registry short-circuit → `query_symbol`
/// RPC → fallback to lowercase address. `network` is the CLI `--network`
/// string (matches `Network::parse_cli`). Any registry/RPC error downgrades
/// to the address fallback — never propagates — so the balance line still
/// prints even when the node is unreachable or the chain has no bundled
/// registry.
async fn resolve_token_metadata(
    provider: &RootProvider<Ethereum>,
    network: &str,
    token_addr: Address,
) -> TokenMetadata {
    let chain_id = match eth_wallet_core::Network::parse_cli(network) {
        Ok(n) => n.chain_id(),
        // Unknown CLI network (e.g. local Anvil) → chain_id 31337. The
        // Anvil registry is the empty stub, so lookup misses, and the
        // call falls through to the RPC path.
        Err(_) => 31337,
    };
    if let Ok(Some(t)) = eth_wallet_core::lookup_by_address(chain_id, token_addr) {
        return TokenMetadata {
            label: t.symbol,
            decimals: Some(t.decimals),
        };
    }
    let label = match eth_wallet_core::erc20::query_symbol(provider, token_addr).await {
        Ok(sym) => sym,
        // RPC unreachable, contract reverts, or non-ERC-20 target. Honest
        // fallback: print the lowercase address so the operator still has
        // a usable line.
        Err(_) => format!("{token_addr:#x}"),
    };
    TokenMetadata {
        label,
        decimals: None,
    }
}

/// Look up a transaction by hash. Returns `Error::Rpc("transaction not
/// found: ...")` (exit 3) when the node has no record.
pub async fn tx_get(provider: &RootProvider<Ethereum>, tx_hash: &str) -> Result<()> {
    let hash = B256::from_str(tx_hash)
        .map_err(|e| Error::InvalidInput(format!("invalid tx hash: {e}")))?;
    let tx = provider
        .get_transaction_by_hash(hash)
        .await
        .map_err(|e| Error::Rpc(format!("get_transaction_by_hash: {e}")))?
        .ok_or_else(|| Error::Rpc(format!("transaction not found: {tx_hash}")))?;
    // Debug-print: alloy's `Transaction<T>` shape changes between 0.x
    // and 1.x; `{:#?}` avoids hand-rolled field breakage. PR-B can switch
    // to a typed formatter once the eth-cli output schema is reviewed.
    println!("{tx:#?}");
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Redact RPC URL substrings from an upstream alloy error before
/// formatting it into a user-visible `Error::Rpc` string. H-6 finding
/// from L12 security review: alloy's transport error `Display` embeds
/// the full RPC URL, which an attacker-controlled node could weaponize
/// for log poisoning or operator phishing.
///
/// Local wrapper preserved as `redact_rpc_error` so existing
/// callsites (`send_raw_transaction`, etc.) read naturally; delegates
/// to the lib-level `eth_wallet_core::redact_rpc_url` (promoted from
/// this file by Issue #356 security-sweep follow-up).
fn redact_rpc_error(e: impl std::fmt::Display) -> String {
    eth_wallet_core::redact_rpc_url(e)
}

/// Format a `U256` wei amount as `<whole>.<frac>` with `decimals` fractional
/// digits (zero-padded). Returns just `<whole>` when the fractional part
/// is zero.
fn format_wei_as(wei: U256, decimals: u8) -> String {
    if decimals == 0 {
        return wei.to_string();
    }
    let div = U256::from(10u128.pow(u32::from(decimals)));
    let whole = wei / div;
    let frac = wei % div;
    if frac.is_zero() {
        whole.to_string()
    } else {
        let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
        format!("{whole}.{frac_str}")
    }
}

// ---------------------------------------------------------------------------
// PR-B deferred handlers (compile-time stubs)
// ---------------------------------------------------------------------------

/// Send a native ETH transaction. Cycle 3 (#339 PR-B): sign EIP-1559
/// envelope locally, then broadcast via `send_raw_transaction`. Cycle 16
/// (#354): gas resolved via `resolve_gas` — explicit CLI/env overrides win
/// over `provider.estimate_eip1559_fees()` (default for live testnets).
pub async fn wallet_send_native(
    provider: &RootProvider<Ethereum>,
    signer: &PrivateKeySigner,
    wallet_network: eth_wallet_core::Network,
    to: Address,
    amount_wei: U256,
    max_fee_per_gas: Option<u128>,
    max_priority_fee_per_gas: Option<u128>,
) -> Result<()> {
    let from = signer.address();
    // Step 1: validate overrides (pure, no RPC) — fail fast on partial
    // override OR fee-ordering violation BEFORE any network call. This
    // preserves the L28 Gate C "partial override → exit 2 without RPC"
    // contract that the prior round-1 reorder broke.
    let overrides = resolve_overrides(max_fee_per_gas, max_priority_fee_per_gas)?;
    // Step 2: chain_id trust-boundary check (per L12 security L-1 + L-4):
    // an attacker-controlled RPC must not return gas values we sign
    // against a legitimate chain.
    let provider_chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| Error::Rpc(format!("get_chain_id: {}", redact_rpc_error(&e))))?;
    let expected_chain_id = wallet_network.chain_id();
    if provider_chain_id != expected_chain_id {
        return Err(Error::InvalidInput(format!(
            "rpc chain_id {provider_chain_id} does not match wallet network {wallet_network:?} (expected {expected_chain_id})"
        )));
    }
    // Step 3: resolve gas (estimate only if overrides were None).
    let resolved = resolve_gas(provider, overrides).await?;
    let nonce_val = provider
        .get_transaction_count(from)
        .await
        .map_err(|e| Error::Rpc(format!("get_transaction_count: {}", redact_rpc_error(&e))))?;

    let tx_req = TransactionRequest {
        from: Some(from),
        to: Some(alloy_primitives::TxKind::Call(to)),
        value: Some(amount_wei),
        chain_id: Some(provider_chain_id),
        nonce: Some(nonce_val),
        gas: Some(21000u64),
        max_fee_per_gas: Some(resolved.max_fee_per_gas),
        max_priority_fee_per_gas: Some(resolved.max_priority_fee_per_gas),
        ..Default::default()
    };

    let signed = eth_wallet_core::sign_native_eth_tx(signer, tx_req)
        .map_err(|e| Error::Rpc(format!("sign: {e}")))?;
    let bytes = eth_wallet_core::encoded_envelope(&signed);
    let pending = provider
        .send_raw_transaction(&bytes)
        .await
        .map_err(|e| Error::Rpc(format!("send_raw_transaction: {}", redact_rpc_error(&e))))?;
    println!("{}", pending.tx_hash());
    Ok(())
}

/// Send an ERC-20 transfer. Cycle 4 (#339 PR-B): sign + broadcast via
/// `send_raw_transaction`. Cycle 16 (#354): gas resolved via `resolve_gas`
/// — same override/estimate precedence as native send.
///
/// `#[allow(clippy::too_many_arguments)]`: EIP-1559 + ERC-20 + network +
/// signer naturally need 9 args; grouping into structs adds noise without
/// value (the call site is single-thread dispatch from main.rs).
#[allow(clippy::too_many_arguments)]
pub async fn wallet_send_erc20(
    provider: &RootProvider<Ethereum>,
    signer: &PrivateKeySigner,
    wallet_network: eth_wallet_core::Network,
    token: Address,
    to: Address,
    amount_wei: U256,
    gas_limit: u64,
    max_fee_per_gas: Option<u128>,
    max_priority_fee_per_gas: Option<u128>,
) -> Result<()> {
    let from = signer.address();
    // Step 1: validate overrides (pure, no RPC).
    let overrides = resolve_overrides(max_fee_per_gas, max_priority_fee_per_gas)?;
    // Step 2: chain_id trust-boundary check (per L12 security L-1 + L-4).
    let provider_chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| Error::Rpc(format!("get_chain_id: {}", redact_rpc_error(&e))))?;
    let expected_chain_id = wallet_network.chain_id();
    if provider_chain_id != expected_chain_id {
        return Err(Error::InvalidInput(format!(
            "rpc chain_id {provider_chain_id} does not match wallet network {wallet_network:?} (expected {expected_chain_id})"
        )));
    }
    // Step 3: resolve gas (estimate only if overrides were None).
    let resolved = resolve_gas(provider, overrides).await?;
    let nonce_val = provider
        .get_transaction_count(from)
        .await
        .map_err(|e| Error::Rpc(format!("get_transaction_count: {}", redact_rpc_error(&e))))?;

    let calldata = eth_wallet_core::erc20::transfer_calldata(to, amount_wei);
    let signed = eth_wallet_core::sign_erc20_tx_bytes(
        signer,
        token,
        calldata,
        U256::ZERO, // ERC-20 transfer sends 0 native ETH
        nonce_val,
        provider_chain_id,
        resolved.max_fee_per_gas,
        resolved.max_priority_fee_per_gas,
        gas_limit,
    )
    .map_err(|e| Error::Rpc(format!("sign-erc20: {}", redact_rpc_error(&e))))?;
    let bytes = eth_wallet_core::encoded_envelope(&signed);
    let pending = provider.send_raw_transaction(&bytes).await.map_err(|e| {
        Error::Rpc(format!(
            "send_raw_transaction (erc20): {}",
            redact_rpc_error(&e)
        ))
    })?;
    println!("{}", pending.tx_hash());
    Ok(())
}

/// List recent transactions on the chain. Cycle 5 (#339 PR-B): minimal
/// get_block_number scan — proves the path is wired. Cycle 6 replaces
/// with `provider.get_logs(Filter)` address-scoped scan + topic decode.
pub async fn tx_list(provider: &RootProvider<Ethereum>, limit: u32) -> Result<()> {
    let block = provider
        .get_block_number()
        .await
        .map_err(|e| Error::Rpc(format!("get_block_number: {e}")))?;
    println!("latest_block={block} limit={limit}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Pretty printer for WalletCreated (used by wallet_create + wallet_import)
// ---------------------------------------------------------------------------

pub fn print_wallet_created(w: &WalletCreated) {
    println!("wallet_id:  {}", w.wallet_id);
    println!("name:       {}", w.name);
    println!("network:    {:?}", w.network);
    println!("address:    0x{:x}", w.address);
    println!(
        "# NOTE: mnemonic is NOT shown for safety. Back up via the recovery\n\
         # phrase written to your secret manager before closing this session."
    );
}

// ---------------------------------------------------------------------------
// Issue #341 — `eth config show` handler. Prints the effective resolved
// configuration (network, chain_id, rpc_url, data_dir, gas_limit) so
// operators can audit which env / .env / flag is active without reading
// source. Resolution precedence per KTD1 (session-settled): explicit flag
// > env var > .env file > centralised default. We read process env here;
// clap has already populated it from `.env` via `dotenvy::dotenv()` at
// `main()` startup.
//
// Lazy-validation posture per KTD2 (session-settled): we do NOT ping the
// RPC. The handler prints whatever is in env. Errors surface at use-site
// (the actual subcommand that needs the value).
// ---------------------------------------------------------------------------

pub fn config_show(rpc_url: &str, data_dir: Option<&PathBuf>, json: bool) -> Result<()> {
    // Network: read ETH_NETWORK from env (clap populated from dotenvy at
    // startup). When unset, print "(unset)" — do NOT silently default to
    // "sepolia" (review finding #1: misconfig hides behind defaults).
    let network_raw = std::env::var("ETH_NETWORK").ok();
    let (network_str, chain_id_str) = match network_raw.as_deref() {
        None => ("(unset)".to_string(), "(unset)".to_string()),
        Some(s) => {
            let net = Network::parse_cli(s).map_err(|e| {
                Error::InvalidInput(format!("config-show: invalid ETH_NETWORK={s:?}: {e}"))
            })?;
            (
                format!("{net:?}").to_lowercase(),
                net.chain_id().to_string(),
            )
        }
    };

    // Gas limit: ETH_GAS_LIMIT env (diagnostic visibility; send
    // subcommands have their own per-call default of 65000).
    let gas_limit_str = std::env::var("ETH_GAS_LIMIT").unwrap_or_else(|_| "(unset)".into());

    let data_dir_str = data_dir
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(xdg default)".into());

    if json {
        let payload = serde_json::json!({
            "network":   network_str,
            "chain_id":  chain_id_str,
            "rpc_url":   rpc_url,
            "data_dir":  data_dir_str,
            "gas_limit": gas_limit_str,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|e| Error::InvalidInput(format!("config-show json: {e}")))?
        );
    } else {
        println!("network:    {network_str}");
        println!("chain_id:   {chain_id_str}");
        println!("rpc_url:    {rpc_url}");
        println!("data_dir:   {data_dir_str}");
        println!("gas_limit:  {gas_limit_str}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Gas override resolution (Issue #354, M-3 from #352 code-review)
//
// Precedence: explicit CLI/env overrides > provider.estimate_eip1559_fees().
// A partial override (only one of max_fee / max_prio set) is a user error:
// EIP-1559 requires BOTH or NEITHER; otherwise the tx envelope is malformed.
// Returning `Err(InvalidInput)` here maps to exit 2 per #297 M11, matching
// the "missing required argument" exit-code contract.
// ---------------------------------------------------------------------------

/// Resolve the gas-override precedence for an outgoing transaction.
///
/// - `(Some(f), Some(p))` → `Ok(Some((f, p)))` — caller uses these verbatim.
/// - `(None, None)` → `Ok(None)` — caller falls through to `provider.estimate_eip1559_fees()`.
/// - partial override (only one set) → `Err(InvalidInput)` — exit 2 per #297 M11.
fn resolve_overrides(
    max_fee_per_gas: Option<u128>,
    max_priority_fee_per_gas: Option<u128>,
) -> Result<Option<(u128, u128)>> {
    match (max_fee_per_gas, max_priority_fee_per_gas) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(Error::InvalidInput(
            "either set both --max-fee-per-gas and --max-priority-fee-per-gas, \
             or omit both to use the network fee estimate"
                .into(),
        )),
        (Some(0), Some(p)) => Err(Error::FeeTooLow {
            max_fee_per_gas: 0,
            min_required: p,
        }),
        (Some(f), Some(p)) if p > f => Err(Error::FeeTooLow {
            max_fee_per_gas: f,
            min_required: p,
        }),
        (Some(f), Some(p)) => Ok(Some((f, p))),
    }
}

/// Resolve `(max_fee_per_gas, max_priority_fee_per_gas)` for an outgoing tx.
/// Precedence: explicit CLI/env overrides > `provider.estimate_eip1559_fees()`.
/// The estimate RPC error is redacted via the lib-level helper to avoid
/// embedding the RPC URL in `Error::Rpc` (per L12 H-6 → Issue #356 fix).
/// Resolved EIP-1559 gas parameters — named struct eliminates positional-tuple
/// swap risk at call sites (`wallet_send_erc20` consumes these positionally
/// in `sign_erc20_tx_bytes`, so a swap would silently produce a malformed
/// envelope; L12 type-design finding P1).
struct ResolvedGas {
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
}

/// Resolve gas parameters for an outgoing tx given pre-validated overrides.
/// If overrides present → use verbatim. Else → call `provider.estimate_eip1559_fees()`.
/// Estimate RPC errors map to `Error::GasEstimateFailed` (L12 code-review HIGH #2),
/// with RPC URLs redacted via the local helper (L12 H-6 → Issue #356 fix).
///
/// Callers MUST pre-validate overrides via `resolve_overrides` and run the
/// chain_id trust-boundary check before calling this — the order matters
/// for fail-fast behavior on partial override (L28 Gate C test #2) and
/// defense against wrong-chain RPC gas values (L12 security L-1).
async fn resolve_gas(
    provider: &RootProvider<Ethereum>,
    overrides: Option<(u128, u128)>,
) -> Result<ResolvedGas> {
    if let Some((f, p)) = overrides {
        return Ok(ResolvedGas {
            max_fee_per_gas: f,
            max_priority_fee_per_gas: p,
        });
    }
    let estimate = provider.estimate_eip1559_fees().await.map_err(|e| {
        Error::GasEstimateFailed(format!("estimate_eip1559_fees: {}", redact_rpc_error(&e)))
    })?;
    Ok(ResolvedGas {
        max_fee_per_gas: estimate.max_fee_per_gas,
        max_priority_fee_per_gas: estimate.max_priority_fee_per_gas,
    })
}

#[cfg(test)]
mod gas_overrides_tests {
    use super::*;

    #[test]
    fn both_overrides_some_returns_resolved_pair() {
        let r = resolve_overrides(Some(7_000_000_000), Some(1_000_000_000));
        assert_eq!(r.unwrap(), Some((7_000_000_000u128, 1_000_000_000u128)));
    }

    #[test]
    fn both_overrides_none_returns_none() {
        let r = resolve_overrides(None, None);
        assert_eq!(r.unwrap(), None);
    }

    #[test]
    fn only_max_fee_set_returns_invalid_input() {
        let r = resolve_overrides(Some(7_000_000_000), None);
        assert!(matches!(r, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn only_max_priority_fee_set_returns_invalid_input() {
        let r = resolve_overrides(None, Some(1_000_000_000));
        assert!(matches!(r, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn max_fee_zero_with_priority_set_returns_fee_too_low() {
        let r = resolve_overrides(Some(0), Some(1_000_000_000));
        assert!(
            matches!(r, Err(Error::FeeTooLow { max_fee_per_gas: 0, min_required: 1_000_000_000 })),
            "max_fee_per_gas=0 must yield FeeTooLow (per L12 code-review HIGH #1 + security M-1): got {r:?}",
        );
    }

    #[test]
    fn priority_exceeds_max_fee_returns_fee_too_low() {
        let r = resolve_overrides(Some(5_000_000_000), Some(10_000_000_000));
        assert!(
            matches!(
                r,
                Err(Error::FeeTooLow {
                    max_fee_per_gas: 5_000_000_000,
                    min_required: 10_000_000_000
                })
            ),
            "priority > max_fee must yield FeeTooLow (EIP-1559 invariant): got {r:?}",
        );
    }
}
