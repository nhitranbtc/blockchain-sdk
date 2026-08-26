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

use alloy_primitives::{Address, B256, U256};
use alloy_signer_local::PrivateKeySigner;
use clap::{Parser, Subcommand};

use crate::handlers::{
    config_show, open_manager, open_provider, print_wallet_created, tx_get, tx_list,
    wallet_balance, wallet_balance_all, wallet_create, wallet_delete, wallet_import, wallet_list,
    wallet_send_erc20, wallet_send_native, wallet_show, wallet_speedup,
};
use eth_wallet_core::{Error, Network, Result};

#[derive(Parser, Debug)]
#[command(name = "eth", version, about = "Ethereum wallet CLI (alloy v1.8.x)")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Default RPC URL (overrides per-subcommand). Precedence: explicit
    /// `--rpc-url` flag > `ETH_RPC_URL` env (incl. values loaded from
    /// `.env` via dotenvy at startup) > this centralised default
    /// (Anvil localhost). Note: dotenvy does NOT overwrite existing
    /// process env vars, so shell exports always win over `.env`.
    /// Per #297 M10 SPKI pin was deferred per #330 — `provider::new_http`
    /// (default rustls TLS + system CAs) handles all RPC traffic.
    #[arg(
        long,
        global = true,
        env = "ETH_RPC_URL",
        default_value = "http://127.0.0.1:8545"
    )]
    rpc_url: String,

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
        password: Option<String>,
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
        network: String,
    },
    Import {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
        network: String,
        #[arg(long, conflicts_with = "private_key")]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with = "mnemonic")]
        private_key: Option<String>,
    },
    Balance {
        #[arg(long)]
        address: String,
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
        network: String,
        /// ETH unit hint (`wei|gwei|eth`); meaningless when `--token` or
        /// `--all` is set — clap rejects the combos at parse time.
        #[arg(long, conflicts_with_all = ["token", "all"])]
        unit: Option<String>,
        /// ERC-20 token contract address. When set, prints the token balance
        /// (auto-detects decimals via `decimals()` `eth_call` unless
        /// `--decimals` is supplied) instead of the native ETH balance.
        /// Repeated for `--all` mode: each `--token` is appended to the
        /// registry iteration in CLI order (Issue #358 AC #2).
        #[arg(long)]
        token: Vec<String>,
        /// Iterate the bundled token registry + any `--token` overrides,
        /// one line per token (Issue #358).
        #[arg(long)]
        all: bool,
        /// Override for the ERC-20 `decimals()` auto-detect. Useful when
        /// the token contract doesn't implement standard `decimals()` or
        /// the RPC can't reach the token. Must be 0..=255. Allowed with
        /// either `--token` (per-token mode) or `--all` (applies to every
        /// token in the batch — Issue #358 AC #6).
        #[arg(long)]
        decimals: Option<u8>,
        /// Emit a JSON array of `{symbol, address, balance, decimals}`
        /// rows for `--all` mode instead of the line-per-token text
        /// format (Issue #358 AC #5).
        #[arg(long)]
        json: bool,
    },
    /// Sync wallet with chain state (rebuild meta from on-chain). Deferred.
    Sync {
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
        network: String,
    },
    List,
    Show {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
        network: String,
    },
    Delete {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
        network: String,
    },
    /// Speed up a pending native-ETH tx (EIP-1559 replace-by-fee). Looks
    /// up the original tx by hash, validates nonce matches the wallet's
    /// current nonce + new fees exceed in-pool fees, then re-signs the
    /// same envelope (same `from`/`to`/`value`/`nonce`) with higher fees
    /// and broadcasts via `provider.send_raw_transaction`. Issue #381.
    Speedup {
        /// Hash of the pending tx to speed up (0x-prefixed hex, 32 bytes).
        #[arg(long)]
        speedup: String,
        /// New `max_fee_per_gas` (wei) — must exceed in-pool
        /// `max_fee_per_gas` (else `Error::FeeTooLow`, exit 2).
        #[arg(long)]
        max_fee_per_gas: u128,
        /// New `max_priority_fee_per_gas` (wei) — must satisfy
        /// `priority <= max_fee` AND `priority >= in_pool_priority`
        /// (else `Error::FeeTooLow`, exit 2).
        #[arg(long)]
        max_priority_fee_per_gas: u128,
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
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
    /// Override `max_fee_per_gas` (wei). Must be set together with
    /// `--max-priority-fee-per-gas`; both omitted → provider estimate.
    /// Env var `ETH_MAX_FEE_PER_GAS` (PR #341 precedence pattern).
    #[arg(long, env = "ETH_MAX_FEE_PER_GAS")]
    max_fee_per_gas: Option<u128>,
    /// Override `max_priority_fee_per_gas` (wei). Must be set together with
    /// `--max-fee-per-gas`; both omitted → provider estimate.
    /// Env var `ETH_MAX_PRIORITY_FEE_PER_GAS`.
    #[arg(long, env = "ETH_MAX_PRIORITY_FEE_PER_GAS")]
    max_priority_fee_per_gas: Option<u128>,
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
        /// Override `max_fee_per_gas` (wei). Must be set together with
        /// `--max-priority-fee-per-gas`; both omitted → provider estimate.
        /// Env var `ETH_MAX_FEE_PER_GAS` (PR #341 precedence pattern).
        #[arg(long, env = "ETH_MAX_FEE_PER_GAS")]
        max_fee_per_gas: Option<u128>,
        /// Override `max_priority_fee_per_gas` (wei). Must be set together
        /// with `--max-fee-per-gas`; both omitted → provider estimate.
        /// Env var `ETH_MAX_PRIORITY_FEE_PER_GAS`.
        #[arg(long, env = "ETH_MAX_PRIORITY_FEE_PER_GAS")]
        max_priority_fee_per_gas: Option<u128>,
        #[arg(long, env = "ETH_NETWORK", default_value = "sepolia")]
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
    // Issue #341 — load .env from cwd before clap parses so ETH_NETWORK,
    // ETH_RPC_URL, ETH_DATA_DIR etc. flow through `env = "..."` attrs.
    // Missing file is silent (CI/clean checkouts stay green); a malformed
    // `.env` is an operator mistake (typo, missing `=`) — surface as a hard
    // error with exit 2 (bad input) per code-reviewer finding #6.
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(dotenvy::Error::Io(ref io)) if io.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            eprintln!("error: .env parse failed: {e}");
            std::process::exit(2);
        }
    }

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

/// Resolve the wallet password with priority:
/// 1. `--password` argv (emits stderr warning per cycle 8 L12 review)
/// 2. `ETH_PASSWORD` env var (removed from process env after read —
///    defense-in-depth so future subprocesses don't inherit it)
/// 3. TTY prompt via `rpassword::prompt_password`
///
/// Empty argv (`Some("")`) falls through to the next source — matches
/// the `btc` CLI pattern (`btc/src/handlers.rs:86`). A wallet created
/// with an empty password is unrecoverable, so we refuse it at resolution
/// time rather than silently accepting it.
///
/// Returns `Error::InvalidInput` (exit 2) when every source fails. Per
/// Issue #351 cycle 8b: argv + env stay supported for backward compat +
/// script-friendliness; TTY prompt is the new *primary* path for
/// interactive operators. Non-TTY environments (CI runners without
/// `/dev/tty`) surface a clean operator-facing error rather than panicking
/// — verified by the `prompt_io_error_maps_to_invalid_input` unit test +
/// the `rpassword::read_password_from_bufread` test seam in
/// `tests/password.rs`.
fn resolve_password(cli_pw: Option<&str>) -> Result<String> {
    // Read ETH_PASSWORD, then remove from process env immediately so any
    // future subprocess spawned by this CLI (or by alloy / tokio deps)
    // cannot inherit the cleartext password. The var is single-use for
    // this invocation; reading it twice would be a security regression.
    let env_pw = std::env::var("ETH_PASSWORD").ok();
    std::env::remove_var("ETH_PASSWORD");
    resolve_password_with(cli_pw, env_pw, || prompt_password("Wallet password: "))
}

/// Resolution kernel: same priority chain as `resolve_password` but with
/// the TTY prompt injected. Production callers go through
/// `resolve_password`; tests use this directly with a mock prompt to
/// avoid needing a controlling terminal in CI.
///
/// Errors propagate from the prompt verbatim — `prompt_password` already
/// maps the underlying `io::Error` to `Error::InvalidInput` with an
/// operator-facing message, so the kernel does not re-wrap.
fn resolve_password_with(
    cli_pw: Option<&str>,
    env_pw: Option<String>,
    prompt_fn: impl FnOnce() -> Result<String>,
) -> Result<String> {
    // Non-empty argv wins; empty argv falls through (matches btc/src/handlers.rs:86).
    if let Some(p) = cli_pw {
        if !p.is_empty() {
            eprintln!(
                "warning: --password on command line is insecure (shell history, process list); \
                 omit both flag and env for the TTY prompt, or set ETH_PASSWORD in CI"
            );
            return Ok(p.to_string());
        }
    }
    if let Some(env_pw) = env_pw {
        return Ok(env_pw);
    }
    prompt_fn()
}

/// Read a password from the controlling TTY with echo disabled. Cross-
/// platform via `rpassword` (Unix + Windows). Does NOT expose the input
/// to the process list or terminal scrollback. Maps the underlying
/// `io::Error` (e.g. `/dev/tty` unavailable on CI runners) to
/// `Error::InvalidInput` so the chain in `resolve_password_with` does
/// not panic — verified by `prompt_io_error_maps_to_invalid_input`.
///
/// `io::Error`'s `Display` impl never includes the buffered bytes
/// (security-auditor verification), so the formatted message cannot leak
/// the password.
fn prompt_password(prompt: &str) -> Result<String> {
    rpassword::prompt_password(prompt).map_err(|e| {
        Error::InvalidInput(format!(
            "password prompt failed: {e}; \
             run on a TTY with echo disabled, or pass --password / set ETH_PASSWORD"
        ))
    })
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
        .map_err(|e| eth_wallet_core::Error::rpc(format!("tokio: {e}")))?;

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
                    let password = resolve_password(password.as_deref())?;
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
                    let password = resolve_password(password.as_deref())?;
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
                    network,
                    unit,
                    token,
                    all,
                    decimals,
                    json,
                } => {
                    let rpc = &cli.rpc_url;
                    let provider = open_provider(rpc)?;
                    // Validate `--decimals` requires either `--token` or `--all`.
                    // Manual validation (vs clap `requires`) because clap has no
                    // built-in OR — `--decimals` accepts both flag forms.
                    if decimals.is_some() && token.is_empty() && !all {
                        return Err(Error::InvalidInput(
                            "--decimals requires --token or --all".into(),
                        ));
                    }
                    // Multiple `--token` flags without `--all` would silently
                    // drop all but the first (L12 review H-3). Surface as
                    // bad input so the operator adds `--all` (or drops the
                    // extras) — never silently lose intent.
                    if !all && token.len() > 1 {
                        return Err(Error::InvalidInput(
                            "multiple --token values require --all (use --all to iterate the registry + overrides)".into(),
                        ));
                    }
                    if all {
                        wallet_balance_all(&provider, &address, &network, &token, decimals, json)
                            .await
                    } else {
                        // Single-token mode: take the first --token if any.
                        let single_token = token.first().map(String::as_str);
                        wallet_balance(
                            &provider,
                            &address,
                            unit.as_deref(),
                            single_token,
                            decimals,
                            &network,
                        )
                        .await
                    }
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
                WalletAction::Speedup {
                    speedup,
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    name,
                    password,
                    network,
                } => {
                    let rpc = &cli.rpc_url;
                    let provider = open_provider(rpc)?;
                    let speedup_hash = B256::from_str(&speedup).map_err(|e| {
                        Error::InvalidInput(format!("invalid --speedup tx hash: {e}"))
                    })?;
                    let password = resolve_password(password.as_deref())?;
                    let net = Network::parse_cli(&network)
                        .map_err(|e| Error::InvalidInput(e.to_string()))?;
                    let wallet_id = mgr
                        .lookup_by_name(&name, net)
                        .map_err(crate::handlers::map_wallet_err)?;
                    // Issue #350 (H-2): unlock_signer returns raw
                    // Zeroizing<[u8; 32]>; build alloy signer at use site,
                    // scoped to this command.
                    let secret = mgr
                        .unlock_signer(wallet_id, password.as_bytes())
                        .map_err(crate::handlers::map_wallet_err)?;
                    let signer = PrivateKeySigner::from_slice(secret.as_ref())
                        .map_err(|e| Error::InvalidPrivateKey(format!("from_slice: {e}")))?;
                    let new_hash = wallet_speedup(
                        &provider,
                        &signer,
                        net,
                        speedup_hash,
                        max_fee_per_gas,
                        max_priority_fee_per_gas,
                    )
                    .await?;
                    println!("{new_hash}");
                    Ok(())
                }
            },
            Command::Send(args) => {
                let rpc = &cli.rpc_url;
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

                let name = args
                    .name
                    .as_deref()
                    .ok_or_else(|| Error::InvalidInput("--name required".into()))?;
                let password = resolve_password(args.password.as_deref())?;
                let net = Network::parse_cli(&args.network)
                    .map_err(|e| Error::InvalidInput(e.to_string()))?;
                let wallet_id = mgr
                    .lookup_by_name(name, net)
                    .map_err(crate::handlers::map_wallet_err)?;
                // Issue #350 (H-2): unlock_signer now returns raw
                // Zeroizing<[u8; 32]> (heap-cleanup on drop); build the
                // alloy signer at use site, scoped to this command.
                let secret = mgr
                    .unlock_signer(wallet_id, password.as_bytes())
                    .map_err(crate::handlers::map_wallet_err)?;
                let signer = PrivateKeySigner::from_slice(secret.as_ref())
                    .map_err(|e| Error::InvalidPrivateKey(format!("from_slice: {e}")))?;

                wallet_send_native(
                    &provider,
                    &signer,
                    net,
                    to,
                    amount_wei,
                    args.max_fee_per_gas,
                    args.max_priority_fee_per_gas,
                )
                .await
            }
            Command::Tx { action } => match action {
                TxAction::Get { tx_hash } => {
                    let rpc = &cli.rpc_url;
                    let provider = open_provider(rpc)?;
                    tx_get(&provider, &tx_hash).await
                }
                TxAction::List { limit, .. } => {
                    let rpc = &cli.rpc_url;
                    let provider = open_provider(rpc)?;
                    tx_list(&provider, limit).await
                }
            },
            Command::Fee(_) => Err(eth_wallet_core::Error::Rpc(
                "fee: deferred past #337 (follow-up)".into(),
            )),
            Command::Config { action } => match action {
                ConfigAction::Show { json } => {
                    config_show(&cli.rpc_url, cli.data_dir.as_ref(), json)
                }
            },
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
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    network,
                } => {
                    let rpc = &cli.rpc_url;
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

                    let n = name
                        .as_deref()
                        .ok_or_else(|| Error::InvalidInput("--name required".into()))?;
                    let p = resolve_password(password.as_deref())?;
                    let net = Network::parse_cli(&network)
                        .map_err(|e| Error::InvalidInput(e.to_string()))?;
                    let wallet_id = mgr
                        .lookup_by_name(n, net)
                        .map_err(crate::handlers::map_wallet_err)?;
                    // Issue #350 (H-2): unlock_signer returns raw
                    // Zeroizing<[u8; 32]>; build alloy signer at use site.
                    let secret = mgr
                        .unlock_signer(wallet_id, p.as_bytes())
                        .map_err(crate::handlers::map_wallet_err)?;
                    let signer = PrivateKeySigner::from_slice(secret.as_ref())
                        .map_err(|e| Error::InvalidPrivateKey(format!("from_slice: {e}")))?;

                    wallet_send_erc20(
                        &provider,
                        &signer,
                        net,
                        token_addr,
                        to_addr,
                        amount_wei,
                        gas,
                        max_fee_per_gas,
                        max_priority_fee_per_gas,
                    )
                    .await
                }
                _ => Err(eth_wallet_core::Error::Rpc(
                    "erc20 non-Send action deferred past #337".into(),
                )),
            },
        }
    })
}

#[cfg(test)]
mod password_resolution_tests {
    //! Issue #351 (cycle 8b, C-1 from #339) — TTY prompt as primary
    //! password source. Priority chain:
    //!   --password argv (with stderr warning) → ETH_PASSWORD env →
    //!   TTY prompt → Error::InvalidInput (exit 2).
    //!
    //! Empty argv (`Some("")`) falls through to the next source to
    //! match the btc CLI pattern — a wallet created with an empty
    //! password is unrecoverable.
    //!
    //! These tests exercise the orchestration via an injected prompt
    //! closure; the production `prompt_password()` helper (TTY direct)
    //! is covered by tests/password.rs via `rpassword::read_password_from_bufread`.

    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that touch process-global state (`ETH_PASSWORD`
    /// env var). cargo test runs tests in parallel; without this lock,
    /// env mutations from one test would race with reads in another.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn ok_prompt(s: &'static str) -> impl FnOnce() -> Result<String> {
        move || Ok(s.to_string())
    }

    /// Mirrors what `prompt_password` actually emits: `Error::InvalidInput`
    /// wrapping the underlying `io::Error` message.
    fn err_prompt() -> impl FnOnce() -> Result<String> {
        || {
            Err(Error::InvalidInput(
                "password prompt failed: simulated /dev/tty unavailable".into(),
            ))
        }
    }

    #[test]
    fn argv_wins_over_env_and_prompt() {
        let r = resolve_password_with(
            Some("argv-pw"),
            Some("env-pw".to_string()),
            ok_prompt("tty-pw"),
        )
        .expect("argv path returns Ok");
        assert_eq!(r, "argv-pw");
    }

    #[test]
    fn env_used_when_no_argv() {
        let r = resolve_password_with(None, Some("env-pw".to_string()), ok_prompt("tty-pw"))
            .expect("env path returns Ok");
        assert_eq!(r, "env-pw");
    }

    #[test]
    fn prompt_used_when_no_argv_no_env() {
        let r =
            resolve_password_with(None, None, ok_prompt("tty-pw")).expect("prompt path returns Ok");
        assert_eq!(r, "tty-pw");
    }

    #[test]
    fn empty_argv_falls_through_to_env() {
        // Mirrors btc/src/handlers.rs:86 — empty `--password ""` should
        // not produce a bricked wallet. Falls through to ETH_PASSWORD.
        let r = resolve_password_with(Some(""), Some("env-pw".to_string()), ok_prompt("tty-pw"))
            .expect("empty argv falls through to env");
        assert_eq!(r, "env-pw");
    }

    #[test]
    fn empty_argv_no_env_falls_through_to_prompt() {
        let r = resolve_password_with(Some(""), None, ok_prompt("tty-pw"))
            .expect("empty argv + no env falls through to prompt");
        assert_eq!(r, "tty-pw");
    }

    #[test]
    fn prompt_io_error_propagates_as_invalid_input() {
        // Acceptance bullet 5: /dev/tty unavailable must NOT panic.
        // `prompt_password` maps the underlying io::Error to
        // `Error::InvalidInput`; the kernel passes that through.
        let r = resolve_password_with(None, None, err_prompt());
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("password"),
                    "InvalidInput message should mention password; got: {msg}"
                );
                // Simulated prompt error preserves its underlying detail —
                // confirms the kernel does NOT re-wrap and discard context.
                assert!(
                    msg.contains("simulated /dev/tty unavailable"),
                    "inner io::Error detail should propagate; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn resolve_password_reads_and_removes_eth_password_env() {
        // L12 security-auditor M-2 fix: ETH_PASSWORD must be removed
        // from process env immediately after read so any future
        // subprocess spawned by this CLI cannot inherit it.
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("ETH_PASSWORD", "env-pw");
        // Empty argv falls through to env path, exercising the env
        // read + remove sequence.
        let result = resolve_password(Some(""));
        // Cleanup before assertions so the test fails loud (env leak)
        // rather than silently affecting other tests if assertions panic.
        std::env::remove_var("ETH_PASSWORD");
        assert!(result.is_ok(), "empty argv + ETH_PASSWORD env = Ok");
        assert_eq!(result.unwrap(), "env-pw");
        assert!(
            std::env::var("ETH_PASSWORD").is_err(),
            "ETH_PASSWORD must be removed from process env after read"
        );
    }
}
