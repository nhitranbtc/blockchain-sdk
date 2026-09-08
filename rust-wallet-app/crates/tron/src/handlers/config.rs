//! `tron config` handlers — plan §Phase 6 Task 5.7.
//!
//! `tron_wallet_core::config::TronConfig` is not `Serialize` (it holds a
//! `PathBuf` and was built as an in-process value), so the CLI owns its own
//! on-disk shape and converts. When the core grows `TronConfig::load`/`save`
//! (plan §Phase 3 carry-over) this module becomes a thin delegate.

use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use tron_wallet_core::config::{default_rpc_url, Network, TronConfig};
use tron_wallet_core::Error as CoreError;

use super::{CliError, Result};

/// File name under the data dir.
const CONFIG_FILE: &str = "config.json";

/// On-disk config shape. `network` reuses the core enum's `lowercase` serde
/// tags so a hand-edited `config.json` with `"network": "mainnet"` round-trips
/// without any custom (de)serialisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConfig {
    #[serde(default)]
    pub network: Network,
    pub rpc_url: String,
    /// Per-tx energy ceiling in SUN. Zero is nonsensical (the chain would
    /// refuse the transaction) and survives a serialise round-trip as the
    /// same non-zero value.
    #[serde(deserialize_with = "de_nonzero_u64")]
    pub fee_limit_sun: NonZeroU64,
}

/// Custom deserialiser: reject `0` at the JSON layer so a hand-edited config
/// file cannot slip past `TronConfig::validate`. `NonZeroU64`'s serde impl
/// already does this; we only need it because the field's stored form is
/// `u64` (forward-compatible with the prior schema).
fn de_nonzero_u64<'de, D>(de: D) -> std::result::Result<NonZeroU64, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = u64::deserialize(de)?;
    NonZeroU64::new(raw).ok_or_else(|| serde::de::Error::custom("fee_limit_sun must be > 0"))
}

impl StoredConfig {
    fn for_network(network: Network) -> Self {
        let base = TronConfig::for_network(network);
        Self {
            network,
            rpc_url: base.rpc_url,
            // `TronConfig::for_network` always sets 100_000_000 SUN, which is
            // strictly non-zero by construction. The `unwrap` documents that
            // invariant.
            fee_limit_sun: NonZeroU64::new(base.fee_limit_sun as u64)
                .expect("TronConfig::for_network fee_limit_sun is non-zero"),
        }
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
    let cfg = TronConfig {
        network: stored.network,
        rpc_url: stored.rpc_url,
        fee_limit_sun: stored.fee_limit_sun.get() as i64,
        data_dir: Some(data_dir.to_path_buf()),
    };
    cfg.validate()?;
    Ok(cfg)
}

/// Persists the config. Written to a temp sibling then renamed, so a crash
/// mid-write cannot leave a half-parsed config behind.
fn save(data_dir: &Path, cfg: &TronConfig) -> Result<()> {
    let stored = StoredConfig {
        network: cfg.network,
        rpc_url: cfg.rpc_url.clone(),
        fee_limit_sun: NonZeroU64::new(cfg.fee_limit_sun as u64).ok_or_else(|| {
            CliError::BadInput(format!(
                "fee_limit_sun must be > 0 (got {})",
                cfg.fee_limit_sun
            ))
        })?,
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
///
/// Only `https://` is accepted: a signed TRON envelope sent over plaintext
/// HTTP can be intercepted and replayed inside its 60s window. The plan
/// (Risk Register #2 — single-SHA256 txid) leaves no margin for an
/// attacker who also gets to rebroadcast.
pub fn set_rpc(data_dir: &Path, url: String) -> Result<()> {
    if url.trim().is_empty() {
        return Err(CliError::BadInput("rpc url must not be empty".into()));
    }
    if url.starts_with("http://") {
        return Err(CliError::BadInput(
            "rpc url must use https:// — http:// exposes the signed envelope to interception"
                .into(),
        ));
    }
    if !url.starts_with("https://") {
        return Err(CliError::BadInput(format!(
            "rpc url {url:?} must start with https://"
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
pub fn set_network(data_dir: &Path, network: Network) -> Result<()> {
    let mut cfg = load(data_dir)?;
    cfg.network = network;
    cfg.rpc_url = default_rpc_url(network).to_string();
    save(data_dir, &cfg)?;
    eprintln!("network set to {} (rpc_url {})", network.tag(), cfg.rpc_url);
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
        set_network(dir.path(), Network::Mainnet).expect("set-network");
        let cfg = load(dir.path()).expect("load");
        assert_eq!(cfg.network, Network::Mainnet);
        assert_eq!(cfg.rpc_url, default_rpc_url(Network::Mainnet));
    }

    #[test]
    fn set_rpc_rejects_http() {
        // http:// exposes the signed envelope to on-path interception and
        // replay. The set-rpc gate exists to make that mistake loud, not
        // silent.
        let dir = tempfile::tempdir().expect("tempdir");
        let err = set_rpc(dir.path(), "http://example.invalid".into())
            .expect_err("http:// must be refused");
        let msg = match err {
            CliError::BadInput(m) => m,
            other => panic!("expected BadInput, got {other:?}"),
        };
        assert!(
            msg.contains("https://"),
            "error must mention https://, got {msg:?}"
        );
    }

    #[test]
    fn set_rpc_rejects_ftp() {
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

    #[test]
    fn stored_config_network_round_trips_as_enum() {
        // The on-disk shape uses the lowercase tag from `Network::tag()` so a
        // hand-edited `config.json` with `"network": "mainnet"` still loads.
        let body = r#"{
            "network": "mainnet",
            "rpc_url": "https://example.invalid",
            "fee_limit_sun": 100000000
        }"#;
        let parsed: StoredConfig = serde_json::from_str(body).expect("parse");
        assert_eq!(parsed.network, Network::Mainnet);
        assert_eq!(parsed.fee_limit_sun.get(), 100_000_000);

        // Serialise back and confirm the tag survives the round-trip.
        let again = serde_json::to_string(&parsed).expect("serialise");
        assert!(again.contains(r#""network":"mainnet""#));
    }

    #[test]
    fn stored_config_rejects_zero_fee_limit() {
        // Zero is not a legal fee limit; it would fail later at the chain
        // anyway, but catching it at parse time gives a clearer error.
        let body = r#"{
            "network": "nile",
            "rpc_url": "https://example.invalid",
            "fee_limit_sun": 0
        }"#;
        assert!(serde_json::from_str::<StoredConfig>(body).is_err());
    }

    #[test]
    fn load_hand_edited_config_with_mainnet_tag() {
        // The lowercase serde tags let a CLI operator write a config by hand
        // without going through `set-network`.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(CONFIG_FILE);
        std::fs::write(
            &path,
            r#"{
                "network": "mainnet",
                "rpc_url": "https://custom.example",
                "fee_limit_sun": 200000000
            }"#,
        )
        .expect("write");
        let cfg = load(dir.path()).expect("load");
        assert_eq!(cfg.network, Network::Mainnet);
        assert_eq!(cfg.rpc_url, "https://custom.example");
        assert_eq!(cfg.fee_limit_sun, 200_000_000);
    }
}
