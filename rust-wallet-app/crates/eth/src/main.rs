//! `eth` CLI binary — Issue #309 (Task 10).
//!
//! Initial scaffold: clap subcommand LAYOUT per the 27-story spec.
//! Business logic per subcommand lands in follow-up issues once Task 11
//! (Sepolia smoke) shows the underlying library stable.
//!
//! ## Subcommand structure (per Issue #309)
//!
//! - `wallet create --name --network --rpc-url --derivation-path`
//! - `wallet import --name --mnemonic | --private-key --network`
//! - `wallet balance --network --rpc-url --unit --human`
//! - `wallet sync --network --rpc-url`
//! - `wallet list | show | delete`
//! - `send --to --amount --network --rpc-url --fee | --max-fee-gwei --priority-fee-gwei
//!       --nonce --gas-limit --batch | --drain | --speed-up --dry-run --wait`
//! - `tx list --since-block --limit --pending` / `tx get --tx-hash`
//! - `fee --network --rpc-url`
//! - `config show --json`
//! - `sign-message --mnemonic --message --address --verify` (Story 18)
//! - `sign-typed --mnemonic --typed-data|--typed-data-file --address --verify` (Story 27)
//! - `erc20 balance --token|--all --network --rpc-url --json` (Story 22)
//! - `erc20 send --token|--token-address --to --amount --gas-limit` (Story 21)
//! - `erc20 list --json [--include-bundled]` (Story 23)
//! - `erc20 register --address|--list|--remove --symbol` (Story 24)
//! - `erc20 approve --token --spender --amount|--amount unlimited|max` (Story 25)
//! - `erc20 deploy --token-name --token-symbol --decimals` (Story 26, anvil-only)

use clap::{Parser, Subcommand};

/// Stable exit codes per #297 M11 (forwarded from
/// `eth-wallet-core::error::Error::exit_code()`).
///
/// | Code | Meaning                                                          |
/// | ---- | ---------------------------------------------------------------- |
/// | 0    | success                                                          |
/// | 1    | user abort (confirm_prompt declined)                             |
/// | 2    | bad input (CLI flag parsing, validation)                         |
/// | 3    | RPC / upstream error                                              |
/// | 4    | wallet / balance issue                                            |
/// | 5    | signing / broadcast error                                         |
///
/// The CLI maps errors in `main.rs::run()` to these; `std::process::ExitCode`
/// carries them to the shell.
#[derive(Parser, Debug)]
#[command(name = "eth", version, about = "Ethereum wallet CLI (alloy v1.8.x)")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Default RPC URL (overrides per-subcommand).
    /// Per #297 M10 SPKI pin is applied via the
    /// `provider::new_http_pinned` codepath (currently fail-closed per
    /// the PR #316 security review follow-up).
    #[arg(long, global = true, env = "ETH_RPC_URL")]
    rpc_url: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Wallet {
        #[command(subcommand)]
        action: WalletAction,
    },
    Send(SendArgs),
    Tx {
        #[command(subcommand)]
        action: TxAction,
    },
    Fee(FeeArgs),
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
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
        #[arg(long, default_value = "sepolia")]
        network: String,
        #[arg(long)]
        derivation_path: Option<String>,
    },
    Import {
        #[arg(long)]
        name: String,
        #[arg(long, conflicts_with = "private_key")]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with = "mnemonic")]
        private_key: Option<String>,
        #[arg(long, default_value = "sepolia")]
        network: String,
    },
    Balance {
        #[arg(long, default_value = "sepolia")]
        network: String,
        #[arg(long)]
        unit: Option<String>,
    },
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
    },
    Delete {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },
}

#[derive(clap::Args, Debug)]
struct SendArgs {
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
        token: Option<String>,
        #[arg(long)]
        token_address: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value = "0")]
        amount: String,
        #[arg(long)]
        gas_limit: Option<u64>,
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
    let exit_code = run(cli);
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> i32 {
    // v0.2 scaffold: each subcommand is a placeholder that confirms the
    // clap layout parses correctly. Business logic per Issue #309 ships in
    // follow-up PRs once the underlying library surfaces stabilize.
    match cli.command {
        Command::Version => {
            println!("eth {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Command::Wallet { action } => {
            println!("[v0.2 scaffold] wallet action: {:?}", action);
            0
        }
        Command::Send(args) => {
            println!("[v0.2 scaffold] send args: {:?}", args);
            0
        }
        Command::Tx { action } => {
            println!("[v0.2 scaffold] tx action: {:?}", action);
            0
        }
        Command::Fee(args) => {
            println!("[v0.2 scaffold] fee args: {:?}", args);
            0
        }
        Command::Config { action } => {
            println!("[v0.2 scaffold] config action: {:?}", action);
            0
        }
        Command::SignMessage {
            message,
            mnemonic,
            address,
            verify,
        } => {
            println!(
                "[v0.2 scaffold] sign-message: message_len={} mnemonic_provided={} address_provided={} verify={:?}",
                message.len(), mnemonic.is_some(), address.is_some(), verify
            );
            0
        }
        Command::SignTyped {
            typed_data,
            typed_data_file,
            mnemonic,
            address,
            verify,
        } => {
            println!(
                "[v0.2 scaffold] sign-typed: inline_len={} file_provided={} mnemonic_provided={} address_provided={} verify={:?}",
                typed_data.as_ref().map(|s| s.len()).unwrap_or(0),
                typed_data_file.is_some(),
                mnemonic.is_some(),
                address.is_some(),
                verify
            );
            0
        }
        Command::Erc20 { action } => {
            println!("[v0.2 scaffold] erc20 action: {:?}", action);
            0
        }
    }
}
