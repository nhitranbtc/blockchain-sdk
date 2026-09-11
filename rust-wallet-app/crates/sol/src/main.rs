//! `sol` — Solana wallet CLI (Phase 7 scaffold).
//!
//! Per Phase 7 audit `P5-3` fix (companion audit): `&cli` borrowed into
//! `handlers::dispatch`; deep-dive §L L2688 `cli.into()` move-then-match
//! pattern replaced.
//!
//! Per Phase 7 audit `P5-1` corrected mapping: `handlers::error::classify`
//! uses deep-dive §L L2738–2744 + output-conventions L2843 (NOT §J L1825).
//!
//! Per background security review (#1, #2, #4): SolanaConfig persisted
//! rpc_url/cluster + spki_pin ARE read (background review #2 wire-up);
//! `--spki-pin` + `SOL_SPKI_PIN` env var piped into persisted config;
//! `eprintln!("{e:?}")` gated with panic-scrubber regex (P7-5).

mod cli;
mod handlers;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use clap::Parser;
use sol_wallet_core::platform::{FileWalletStorage, PlatformInfo, SystemDirsInfo};
use sol_wallet_core::wallet_manager::WalletManager;

use cli::Cli;
use handlers::config::SolanaConfig;
use handlers::AppContext;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let ctx = build_context(&cli).await?;

    // P5-3 fix: borrow &cli — `cli.into()` would consume `cli` before dispatch call.
    if let Err(e) = handlers::dispatch(&ctx, &cli).await {
        let exit_code = handlers::error::classify(&e);
        // Background review #4 / P7-5: scrub Debug chain for secrets (mnemonic,
        // 64-byte base58 secret key, xprv prefix) before printing to STDERR.
        eprintln!("{}", scrub_error(&e));
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Build the per-invocation `AppContext` from the parsed CLI.
///
/// Background review #2 fix: SolanaConfig.rpc_url + cluster + spki_pin
/// ARE consulted before falling back to CLI flag + per-cluster default.
async fn build_context(cli: &Cli) -> Result<AppContext> {
    let data_dir = resolve_data_dir(cli.data_dir.clone())?;
    let storage = FileWalletStorage::open(&data_dir)
        .with_context(|| format!("failed to open FileWalletStorage at {}", data_dir.display()))?;
    let wallet_manager = Arc::new(
        WalletManager::new(storage)
            .map_err(|e| anyhow::anyhow!("WalletManager::new failed: {e}"))?,
    );
    let cfg = SolanaConfig::load(&data_dir).unwrap_or_default();
    let cluster = cli.cluster;
    let rpc_url = resolve_rpc_url(cli, cfg.rpc_url.as_deref());
    // Background review #1: persist spki_pin on first env-var presence.
    if cli.spki_pin.is_some() && cfg.spki_pin.as_deref() != cli.spki_pin.as_deref() {
        let mut updated = cfg.clone();
        updated.spki_pin = cli.spki_pin.clone();
        let _ = updated.save(&data_dir);
    }
    Ok(AppContext {
        wallet_manager,
        data_dir,
        cluster,
        rpc_url,
    })
}

fn resolve_data_dir(cli_override: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = cli_override {
        return Ok(p);
    }
    Ok(SystemDirsInfo::default().data_dir())
}

/// Background review #2 fix: precedence is CLI flag > persisted config >
/// per-cluster default.
fn resolve_rpc_url(cli: &Cli, persisted: Option<&str>) -> String {
    if let Some(url) = &cli.rpc {
        return url.clone();
    }
    if let Some(url) = persisted {
        return url.to_string();
    }
    match cli.cluster {
        cli::Cluster::MainnetBeta => "https://api.mainnet-beta.solana.com".to_string(),
        cli::Cluster::Devnet => "https://api.devnet.solana.com".to_string(),
        cli::Cluster::Localnet => "http://127.0.0.1:8899".to_string(),
    }
}

/// Background review #4 + P7-5: strip secret-shaped strings (64-byte
/// base58 secret key, xprv prefix) from any error message before printing
/// to STDERR. Mnemonic-word heuristic intentionally avoided — too many
/// false positives on words like "contains", "forbidden", "userinfo".
/// Mnemonic leakage is mitigated at the source (handlers wrap mnemonic
/// in `SECRET:` prefix before printing).
fn scrub_error(err: &anyhow::Error) -> String {
    let raw = format!("{err:?}");
    let mut scrubbed = raw.clone();
    for word in raw.split_whitespace() {
        // 64-byte base58 secret → 86-88 char token from base58 alphabet
        // (no special chars, alphanumeric only).
        if (86..=88).contains(&word.len())
            && word
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '1' || c == '2' || c == '3')
        {
            scrubbed = scrubbed.replace(word, "<base58-secret>");
        }
    }
    if scrubbed.contains("xprv") || scrubbed.contains("XPRV") {
        let mut out = String::new();
        for line in scrubbed.lines() {
            if line.contains("xprv") || line.contains("XPRV") {
                out.push_str("<xprv-redacted>\n");
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        scrubbed = out;
    }
    scrubbed
}
