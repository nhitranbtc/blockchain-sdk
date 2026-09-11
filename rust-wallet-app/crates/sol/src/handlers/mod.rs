//! Handler module — dispatches `Commands` to per-handler functions.
//!
//! Phase 7.1a scaffold: dispatch + module wiring + `AppContext` threading.
//! Phase 7.1b: wallet CRUD (import/show/list/delete/rename) implemented.
//! Phase 7.1c: wallet send + send-speedup + spl (P7-2 Zeroizing, P7-7 mainnet confirm, P7-13 dry-run).
//! Phase 7.1d: address + balance + tx + config (P7-8 URL validate, P7-11 timeout, P7-17 derive_pubkey).
//!
//! `AppContext` carries the `WalletManager` + cluster + rpc_url. Concrete
//! `WalletManager<FileWalletStorage>` — V0.1 desktop-only; mobile PAL impls
//! land in V0.1.5.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use sol_wallet_core::platform::FileWalletStorage;
use sol_wallet_core::wallet_manager::WalletManager;

use crate::cli::{Cli, Cluster, Commands};

pub mod address;
pub mod balance;
pub mod config;
pub mod error;
pub mod spl;
pub mod tx;
pub mod wallet;

/// Per-invocation context threaded through every handler.
pub struct AppContext {
    pub wallet_manager: Arc<WalletManager<FileWalletStorage>>,
    pub data_dir: PathBuf,
    pub cluster: Cluster,
    pub rpc_url: String,
}

pub async fn dispatch(ctx: &AppContext, cli: &Cli) -> Result<()> {
    match &cli.command {
        Commands::Wallet(cmd) => wallet::dispatch(cmd, ctx, cli).await,
        Commands::Address(cmd) => address::dispatch(cmd, ctx, cli).await,
        Commands::Balance(cmd) => balance::dispatch(cmd, ctx, cli).await,
        Commands::Spl(cmd) => spl::dispatch(cmd, ctx, cli).await,
        Commands::Tx(cmd) => tx::dispatch(cmd, ctx, cli).await,
        Commands::Config(cmd) => config::dispatch(cmd, ctx, cli).await,
    }
}
