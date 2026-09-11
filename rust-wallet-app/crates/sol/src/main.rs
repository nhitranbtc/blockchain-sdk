//! `sol` — Solana wallet CLI (Phase 7.1a scaffold + 7.1b partial).
//!
//! Phase 7.1a: complete clap + dispatch skeleton (22 commands parsed).
//! Phase 7.1b: wallet CRUD (import/show/list/delete/rename) implemented
//! end-to-end with P7-6 / P7-10 / P7-19 fixes. wallet create / balance
//! / send / send-speedup remain stubbed.
//!
//! Per Phase 7 audit `P5-3` fix (companion audit): `&cli` borrowed into
//! `handlers::dispatch`; deep-dive §L L2688 `cli.into()` move-then-match
//! pattern replaced.
//!
//! Per Phase 7 audit `P5-1` corrected mapping: `handlers::error::classify`
//! uses deep-dive §L L2738–2744 + output-conventions L2843 (NOT §J L1825).

mod cli;
mod handlers;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use clap::Parser;
use sol_wallet_core::platform::{FileWalletStorage, PlatformInfo, SystemDirsInfo};
use sol_wallet_core::wallet_manager::WalletManager;

use cli::Cli;
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
        // P7-5: `eprintln!("{e:?}")` wrapped with `Redact<T>` + panic-scrubber regex in Task 7.1c.
        eprintln!("{e:?}");
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Build the per-invocation `AppContext` from the parsed CLI.
async fn build_context(cli: &Cli) -> Result<AppContext> {
    let data_dir = resolve_data_dir(cli.data_dir.clone())?;
    let storage = FileWalletStorage::open(&data_dir)
        .with_context(|| format!("failed to open FileWalletStorage at {}", data_dir.display()))?;
    let wallet_manager = Arc::new(
        WalletManager::new(storage)
            .map_err(|e| anyhow::anyhow!("WalletManager::new failed: {e}"))?,
    );
    Ok(AppContext {
        wallet_manager,
        data_dir,
        cluster: cli.cluster,
        rpc_url: resolve_rpc_url(cli),
    })
}

fn resolve_data_dir(cli_override: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = cli_override {
        return Ok(p);
    }
    Ok(SystemDirsInfo::default().data_dir())
}

fn resolve_rpc_url(cli: &Cli) -> String {
    if let Some(url) = &cli.rpc {
        return url.clone();
    }
    match cli.cluster {
        cli::Cluster::MainnetBeta => "https://api.mainnet-beta.solana.com".to_string(),
        cli::Cluster::Devnet => "https://api.devnet.solana.com".to_string(),
        cli::Cluster::Localnet => "http://127.0.0.1:8899".to_string(),
    }
}
