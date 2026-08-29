//! Wallet command handlers — Issue #426 / T6c sub-task (L25 split).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §3.3 (handlers/{mod,wallet,tx,erc20,fee,config,faucet,sign}.rs split)
//! + §5.4 (per-command signatures).
//!
//! T6c3 follow-up: real `wallet_show` impl (Story 9 — `wallet show`).
//! Reads `.meta.json` (plaintext metadata; no decrypt — encrypted blob
//! inspection deferred to T6d when rpassword + AES-GCM decryption
//! wires up). Real `wallet_create` + `wallet_import` deferred to T6c4;
//! `wallet_send_*` to T6c5 per L25 sub-task split.

use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use std::str::FromStr;

use polygon_wallet_core::{new_http, new_http_polygon_amoy, Error, Result, WalletInfo};

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
        if path.extension().and_then(|s| s.to_str()) == Some("meta.json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
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

#[allow(dead_code)]
pub fn wallet_create(_name: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet create: deferred past T6c3 follow-up (lands in T6c4)".into(),
    ))
}
#[allow(dead_code)]
pub fn wallet_import(_name: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet import: deferred past T6c3 follow-up (lands in T6c4)".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_sync(_address: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet sync: deferred past T6c3 follow-up (lands in T6d)".into(),
    ))
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

#[cfg(test)]
mod tests {
    use super::wallet_list;
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
}
