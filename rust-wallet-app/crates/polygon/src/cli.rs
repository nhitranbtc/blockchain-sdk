//! `polygon` CLI argument types — Issue #426 / T6b sub-task (L25 split).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §3.4 (clap subcommand tree) + §5.2 (clap derive types).
//!
//! T6b = scaffold only. Handler bodies land in T6c/T6d (per L25 sub-task
//! split). Cross-cutting flags wired here; per-handler stubs in `main.rs::run()`
//! dispatch to `Error::Rpc("deferred past T6b — landing in T6c/T6d")`.
//!
//! Mirror of `rust-wallet-app/crates/eth/src/main.rs:48-372` (single-file
//! clap derive pattern) + `btc/src/cli.rs:13-189` (per-action struct
//! split). The 7-file handler split design (per design doc §3.3) is deferred
//! to T6c when per-handler impls land.

use alloy_primitives::Address;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// clap value_parser for Address-typed flags (mirrors eth/src/main.rs:44-46).
fn parse_address(s: &str) -> Result<Address, String> {
    s.parse::<Address>()
        .map_err(|e| format!("invalid address: {e}"))
}

/// Top-level CLI. `name = "polygon"`, version from Cargo.toml, about string.
#[derive(Parser, Debug)]
#[command(
    name = "polygon",
    version,
    about = "Polygon PoS wallet CLI (alloy v1.8.x)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Default RPC URL override (env: `POLYGON_RPC_URL`).
    #[arg(long, global = true, env = "POLYGON_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Wallet-store base directory override (env: `POLYGON_DATA_DIR`).
    #[arg(long, global = true, env = "POLYGON_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
}

/// Top-level command enum — 9 variants per design §3.4.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Wallet commands (create/import/list/show/delete/balance/sync/send/speed-up).
    Wallet {
        #[command(subcommand)]
        action: WalletAction,
    },
    /// Transaction queries (list/get).
    Tx {
        #[command(subcommand)]
        action: TxAction,
    },
    /// ERC-20 token commands (send/balance/list/register/approve).
    Erc20 {
        #[command(subcommand)]
        action: Erc20Action,
    },
    /// Query current fee parameters (gas price, base fee).
    Fee(FeeArgs),
    /// Display resolved CLI configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Print Amoy faucet URL + drip-to instructions (Story 30).
    Faucet(FaucetArgs),
    /// Sign an EIP-191 personal message (Story 18).
    SignMessage(SignMessageArgs),
    /// Sign EIP-712 typed data with chain_id validation (Story 27, Q7 gate).
    SignTyped(SignTypedArgs),
    /// Print version + exit.
    Version,
}

/// Wallet subcommand actions (9 variants per design §3.4).
#[derive(Subcommand, Debug)]
pub enum WalletAction {
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long, default_value = "m/44'/60'/0'/0/0")]
        derivation_path: String,
        #[arg(long, default_value_t = 0)]
        account_index: u32,
        #[arg(long)]
        legacy_token_symbol: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    Import {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long, conflicts_with = "private_key")]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with = "mnemonic")]
        private_key: Option<String>,
        #[arg(long, default_value_t = 0)]
        account_index: u32,
        #[arg(long)]
        legacy_token_symbol: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    List {
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    Show {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        addresses: bool,
        #[arg(long)]
        export: bool,
        #[arg(long)]
        json: bool,
    },
    Delete {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
    },
    Balance {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long, default_value = "pol")]
        unit: String,
        #[arg(long)]
        legacy_token_symbol: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    Sync {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        rpc_url: Option<String>,
        /// Output Transfer events as JSON (`Vec<TxSummary>`).
        /// JSON fields: `block_number` (u64), `tx_hash` (0x-hex B256),
        /// `from` (0x-hex Address), `to` (0x-hex Address), `value`
        /// (decimal U256). Default = human-readable text (one line per
        /// Transfer).
        #[arg(long)]
        json: bool,
    },
    Send(SendArgs),
    /// RBF / speed-up replace-by-fee (Story 17).
    SendSpeedup(SendSpeedupArgs),
}

/// `wallet send` args (13 fields per design §3.4).
#[derive(clap::Args, Debug)]
pub struct SendArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub to: String,
    #[arg(long)]
    pub amount: String,
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long, default_value = "pol")]
    pub unit: String,
    #[arg(long)]
    pub batch: Option<String>,
    #[arg(long)]
    pub drain: bool,
    #[arg(long)]
    pub nonce: Option<u64>,
    #[arg(long)]
    pub gas_limit: Option<u64>,
    /// Fee tier: fastest | half_hour | hour | economy (default half_hour).
    #[arg(long, default_value = "half_hour")]
    pub fee: String,
    #[arg(long)]
    max_fee_gwei: Option<f64>,
    #[arg(long)]
    priority_fee_gwei: Option<f64>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub wait: bool,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `wallet send speed-up` args (Story 17).
#[derive(clap::Args, Debug)]
pub struct SendSpeedupArgs {
    #[arg(long)]
    pub tx_hash: String,
    #[arg(long)]
    max_fee_gwei: f64,
    #[arg(long)]
    priority_fee_gwei: f64,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `tx` subcommands (Story 7).
#[derive(Subcommand, Debug)]
pub enum TxAction {
    List {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        since_block: Option<u64>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        json: bool,
    },
    Get {
        #[arg(long)]
        tx_hash: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
}

/// `erc20` subcommands (Stories 21-25).
#[derive(Subcommand, Debug)]
pub enum Erc20Action {
    Send {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        /// Token symbol (USDC/USDT/DAI) or hex address via `--token-address`.
        #[arg(long)]
        token: String,
        #[arg(long, value_parser = parse_address)]
        token_address: Option<String>,
        #[arg(long, value_parser = parse_address)]
        to: String,
        #[arg(long)]
        amount: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        gas_limit: Option<u64>,
        #[arg(long)]
        max_fee_gwei: Option<f64>,
        #[arg(long)]
        priority_fee_gwei: Option<f64>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    Balance {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long)]
        token: String,
        #[arg(long, value_parser = parse_address)]
        token_address: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        decimals: Option<u8>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    List {
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        json: bool,
    },
    Register {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        list: bool,
        #[arg(long)]
        remove: Option<String>,
    },
    Approve {
        #[arg(long)]
        name: String,
        #[arg(long)]
        token: String,
        #[arg(long, value_parser = parse_address)]
        spender: String,
        #[arg(long, default_value = "0")]
        amount: String,
        #[arg(long)]
        unlimited: bool,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        gas_limit: Option<u64>,
        #[arg(long)]
        max_fee_gwei: Option<f64>,
        #[arg(long)]
        priority_fee_gwei: Option<f64>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
}

/// `fee` args (Story 8).
#[derive(clap::Args, Debug)]
pub struct FeeArgs {
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `config` subcommands (Story 11).
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    Show {
        #[arg(long)]
        json: bool,
    },
}

/// `faucet` args (Story 30).
#[derive(clap::Args, Debug)]
pub struct FaucetArgs {
    #[arg(long, value_parser = parse_address)]
    pub address: String,
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long, default_value = "POL")]
    pub faucet_token: String,
    /// Auto-claim reserved for T7 (operator-driven per L29).
    #[arg(long)]
    pub auto: bool,
}

/// `sign-message` args (Story 18) — EIP-191 personal_sign.
#[derive(clap::Args, Debug)]
pub struct SignMessageArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long)]
    pub message: String,
    #[arg(long, value_parser = parse_address)]
    pub address: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub verify: Option<String>,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `sign-typed` args (Story 27) — EIP-712 typed-data with chain_id validation.
///
/// Q7 critical-tier: `--chain-id` is **required** (no `Option`), and
/// `assert_polygon_chain_id` (handlers/sign.rs) rejects any chain_id
/// outside {137, 80002} before signing.
#[derive(clap::Args, Debug)]
pub struct SignTypedArgs {
    /// REQUIRED. Validated against {137, 80002} before signing (Q7 gate).
    #[arg(long)]
    pub chain_id: u64,
    #[arg(long, conflicts_with = "typed_data_file")]
    pub typed_data: Option<String>,
    #[arg(long, conflicts_with = "typed_data")]
    pub typed_data_file: Option<PathBuf>,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub address: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub verify: Option<String>,
    #[arg(long)]
    pub rpc_url: Option<String>,
}
