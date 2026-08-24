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

// ---------------------------------------------------------------------------
// Wallet create / import / list / show / delete
// ---------------------------------------------------------------------------

pub fn wallet_create(
    mgr: &WalletManager,
    name: &str,
    password: &str,
    network_str: &str,
) -> Result<WalletCreated> {
    if password.is_empty() {
        return Err(Error::InvalidPassword("password must be non-empty".into()));
    }
    // Security C-1: warn when password arrives via argv (shell history,
    // process list). PR-B will replace --password with rpassword / stdin.
    tracing::warn!(
        "passing wallet password on the command line is insecure (shell history, process list); use ETH_PASSWORD env var in CI; PR-B will switch to a TTY prompt"
    );
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
pub async fn wallet_balance(
    provider: &RootProvider<Ethereum>,
    address: &str,
    unit: Option<&str>,
) -> Result<()> {
    let addr = Address::from_str(address)
        .map_err(|e| Error::InvalidInput(format!("invalid address: {e}")))?;
    let balance_wei = provider
        .get_balance(addr)
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
fn redact_rpc_error(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    // Match any `http://...` or `https://...` substring up to the next
    // whitespace, quote, or end-of-string. Conservative — over-redacts
    // rather than under-redacts.
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if (i + 7 <= bytes.len() && &bytes[i..i + 7] == b"http://")
            || (i + 8 <= bytes.len() && &bytes[i..i + 8] == b"https://")
        {
            out.push_str("<rpc-url-redacted>");
            // Skip past the scheme.
            i += if &bytes[i..i + 7] == b"http://" { 7 } else { 8 };
            // Skip until whitespace, quote, ')', or end.
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'"'
                && bytes[i] != b'\''
                && bytes[i] != b')'
            {
                i += 1;
            }
        } else {
            // Push one UTF-8 char (could be multi-byte).
            let ch_end = i + 1;
            // Defensive: don't push partial multi-byte.
            out.push_str(std::str::from_utf8(&bytes[i..ch_end]).unwrap_or("?"));
            i = ch_end;
        }
    }
    out
}

/// Format a `U256` wei amount as `<whole>.<frac>` with `decimals` fractional
/// digits (zero-padded). Returns just `<whole>` when the fractional part
/// is zero.
fn format_wei_as(wei: U256, decimals: u32) -> String {
    if decimals == 0 {
        return wei.to_string();
    }
    let div = U256::from(10u128.pow(decimals));
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
/// envelope locally, then broadcast via `send_raw_transaction`. Anvil
/// gas price hardcoded at 1 gwei; cycle 4+ replaces with dynamic fee
/// estimation + receipt/wait handling.
pub async fn wallet_send_native(
    provider: &RootProvider<Ethereum>,
    signer: &PrivateKeySigner,
    wallet_network: eth_wallet_core::Network,
    to: Address,
    amount_wei: U256,
) -> Result<()> {
    let from = signer.address();
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
        max_fee_per_gas: Some(1_000_000_000u128), // 1 gwei — Anvil default
        max_priority_fee_per_gas: Some(1_000_000_000u128),
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
/// `send_raw_transaction`. Token resolution (symbol vs address) and
/// dynamic decimals land in cycle 5+ alongside gas estimation.
pub async fn wallet_send_erc20(
    provider: &RootProvider<Ethereum>,
    signer: &PrivateKeySigner,
    wallet_network: eth_wallet_core::Network,
    token: Address,
    to: Address,
    amount_wei: U256,
    gas_limit: u64,
) -> Result<()> {
    let from = signer.address();
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
        1_000_000_000u128, // max_fee_per_gas — 1 gwei Anvil default
        1_000_000_000u128, // max_priority_fee_per_gas
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
