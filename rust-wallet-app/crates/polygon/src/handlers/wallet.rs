//! Wallet command handlers — Issue #426 / T6c sub-task (L25 split).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §3.3 (handlers/{mod,wallet,tx,erc20,fee,config,faucet,sign}.rs split)
//! + §5.4 (per-command signatures).
//!
//! T6c2 (this commit) = wallet_list real impl (Story 9) — sync filesystem
//! scan of `<data_dir>/<network>/` for `*.meta.json` files. Remaining
//! wallet commands (create / import / show / delete / balance / sync /
//! send / speed-up) deferred to T6c1/T6c3/T6c4/T6c5 commits per L25.

use std::path::Path;

use polygon_wallet_core::Error;

/// Real `wallet list` impl (Story 9) — T6c2.
///
/// Scans `<data_dir>/<network>/` for `*.meta.json` files and returns
/// their stems as wallet names. Returns empty list when the directory
/// does not exist (no wallets = empty list, not an error).
///
/// T6c2 returns `Vec<String>` (wallet names). T6c2 follow-up adds
/// `--json` structured output (Story 9 AC: `{id, name, created_at}`
/// JSON array per design §3.4).
#[allow(dead_code)] // wired in main.rs::run() WalletAction::List in T6c2 follow-up
pub fn wallet_list(
    data_dir: &Path,
    network: polygon_wallet_core::Network,
) -> polygon_wallet_core::Result<Vec<String>> {
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

/// T6c2 stubs — real impls deferred to T6c1/T6c3/T6c4/T6c5 per L25.
#[allow(dead_code)]
pub fn wallet_create(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet create: deferred past T6c2 (lands in T6c4)".into(),
    ))
}
#[allow(dead_code)]
pub fn wallet_import(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet import: deferred past T6c2 (lands in T6c4)".into(),
    ))
}
#[allow(dead_code)]
pub fn wallet_show(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet show: deferred past T6c2 (lands in T6c3)".into(),
    ))
}
#[allow(dead_code)]
pub fn wallet_delete(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet delete: deferred past T6c2 (lands in T6c3)".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_balance(
    _rpc_url: Option<&str>,
    _address: &str,
) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet balance: deferred past T6c2 (lands in T6c1)".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_sync(_address: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet sync: deferred past T6c2 (lands in T6c3)".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_send_native(_to: &str, _amount: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet send: deferred past T6c2 (lands in T6c5)".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_send_speedup(_tx_hash: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet send speed-up: deferred past T6c2 (lands in T6c5)".into(),
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
}
