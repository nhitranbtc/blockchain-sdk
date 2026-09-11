//! Config command dispatcher.
//!
//! Phase 7.1d (partial) implements 3/3 config commands:
//!   - `show`        — reads `<data_dir>/config.json`
//!   - `set-rpc`     — P7-8: validates scheme (https-only unless `--allow-insecure-tls`);
//!                     rejects URL with userinfo; requires host
//!   - `set-cluster` — P7-20: when transitioning to mainnet-beta, requires
//!                     confirmation (interactive or `SOL_CONFIRM_MAINNET=yes`)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::cli::{Cli, Cluster, ConfigCmd};
use crate::handlers::AppContext;

const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SolanaConfig {
    /// RPC endpoint URL (cluster-specific).
    #[serde(default)]
    pub rpc_url: Option<String>,

    /// SPKI pin (hex SHA-256 of SPKI DER). Env-only per L12 H-1.
    #[serde(default)]
    pub spki_pin: Option<String>,

    /// Current cluster (None = use CLI default).
    #[serde(default)]
    pub cluster: Option<String>,
}

impl SolanaConfig {
    fn config_path(data_dir: &std::path::Path) -> std::path::PathBuf {
        data_dir.join(CONFIG_FILE)
    }

    fn load(data_dir: &std::path::Path) -> Result<Self> {
        let path = Self::config_path(data_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(&path)
            .map_err(|source| anyhow!("failed to read config {}: {source}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("config {} malformed: {e}", path.display()))
    }

    fn save(&self, data_dir: &std::path::Path) -> Result<()> {
        let path = Self::config_path(data_dir);
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(&path, bytes)
            .map_err(|source| anyhow!("failed to write config {}: {source}", path.display()))?;
        // Unix mode 0600 — config may contain sensitive metadata (RPC URLs, SPKI pins).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

pub async fn dispatch(cmd: &ConfigCmd, ctx: &AppContext, cli: &Cli) -> Result<()> {
    match cmd {
        ConfigCmd::Show { json } => show(ctx, *json, cli).await,
        ConfigCmd::SetRpc { url } => set_rpc(ctx, url, cli.allow_insecure_tls).await,
        ConfigCmd::SetCluster { cluster, yes } => set_cluster(ctx, *cluster, cli, *yes).await,
    }
}

async fn show(ctx: &AppContext, json: bool, cli: &Cli) -> Result<()> {
    let mut cfg = SolanaConfig::load(&ctx.data_dir)?;
    if cfg.cluster.is_none() {
        cfg.cluster = Some(format!("{:?}", cli.cluster).to_lowercase());
    }
    if cfg.rpc_url.is_none() {
        cfg.rpc_url = Some(ctx.rpc_url.clone());
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "data_dir": ctx.data_dir.display().to_string(),
                "rpc_url": cfg.rpc_url,
                "spki_pin": cfg.spki_pin,
                "cluster": cfg.cluster,
            }))?
        );
    } else {
        println!("{:<14}  {}", "data_dir", ctx.data_dir.display());
        if let Some(url) = cfg.rpc_url {
            println!("{:<14}  {}", "rpc_url", url);
        }
        if let Some(pin) = cfg.spki_pin {
            println!("{:<14}  {}", "spki_pin", pin);
        }
        if let Some(c) = cfg.cluster {
            println!("{:<14}  {}", "cluster", c);
        }
    }
    Ok(())
}

async fn set_rpc(ctx: &AppContext, url: &str, allow_insecure: bool) -> Result<()> {
    // P7-8: validate URL.
    validate_rpc_url(url, allow_insecure)?;

    let mut cfg = SolanaConfig::load(&ctx.data_dir)?;
    cfg.rpc_url = Some(url.to_string());
    cfg.save(&ctx.data_dir)?;
    println!("set rpc_url = {}", url);
    Ok(())
}

async fn set_cluster(ctx: &AppContext, target: Cluster, cli: &Cli, yes: bool) -> Result<()> {
    let mut cfg = SolanaConfig::load(&ctx.data_dir)?;
    let current_str = cfg.cluster.clone();
    let current = match current_str.as_deref() {
        Some("mainnet-beta") => Cluster::MainnetBeta,
        Some("devnet") => Cluster::Devnet,
        Some("localnet") => Cluster::Localnet,
        _ => cli.cluster,
    };

    // P7-20: confirm transition TO mainnet-beta.
    if matches!(target, Cluster::MainnetBeta) && !matches!(current, Cluster::MainnetBeta) {
        let env_ok = std::env::var("SOL_CONFIRM_MAINNET").ok().as_deref() == Some("yes");
        if !yes && !env_ok && !atty_stdin() {
            return Err(anyhow!(
                "refusing to switch cluster to mainnet-beta without confirmation — \
                 set SOL_CONFIRM_MAINNET=yes or pass --yes"
            ));
        }
        if !yes && !env_ok {
            eprintln!(
                "WARN: switching cluster to mainnet-beta. Subsequent operations will use real SOL."
            );
            eprintln!("Type 'mainnet' to confirm (or pass --yes / set SOL_CONFIRM_MAINNET=yes):");
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(|source| anyhow!("read stdin: {source}"))?;
            if line.trim() != "mainnet" {
                return Err(anyhow!("aborted — confirmation phrase did not match"));
            }
        }
    }

    cfg.cluster = Some(format!("{:?}", target).to_lowercase());
    cfg.save(&ctx.data_dir)?;
    println!("set cluster = {:?}", target);
    Ok(())
}

// -------- helpers --------

/// P7-8 — validate RPC URL before persisting.
fn validate_rpc_url(raw: &str, allow_insecure: bool) -> Result<()> {
    let url = url::Url::parse(raw).map_err(|e| anyhow!("rpc URL parse failed: {e}"))?;
    match url.scheme() {
        "https" => {}
        "http" => {
            if !allow_insecure {
                return Err(anyhow!(
                    "rpc URL must use https (or pass --allow-insecure-tls)"
                ));
            }
        }
        other => {
            return Err(anyhow!(
                "rpc URL scheme must be https or http (got {:?})",
                other
            ));
        }
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(anyhow!("rpc URL must not contain userinfo (user:pass@)"));
    }
    if url.host_str().is_none() {
        return Err(anyhow!("rpc URL must have a host"));
    }
    Ok(())
}

fn atty_stdin() -> bool {
    #[cfg(unix)]
    {
        extern "C" {
            fn isatty(fd: i32) -> i32;
        }
        unsafe { isatty(0) != 0 }
    }
    #[cfg(not(unix))]
    {
        true
    }
}
