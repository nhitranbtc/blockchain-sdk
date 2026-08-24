//! `eth` CLI binary — Issue #309 (Task 10) scaffold + Issue #337 (Task 13
//! follow-up) wiring.
//!
//! ## Subcommand structure (per Issue #309 + #337)
//!
//! PR-A wires these against Anvil-backed handlers:
//! - `wallet create --name --network --password`
//! - `wallet import --name --mnemonic | --private-key --password --network`
//! - `wallet list | show --name|--id --network | delete --name|--id --network`
//! - `wallet balance --address --network --unit [--rpc-url]`
//! - `tx get --tx-hash [--rpc-url]`
//!
//! PR-B (deferred per #337 split): `wallet send-native`, `wallet send-erc20`,
//! `tx list`. Handlers return `Error::Rpc("...wired in PR-B...")` so the
//! user-facing message is honest about scope.
//!
//! ## Exit codes
//!
//! Forwarded from `eth_wallet_core::error::Error::exit_code()` per #297 M11.
//! Stable 0..=5: success / user-abort / bad-input / rpc / wallet-balance /
//! signing-broadcast. `std::process::exit` carries the code to the shell.

mod handlers;

use std::path::PathBuf;
use std::str::FromStr;

use alloy_primitives::{Address, U256};
use clap::{Parser, Subcommand};

use crate::handlers::{
    open_manager, open_provider, print_wallet_created, tx_get, tx_list_stub, wallet_balance,
    wallet_create, wallet_delete, wallet_import, wallet_list, wallet_send_erc20,
    wallet_send_native, wallet_show,
};
use eth_wallet_core::{Error, Network};

#[derive(Parser, Debug)]
#[command(name = "eth", version, about = "Ethereum wallet CLI (alloy v1.8.x)")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Default RPC URL (overrides per-subcommand).
    /// Per #297 M10 SPKI pin was deferred per #330 — `provider::new_http`
    /// (default rustls TLS + system CAs) handles all RPC traffic.
    #[arg(long, global = true, env = "ETH_RPC_URL")]
    rpc_url: Option<String>,

    /// Override the wallet-store base directory (default: XDG data dir).
    /// Tests + CI inject `ETH_DATA_DIR=<tempdir>` so wallet state stays
    /// hermetic per-test.
    #[arg(long, global = true, env = "ETH_DATA_DIR")]
    data_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Wallet {
        #[command(subcommand)]
        action: WalletAction,
    },
    /// Send a transaction (native ETH or ERC-20). PR-B only — returns
    /// Error::Rpc in PR-A. Issue #337 phase 2.
    Send(SendArgs),
    Tx {
        #[command(subcommand)]
        action: TxAction,
    },
    /// Query fee parameters (gas price, base fee). Deferred — returns
    /// Error::Rpc in PR-A.
    Fee(FeeArgs),
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Sign an EIP-191 message. Deferred — returns Error::Rpc in PR-A.
    SignMessage {
        #[arg(long)]
        message: String,
        #[arg(long)]
        mnemonic: Option<String>,
        #[arg(long)]
        address: Option<String>,
        #[arg(long)]
        verify: Option<String>,
    },
    /// Sign EIP-712 typed data. Deferred — blocked on alloy eip712
    /// feature flag per #302 sign_typed_data follow-up.
    SignTyped {
        #[arg(long, conflicts_with = "typed_data_file")]
        typed_data: Option<String>,
        #[arg(long, conflicts_with = "typed_data")]
        typed_data_file: Option<String>,
        #[arg(long)]
        mnemonic: Option<String>,
        #[arg(long)]
        address: Option<String>,
        #[arg(long)]
        verify: Option<String>,
    },
    Erc20 {
        #[command(subcommand)]
        action: Erc20Action,
    },
    Version,
}

#[derive(Subcommand, Debug)]
enum WalletAction {
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value = "sepolia")]
        network: String,
    },
    Import {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value = "sepolia")]
        network: String,
        #[arg(long, conflicts_with = "private_key")]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with = "mnemonic")]
        private_key: Option<String>,
    },
    Balance {
        #[arg(long)]
        address: String,
        #[arg(long, default_value = "sepolia")]
        network: String,
        #[arg(long)]
        unit: Option<String>,
    },
    /// Sync wallet with chain state (rebuild meta from on-chain). Deferred.
    Sync {
        #[arg(long, default_value = "sepolia")]
        network: String,
    },
    List,
    Show {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, default_value = "sepolia")]
        network: String,
    },
    Delete {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, default_value = "sepolia")]
        network: String,
    },
}

#[derive(clap::Args, Debug)]
struct SendArgs {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long, default_value = "0")]
    amount: String,
    #[arg(long, default_value = "sepolia")]
    network: String,
    #[arg(long)]
    fee: Option<String>,
    #[arg(long)]
    max_fee_gwei: Option<f64>,
    #[arg(long)]
    priority_fee_gwei: Option<f64>,
    #[arg(long)]
    nonce: Option<u64>,
    #[arg(long)]
    gas_limit: Option<u64>,
    #[arg(long, default_value = "false")]
    dry_run: bool,
    #[arg(long, default_value = "false")]
    wait: bool,
}

#[derive(Subcommand, Debug)]
enum TxAction {
    List {
        #[arg(long)]
        since_block: Option<u64>,
        #[arg(long, default_value = "25")]
        limit: u32,
        #[arg(long, default_value = "false")]
        pending: bool,
    },
    Get {
        #[arg(long)]
        tx_hash: String,
    },
}

#[derive(clap::Args, Debug)]
struct FeeArgs {
    #[arg(long, default_value = "sepolia")]
    network: String,
    #[arg(long, default_value = "false")]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    Show {
        #[arg(long, default_value = "false")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum Erc20Action {
    Balance {
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "false")]
        all: bool,
        #[arg(long, default_value = "false")]
        json: bool,
    },
    Send {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        token_address: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value = "0")]
        amount: String,
        #[arg(long)]
        gas_limit: Option<u64>,
        #[arg(long, default_value = "sepolia")]
        network: String,
    },
    List {
        #[arg(long, default_value = "false")]
        json: bool,
    },
    Register {
        #[arg(long)]
        address: Option<String>,
        #[arg(long, default_value = "false")]
        list: bool,
        #[arg(long)]
        remove: Option<String>,
    },
    Approve {
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        spender: Option<String>,
        #[arg(long, default_value = "0")]
        amount: String,
    },
    Deploy {
        #[arg(long)]
        token_name: Option<String>,
        #[arg(long)]
        token_symbol: Option<String>,
        #[arg(long)]
        decimals: Option<u8>,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let exit_code = match run(cli) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            e.exit_code()
        }
    };
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> eth_wallet_core::Result<()> {
    let mgr = open_manager(cli.data_dir.as_ref())?;

    // Local tokio runtime for the few async handlers (balance, tx get).
    // The handlers themselves are async; we drive them here with
    // `block_on` because main is sync. Per #333 sync-fns touching async
    // deps are an explicit carve-out — only `cli main()` does this, never
    // test code.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| eth_wallet_core::Error::Rpc(format!("tokio: {e}")))?;

    rt.block_on(async {
        match cli.command {
            Command::Version => {
                println!("eth {}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
            Command::Wallet { action } => match action {
                WalletAction::Create {
                    name,
                    password,
                    network,
                } => {
                    let created = wallet_create(&mgr, &name, &password, &network)?;
                    print_wallet_created(&created);
                    Ok(())
                }
                WalletAction::Import {
                    name,
                    password,
                    network,
                    mnemonic,
                    private_key,
                } => {
                    let created = wallet_import(
                        &mgr,
                        &name,
                        &password,
                        &network,
                        mnemonic.as_deref(),
                        private_key.as_deref(),
                    )?;
                    print_wallet_created(&created);
                    Ok(())
                }
                WalletAction::Balance {
                    address,
                    network: _,
                    unit,
                } => {
                    let rpc = cli.rpc_url.as_deref().unwrap_or("http://127.0.0.1:8545");
                    let provider = open_provider(rpc)?;
                    wallet_balance(&provider, &address, unit.as_deref()).await
                }
                WalletAction::Sync { .. } => Err(eth_wallet_core::Error::Rpc(
                    "wallet sync: deferred past #337 (follow-up)".into(),
                )),
                WalletAction::List => wallet_list(&mgr),
                WalletAction::Show { name, id, network } => {
                    wallet_show(&mgr, name.as_deref(), id.as_deref(), &network)
                }
                WalletAction::Delete { name, id, network } => {
                    wallet_delete(&mgr, name.as_deref(), id.as_deref(), &network)
                }
            },
            Command::Send(args) => {
                let rpc = cli.rpc_url.as_deref().unwrap_or("http://127.0.0.1:8545");
                let provider = open_provider(rpc)?;
                let to_str = args
                    .to
                    .as_deref()
                    .ok_or_else(|| Error::InvalidInput("--to required".into()))?;
                let to = Address::from_str(to_str)
                    .map_err(|e| Error::InvalidInput(format!("invalid --to address: {e}")))?;
                let amount_wei: U256 = args
                    .amount
                    .parse()
                    .map_err(|e| Error::InvalidInput(format!("invalid --amount: {e}")))?;

                let (name, password) = match (args.name.as_deref(), args.password.as_deref()) {
                    (Some(n), Some(p)) => (n, p),
                    _ => {
                        return Err(Error::InvalidInput("--name and --password required".into()));
                    }
                };
                let net = Network::parse_cli(&args.network)
                    .map_err(|e| Error::InvalidInput(e.to_string()))?;
                let wallet_id = mgr
                    .lookup_by_name(name, net)
                    .map_err(crate::handlers::map_wallet_err)?;
                let signer = mgr
                    .unlock_signer(wallet_id, password.as_bytes())
                    .map_err(crate::handlers::map_wallet_err)?;

                wallet_send_native(&provider, &signer, to, amount_wei).await
            }
            Command::Tx { action } => match action {
                TxAction::Get { tx_hash } => {
                    let rpc = cli.rpc_url.as_deref().unwrap_or("http://127.0.0.1:8545");
                    let provider = open_provider(rpc)?;
                    tx_get(&provider, &tx_hash).await
                }
                TxAction::List { .. } => tx_list_stub().await,
            },
            Command::Fee(_) => Err(eth_wallet_core::Error::Rpc(
                "fee: deferred past #337 (follow-up)".into(),
            )),
            Command::Config { .. } => Err(eth_wallet_core::Error::Rpc(
                "config: deferred past #337 (follow-up)".into(),
            )),
            Command::SignMessage { .. } | Command::SignTyped { .. } => Err(
                eth_wallet_core::Error::Rpc("sign-message / sign-typed: deferred".into()),
            ),
            Command::Erc20 { action } => match action {
                Erc20Action::Send {
                    name,
                    password,
                    token,
                    token_address,
                    to,
                    amount,
                    gas_limit,
                    network,
                } => {
                    let rpc = cli.rpc_url.as_deref().unwrap_or("http://127.0.0.1:8545");
                    let provider = open_provider(rpc)?;
                    let to_str = to
                        .as_deref()
                        .ok_or_else(|| Error::InvalidInput("--to required".into()))?;
                    let to_addr = Address::from_str(to_str)
                        .map_err(|e| Error::InvalidInput(format!("invalid --to address: {e}")))?;
                    let amount_wei: U256 = amount
                        .parse()
                        .map_err(|e| Error::InvalidInput(format!("invalid --amount: {e}")))?;
                    let token_str =
                        token
                            .as_deref()
                            .or(token_address.as_deref())
                            .ok_or_else(|| {
                                Error::InvalidInput("--token or --token-address required".into())
                            })?;
                    let token_addr = Address::from_str(token_str).map_err(|e| {
                        Error::InvalidInput(format!("invalid --token address: {e}"))
                    })?;
                    let gas = gas_limit.unwrap_or(65_000);

                    let (n, p) = match (name.as_deref(), password.as_deref()) {
                        (Some(n), Some(p)) => (n, p),
                        _ => {
                            return Err(Error::InvalidInput(
                                "--name and --password required".into(),
                            ));
                        }
                    };
                    let net = Network::parse_cli(&network)
                        .map_err(|e| Error::InvalidInput(e.to_string()))?;
                    let wallet_id = mgr
                        .lookup_by_name(n, net)
                        .map_err(crate::handlers::map_wallet_err)?;
                    let signer = mgr
                        .unlock_signer(wallet_id, p.as_bytes())
                        .map_err(crate::handlers::map_wallet_err)?;

                    wallet_send_erc20(&provider, &signer, token_addr, to_addr, amount_wei, gas)
                        .await
                }
                _ => Err(eth_wallet_core::Error::Rpc(
                    "erc20 non-Send action deferred past #337".into(),
                )),
            },
        }
    })
}
