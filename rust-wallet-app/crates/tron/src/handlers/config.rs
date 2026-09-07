//! `tron config` handlers — plan §Phase 6 Task 5.7.
//!
//! `tron_wallet_core::config::TronConfig` is not `Serialize` (it holds a
//! `PathBuf` and was built as an in-process value), so the CLI owns its own
//! on-disk shape and converts. When the core grows `TronConfig::load`/`save`
//! (plan §Phase 3 carry-over) this module becomes a thin delegate.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tron_wallet_core::config::{default_rpc_url, Network, TronConfig};
use tron_wallet_core::Error as CoreError;

use super::{network_of, CliError, Result};
use crate::cli::NetworkArg;

/// File name under the data dir.
const CONFIG_FILE: &str = "config.json";

/// On-disk config shape. `network` is the lower-case tag from
/// `Network::tag()` so the file stays readable and stable across refactors
/// of the enum's discriminants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConfig {
    pub network: String,
    pub rpc_url: String,
    pub fee_limit_sun: i64,
}

impl StoredConfig {
    fn for_network(network: Network) -> Self {
        let base = TronConfig::for_network(network);
        Self {
            network: network.tag().to_string(),
            rpc_url: base.rpc_url,
            fee_limit_sun: base.fee_limit_sun,
        }
    }
}

/// Parses a stored network tag back to the enum.
fn network_from_tag(tag: &str) -> Result<Network> {
    match tag {
        "mainnet" => Ok(Network::Mainnet),
        "shasta" => Ok(Network::Shasta),
        "nile" => Ok(Network::Nile),
        "local" => Ok(Network::Local),
        other => Err(CliError::Core(CoreError::Config(format!(
            "unknown network tag {other:?} in {CONFIG_FILE}"
        )))),
    }
}

fn path_of(data_dir: &Path) -> PathBuf {
    data_dir.join(CONFIG_FILE)
}

/// Loads the effective config, falling back to Nile defaults when no file
/// exists yet. A first run must not fail — `tron config show` is the plan's
/// Phase 6 CI gate and runs against a clean data dir.
pub fn load(data_dir: &Path) -> Result<TronConfig> {
    let path = path_of(data_dir);
    let stored = match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str::<StoredConfig>(&raw).map_err(|e| {
            CliError::Core(CoreError::Config(format!("parse {}: {e}", path.display())))
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            StoredConfig::for_network(Network::Nile)
        }
        Err(e) => {
            return Err(CliError::Core(CoreError::Config(format!(
                "read {}: {e}",
                path.display()
            ))))
        }
    };
    let network = network_from_tag(&stored.network)?;
    let cfg = TronConfig {
        network,
        rpc_url: stored.rpc_url,
        fee_limit_sun: stored.fee_limit_sun,
        data_dir: Some(data_dir.to_path_buf()),
    };
    cfg.validate()?;
    Ok(cfg)
}

/// Persists the config. Written to a temp sibling then renamed, so a crash
/// mid-write cannot leave a half-parsed config behind.
fn save(data_dir: &Path, cfg: &TronConfig) -> Result<()> {
    let stored = StoredConfig {
        network: cfg.network.tag().to_string(),
        rpc_url: cfg.rpc_url.clone(),
        fee_limit_sun: cfg.fee_limit_sun,
    };
    let body = serde_json::to_string_pretty(&stored)
        .map_err(|e| CliError::Core(CoreError::Config(format!("serialise config: {e}"))))?;
    let final_path = path_of(data_dir);
    let tmp_path = final_path.with_extension("json.tmp");
    std::fs::write(&tmp_path, body)
        .map_err(|e| CliError::Core(CoreError::Config(format!("write config: {e}"))))?;
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| CliError::Core(CoreError::Config(format!("rename config: {e}"))))?;
    Ok(())
}

/// `tron config show [--json]`.
pub fn show(data_dir: &Path, json: bool) -> Result<()> {
    let cfg = load(data_dir)?;
    let value = serde_json::json!({
        "network": cfg.network.tag(),
        "rpc_url": cfg.rpc_url,
        "fee_limit_sun": cfg.fee_limit_sun,
        "data_dir": data_dir.display().to_string(),
    });
    super::emit(
        json,
        value,
        &format!(
            "network      {}\nrpc_url      {}\nfee_limit    {} SUN\ndata_dir     {}",
            cfg.network.tag(),
            cfg.rpc_url,
            cfg.fee_limit_sun,
            data_dir.display()
        ),
    );
    Ok(())
}

/// `tron config set-rpc <url>`.
pub fn set_rpc(data_dir: &Path, url: String) -> Result<()> {
    if url.trim().is_empty() {
        return Err(CliError::BadInput("rpc url must not be empty".into()));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(CliError::BadInput(format!(
            "rpc url {url:?} must start with http:// or https://"
        )));
    }
    let mut cfg = load(data_dir)?;
    cfg.rpc_url = url.trim_end_matches('/').to_string();
    save(data_dir, &cfg)?;
    eprintln!("rpc_url set to {}", cfg.rpc_url);
    Ok(())
}

/// `tron config set-network <net>`.
///
/// Resets `rpc_url` to that network's default. Keeping a mainnet URL under a
/// `nile` label is how funds end up on the wrong chain.
pub fn set_network(data_dir: &Path, network: NetworkArg) -> Result<()> {
    let net = network_of(network);
    let mut cfg = load(data_dir)?;
    cfg.network = net;
    cfg.rpc_url = default_rpc_url(net).to_string();
    save(data_dir, &cfg)?;
    eprintln!("network set to {} (rpc_url {})", net.tag(), cfg.rpc_url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_on_clean_dir_defaults_to_nile() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = load(dir.path()).expect("clean-dir load must succeed");
        assert_eq!(cfg.network, Network::Nile);
        assert!(!cfg.rpc_url.is_empty());
    }

    #[test]
    fn set_network_rewrites_rpc_url() {
        let dir = tempfile::tempdir().expect("tempdir");
        set_rpc(dir.path(), "https://example.invalid".into()).expect("set-rpc");
        set_network(dir.path(), NetworkArg::Mainnet).expect("set-network");
        let cfg = load(dir.path()).expect("load");
        assert_eq!(cfg.network, Network::Mainnet);
        assert_eq!(cfg.rpc_url, default_rpc_url(Network::Mainnet));
    }

    #[test]
    fn set_rpc_rejects_non_http() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(matches!(
            set_rpc(dir.path(), "ftp://example.invalid".into()),
            Err(CliError::BadInput(_))
        ));
    }

    #[test]
    fn set_rpc_round_trips_through_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        set_rpc(dir.path(), "https://nile.example/".into()).expect("set-rpc");
        // Trailing slash is stripped: `TronGridClient` appends its own paths.
        assert_eq!(
            load(dir.path()).expect("load").rpc_url,
            "https://nile.example"
        );
    }
}
