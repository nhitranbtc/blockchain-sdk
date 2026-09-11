//! `sol` CLI — clap struct + enum definitions.
//!
//! Phase 7.1a scaffold. All 22 command variants declared; handler bodies
//! live in `crate::handlers::*` and return `unimplemented!("Phase 7.1b/c/d")`
//! until the corresponding sub-phase lands.
//!
//! Per Phase 7 audit `P5-3` fix (companion audit): `cli.into()` borrow pattern
//! is applied in `main.rs` (not here); `conflicts_with_all = [wait]` was
//! removed from `wait_finalized` (see deep-dive `docs/wallets/2026-09-08-
//! solana-rust-sdks-deep-dive.md` §L L2822 — P5-3 amendment).

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "sol",
    version,
    about = "Solana wallet CLI — Anza stack, Anza-owned crypto"
)]
pub struct Cli {
    /// Wallet data directory (PAL-resolved default).
    #[arg(long, env = "SOL_DATA_DIR", global = true)]
    pub data_dir: Option<PathBuf>,

    /// RPC endpoint URL (overrides per-cluster default).
    #[arg(long, env = "SOL_RPC", global = true)]
    pub rpc: Option<String>,

    /// SPKI pin (hex SHA-256 of SPKI DER) — env-only per L12 H-1.
    #[arg(long, env = "SOL_SPKI_PIN", global = true)]
    pub spki_pin: Option<String>,

    /// Cluster (mainnet-beta | devnet | localnet). NO testnet variant (DEPRECATED 2022-23).
    #[arg(
        long,
        env = "SOL_CLUSTER",
        value_enum,
        default_value_t = Cluster::MainnetBeta,
        global = true
    )]
    pub cluster: Cluster,

    /// Commitment level for confirmation polling.
    #[arg(
        long,
        env = "SOL_COMMITMENT",
        value_enum,
        default_value_t = Commitment::Confirmed,
        global = true
    )]
    pub commitment: Commitment,

    /// Priority fee in micro-lamports per CU.
    #[arg(long, env = "SOL_PRIORITY_FEE", global = true)]
    pub priority_fee: Option<u64>,

    /// Compute unit limit per tx (default 150,000).
    #[arg(long, env = "SOL_CU_LIMIT", global = true)]
    pub cu_limit: Option<u32>,

    /// Allow HTTP RPC (insecure; for localnet only).
    #[arg(long, global = true)]
    pub allow_insecure_tls: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Cluster {
    MainnetBeta,
    Devnet,
    Localnet,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Commitment {
    Processed,
    Confirmed,
    Finalized,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Wallet CRUD + send + send-speedup (9 subcommands).
    #[command(subcommand)]
    Wallet(WalletCmd),

    /// Address derivation (2 subcommands).
    #[command(subcommand)]
    Address(AddressCmd),

    /// Read-only balance queries (2 subcommands).
    #[command(subcommand)]
    Balance(BalanceCmd),

    /// SPL token operations (4 subcommands).
    #[command(subcommand)]
    Spl(SplCmd),

    /// Transaction status + wait polling (2 subcommands).
    #[command(subcommand)]
    Tx(TxCmd),

    /// Persistent config (3 subcommands).
    #[command(subcommand)]
    Config(ConfigCmd),
}

#[derive(Debug, Subcommand)]
pub enum WalletCmd {
    /// Create a new wallet (BIP-39 mnemonic).
    Create {
        #[arg(long)]
        name: String,
        /// Path to file containing BIP-39 mnemonic phrase (REQUIRED per P7-1:
        /// no inline `--mnemonic` flag — trufflehog-detectable).
        #[arg(long)]
        mnemonic_file: PathBuf,
        #[arg(long, value_enum, default_value_t = Cluster::MainnetBeta)]
        cluster: Cluster,
        #[arg(long, default_value_t = 0)]
        account: u32,
        #[arg(long, default_value_t = 0)]
        address_index: u32,
    },

    /// Import existing wallet from mnemonic-file / private-key-file (P7-1: no --mnemonic).
    Import {
        #[arg(long)]
        name: String,
        #[arg(long, value_enum, default_value_t = Cluster::MainnetBeta)]
        cluster: Cluster,
        #[arg(long)]
        mnemonic_file: Option<PathBuf>,
        #[arg(long)]
        private_key_file: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        account: u32,
        #[arg(long, default_value_t = 0)]
        address_index: u32,
        #[arg(long)]
        yes: bool,
    },

    /// Show wallet summary (id, name, address, cluster, created_at).
    Show {
        #[arg(long)]
        id: String,
        #[arg(long)]
        json: bool,
    },

    /// List all wallets.
    List {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        all_clusters: bool,
    },

    /// Permanently delete wallet (P7-6: requires --yes in non-TTY).
    Delete {
        #[arg(long)]
        id: String,
        #[arg(long)]
        yes: bool,
    },

    /// Rename wallet (P7-10: name validation).
    Rename {
        #[arg(long)]
        id: String,
        #[arg(long)]
        to: String,
    },

    /// Wallet balance — unlocks keypair via WalletManager (P7-2: Zeroizing).
    Balance {
        #[arg(long)]
        wallet_id: Option<String>,
        #[arg(long)]
        address: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Send native SOL or SPL token (P7-2 / P7-7 / P7-13).
    Send {
        #[arg(long)]
        wallet_id: Option<String>,
        #[arg(long, conflicts_with = "wallet_id")]
        to: Option<String>,
        #[arg(long, conflicts_with = "to")]
        to_wallet: Option<String>,
        #[arg(long)]
        amount: Option<String>,
        #[arg(long, default_value = "sol")]
        unit: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        priority_fee: Option<u64>,
        #[arg(long)]
        cu_limit: Option<u32>,
        #[arg(long)]
        memo: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, conflicts_with = "dry_run")]
        sign_only: bool,
        #[arg(long, conflicts_with = "wait_finalized")]
        wait: bool,
        #[arg(long, conflicts_with = "wait")]
        wait_finalized: bool,
        #[arg(long, default_missing_value = "yes", num_args = 0..=1)]
        confirm_mainnet: Option<String>,
    },

    /// Re-broadcast same tx with higher priority fee (Solana has no RBF).
    SendSpeedup {
        #[arg(long)]
        wallet_id: String,
        #[arg(long)]
        sig: String,
        #[arg(long)]
        priority_fee: u64,
    },
}

#[derive(Debug, Subcommand)]
pub enum AddressCmd {
    /// Derive address from wallet-id (P7-16) or mnemonic-file (P7-17: returns Pubkey only).
    New {
        #[arg(long)]
        wallet_id: Option<String>,
        #[arg(long)]
        mnemonic_file: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        account: u32,
        #[arg(long, default_value_t = 0)]
        address_index: u32,
    },
    /// Show pubkey for wallet-id.
    Pubkey {
        #[arg(long)]
        wallet_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum BalanceCmd {
    /// SOL balance for base58 address.
    Sol {
        #[arg(long)]
        address: String,
        #[arg(long, default_value = "sol")]
        unit: String,
    },
    /// SPL token balance for address.
    Spl {
        #[arg(long)]
        address: String,
        #[arg(long)]
        token: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SplCmd {
    Send {
        #[arg(long)]
        wallet_id: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        amount: Option<String>,
        #[arg(long)]
        token: String,
        #[arg(long)]
        priority_fee: Option<u64>,
        #[arg(long)]
        memo: Option<String>,
        #[arg(long)]
        skip_memo_required: bool,
        #[arg(long, requires = "skip_memo_required")]
        i_understand_no_memo_enforcement: bool,
        #[arg(long)]
        skip_ata_create: bool,
    },
    Approve {
        #[arg(long)]
        wallet_id: Option<String>,
        #[arg(long)]
        token: String,
        #[arg(long)]
        delegate: String,
        #[arg(long)]
        amount: String,
    },
    Balance {
        #[arg(long)]
        address: String,
        #[arg(long)]
        token: String,
    },
    Allowance {
        #[arg(long)]
        token: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        delegate: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TxCmd {
    Get {
        #[arg(long)]
        sig: String,
        #[arg(long)]
        json: bool,
    },
    Wait {
        #[arg(long)]
        sig: String,
        /// Timeout in seconds (P7-11: 1..=600 — 10 minutes max covers 12-slot
        /// `finalized` path with margin).
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=600))]
        timeout: u64,
        #[arg(long, default_value_t = 2)]
        poll_interval: u64,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Set RPC URL (P7-8: https-only unless --allow-insecure-tls; no userinfo).
    SetRpc {
        #[arg(long)]
        url: String,
    },
    /// Switch cluster (P7-20: mainnet-beta transition requires confirm).
    SetCluster {
        #[arg(long, value_enum)]
        cluster: Cluster,
        #[arg(long)]
        yes: bool,
    },
}
