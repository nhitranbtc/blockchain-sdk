//! `polygon` CLI binary — Issue #426 / Phase 4 of #416.
//!
//! T6b (L25 sub-task split): clap tree + dispatch scaffold. The clap
//! subcommand types live in `cli.rs`; `run()` matches each `Command` variant
//! to a per-handler stub. Handler BODIES land in T6c/T6d (per L25 split).
//! Round 1 critical-tier helpers (`resolve_password`, etc.) remain
//! available; dispatch to them lands in T6c.
//!
//! See `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! for the full T6 surface (31 user-stories + 3 cross-cutting flags).

mod cli;
mod handlers;

/// Resolution kernel: argv → env → TTY prompt priority chain.
///
/// Production callers go through `resolve_password` (which removes
/// `POLYGON_PASSWORD` from process env after read per L54); tests inject
/// a mock prompt closure to avoid needing a controlling terminal in CI.
///
/// Mirrors `eth/src/main.rs:439-458` per design doc §5.1. Returns errors
/// from `prompt_fn` verbatim — the kernel does not re-wrap.
#[allow(dead_code)] // wired into resolve_password wrapper below (L13 fix #6)
fn resolve_password_with(
    cli_pw: Option<&str>,
    env_pw: Option<String>,
    prompt_fn: impl FnOnce() -> polygon_wallet_core::Result<String>,
) -> polygon_wallet_core::Result<String> {
    // Non-empty argv wins; empty argv falls through to env (matches
    // `btc/src/handlers.rs:86` — a wallet created with an empty password
    // is unrecoverable, so we refuse it at resolution time rather than
    // silently accepting it).
    if let Some(p) = cli_pw {
        if !p.is_empty() {
            eprintln!(
                "warning: --password on command line is insecure (shell history, process list); \
                 omit both flag and env for the TTY prompt, or set POLYGON_PASSWORD in CI"
            );
            return Ok(p.to_string());
        }
    }
    if let Some(env_pw) = env_pw {
        return Ok(env_pw);
    }
    prompt_fn()
}

/// Resolve wallet password with priority: argv → env → TTY prompt.
///
/// Reads `POLYGON_PASSWORD` then removes it from process env immediately
/// so any future subprocess spawned by this CLI (or by alloy / tokio
/// deps) cannot inherit the cleartext password (L54 defense-in-depth).
/// The var is single-use for this invocation; reading it twice would be
/// a security regression.
///
/// **Threading assumption (L13 Round 1 fix #6 / review M-3):** must be
/// invoked synchronously *before* any tokio runtime is built or any
/// `tokio::spawn` / `Command::new` happens. A thread spawned before
/// the `remove_var` call still inherits the env var (Unix: env vars
/// are copied at fork / exec time; `std::env::remove_var` mutates only
/// the leader thread). Future async dispatch (T6 follow-up Batch B)
/// must call `resolve_password` BEFORE entering the tokio runtime.
///
/// Mirrors `eth/src/main.rs:421-429`. Returns `Error::InvalidInput`
/// (exit 2) when every source fails.
#[allow(dead_code)] // wired into fn main() in T6 follow-up (before tokio runtime)
fn resolve_password(cli_pw: Option<&str>) -> polygon_wallet_core::Result<String> {
    let env_pw = std::env::var("POLYGON_PASSWORD").ok();
    std::env::remove_var("POLYGON_PASSWORD");
    resolve_password_with(cli_pw, env_pw, || prompt_password("Wallet password: "))
}

/// Stub: TTY prompt placeholder. Real impl uses `rpassword::prompt_password`
/// in a follow-up commit (rpassword dep added in Batch B). Returns
/// `Error::InvalidInput` so the kernel propagates without panicking if
/// the closure is ever invoked before the real impl lands.
///
/// L13 Round 1 fix #6 (review L1): `_prompt` underscore prefix makes the
/// unused status explicit. The real impl will forward it to
/// `rpassword::prompt_password(_prompt)`.
#[allow(dead_code)] // wired into resolve_password wrapper in T6 follow-up
fn prompt_password(_prompt: &str) -> polygon_wallet_core::Result<String> {
    Err(polygon_wallet_core::Error::InvalidInput(
        "prompt_password: stub — rpassword::prompt_password impl lands in Batch B".into(),
    ))
}

fn main() -> std::process::ExitCode {
    // T6c1 follow-up: wrap `run()` (async) in a tokio current-thread
    // runtime via `block_on` (mirrors `eth/src/main.rs:487-491`). Sync
    // commands (list / show / delete / create / import) also route
    // through `block_on` — a no-op for sync fns. Async commands
    // (wallet_balance / future send / sync) drive real work.
    use clap::Parser;
    let cli = cli::Cli::parse();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime (single-threaded, full-featured)");
    match rt.block_on(run(cli)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            // T6b: all errors map to exit 1. Per-error mapping via
            // `Error::exit_code()` lands in T6c when the error table
            // becomes meaningful (currently every error is `Error::Rpc`).
            std::process::ExitCode::from(1)
        }
    }
}

/// T6b dispatch scaffold: match on `Command`, route to per-handler
/// stub. Handler bodies land in T6c (wallet/tx/erc20/send/speedup)
/// and T6d (sign/fee/config/faucet).
/// Format a balance (U256 wei) as a human-readable string per the
/// `--unit` flag. `wei` (default per design §3.4) returns the raw U256.
/// `pol` converts via 1e18 wei with 18-decimal precision and trims
/// trailing zeros. Unknown units fall back to wei.
fn format_balance(balance: alloy_primitives::U256, unit: &str) -> String {
    match unit {
        "pol" => {
            // 1 POL = 1e18 wei. Convert with full 18-decimal precision.
            let one_e18 =
                alloy_primitives::U256::from(10u128).pow(alloy_primitives::U256::from(18u8));
            let whole = balance / one_e18;
            let frac = balance % one_e18;
            let mut s = format!("{}.{:018}", whole, frac.to_string());
            // Trim trailing zeros (but keep at least one decimal digit).
            while s.ends_with('0') && s.contains('.') && !s.ends_with(".0") {
                s.pop();
            }
            format!("{s} POL")
        }
        _ => format!("{balance} wei"),
    }
}

/// Resolve the default wallet data directory: `$XDG_DATA_HOME/polygon/`
/// (Linux) / platform-equivalent via the `directories` crate. Used when
/// `--data-dir` is not provided on the CLI.
fn default_data_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("io", "polygon-cli", "polygon")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_default()
}

/// T6c1 follow-up: `run()` is now `async fn` so it can drive the
/// `wallet_balance` async handler (and future async handlers — T6c3
/// `wallet_sync`, T6c5 `wallet_send_*`). `main()` wraps `run()` in a
/// tokio current-thread runtime via `block_on` (mirrors `eth/src/main.rs:487-491`).
async fn run(cli: cli::Cli) -> polygon_wallet_core::Result<()> {
    use alloy_primitives::{utils::parse_units, Address, U256};
    use cli::{Command, ConfigAction, Erc20Action, TxAction, WalletAction};
    use polygon_wallet_core::Error;
    use std::str::FromStr;
    use zeroize::Zeroizing;

    let stub = |cmd: &'static str| -> polygon_wallet_core::Result<()> {
        Err(Error::Rpc(format!(
            "{cmd}: deferred past T6b — landing in T6c/T6d"
        )))
    };

    match cli.command {
        Command::Version => {
            println!("polygon {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Wallet { action } => match action {
            WalletAction::Create {
                name,
                password,
                network,
                derivation_path: _,
                account_index: _,
                legacy_token_symbol: _,
                rpc_url: _,
            } => {
                // T6c4 follow-up: dispatch `wallet create` to the real
                // `handlers::wallet::wallet_create` (PR #451). Password
                // is read via `resolve_password` (env / argv / TTY
                // priority chain per L54 defense-in-depth) and wrapped
                // in `Zeroizing<Vec<u8>>` before crossing the FFI
                // boundary. Address + wallet_id are echoed to stdout so
                // operators (and the integration test #438) can copy
                // the address into subsequent commands.
                let net = handlers::parse_network(&network)?;
                let pw_string = resolve_password(password.as_deref())?;
                let password = Zeroizing::new(pw_string.into_bytes());
                let data_dir: std::path::PathBuf =
                    cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let created = handlers::wallet::wallet_create(&data_dir, &name, &password, net)?;
                println!(
                    "wallet created: name={name} id={} address=0x{}",
                    created.wallet_id,
                    alloy_primitives::hex::encode(created.address.as_slice()),
                );
                Ok(())
            }
            WalletAction::Import {
                name,
                password,
                network,
                mnemonic,
                private_key: _,
                account_index: _,
                legacy_token_symbol: _,
                rpc_url: _,
            } => {
                // T6c4 follow-up: dispatch `wallet import` to the real
                // `handlers::wallet::wallet_import` (PR #451). Mnemonic
                // phrase comes from `--mnemonic` (private-key import
                // deferred per the lib's hardcoded
                // `Network::default_v0_2()` gap).
                let net = handlers::parse_network(&network)?;
                let phrase = mnemonic.ok_or_else(|| {
                    Error::InvalidInput("--mnemonic required (--private-key deferred)".into())
                })?;
                let pw_string = resolve_password(password.as_deref())?;
                let password = Zeroizing::new(pw_string.into_bytes());
                let data_dir: std::path::PathBuf =
                    cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let created =
                    handlers::wallet::wallet_import(&data_dir, &name, &password, net, &phrase)?;
                println!(
                    "wallet imported: name={name} id={} address=0x{}",
                    created.wallet_id,
                    alloy_primitives::hex::encode(created.address.as_slice()),
                );
                Ok(())
            }
            WalletAction::Sync {
                network,
                rpc_url,
                address,
                json,
            } => {
                // T6c3 follow-up #3: dispatch to the real `wallet_sync`
                // async handler returning `Vec<TxSummary>`. Live RPC
                // body (the `provider.get_logs` call) deferred to T7
                // operator-driven integration per L29 — handler returns
                // `Error::Rpc("deferred to T7")` until then. --json
                // formatter wired here for when T7 lands.
                let net = handlers::parse_network(&network)?;
                // cli.rs WalletAction::Sync.address is `Address` (parsed
                // via `parse_address`); handler expects &str. Convert to
                // EIP-55 checksum so any downstream receipt matches the
                // canonical on-chain encoding.
                let addr_str = format!("{:#x}", address);
                let summaries =
                    handlers::wallet::wallet_sync(rpc_url.as_deref(), net, &addr_str).await?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&summaries).unwrap_or_else(|_| "[]".into())
                    );
                } else {
                    for s in &summaries {
                        println!(
                            "block {} tx {} from {} to {} value {}",
                            s.block_number, s.tx_hash, s.from, s.to, s.value
                        );
                    }
                    if summaries.is_empty() {
                        println!("(no transfers found)");
                    }
                }
                Ok(())
            }
            WalletAction::List {
                network,
                all: _,
                json,
            } => {
                let net = handlers::parse_network(&network)?;
                let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let names = handlers::wallet::wallet_list(&data_dir, net)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&names).unwrap_or_else(|_| "[]".into())
                    );
                } else {
                    for name in &names {
                        println!("{name}");
                    }
                    if names.is_empty() {
                        eprintln!("(no wallets in {})", net.as_dir_name());
                    }
                }
                Ok(())
            }
            WalletAction::Show {
                network,
                id,
                name: _,
                addresses: _,
                export: _,
                json,
            } => {
                // T6c3 follow-up: dispatch to the real `wallet_show`
                // handler (reads .meta.json plaintext — encrypted blob
                // inspection deferred to T6d when rpassword + AES-GCM
                // decryption wires up). --id required (--name look-up
                // deferred).
                let net = handlers::parse_network(&network)?;
                let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let wallet_id =
                    id.ok_or_else(|| Error::InvalidInput("--id required for wallet show".into()))?;
                let info = handlers::wallet::wallet_show(&data_dir, net, wallet_id.as_str())?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&info).unwrap_or_else(|_| "{}".into())
                    );
                } else {
                    println!("wallet: {}", info.name);
                    println!("id: {wallet_id}");
                    // addresses / export deferred to T6c3 follow-up
                    // (requires decrypt + Zeroizing wrap).
                }
                Ok(())
            }
            WalletAction::Delete {
                network,
                id,
                name: _,
            } => {
                // T6c3: dispatch to the real `wallet_delete` handler.
                // Wallet ID comes from `--id` (preferred) or `--name`
                // (look up first, deferred to T6c3 follow-up). For now
                // `--id` is required.
                let net = handlers::parse_network(&network)?;
                let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let wallet_id = id
                    .ok_or_else(|| Error::InvalidInput("--id required for wallet delete".into()))?;
                handlers::wallet::wallet_delete(&data_dir, net, wallet_id.as_str())?;
                println!("wallet deleted: {wallet_id}");
                Ok(())
            }
            WalletAction::Balance {
                address,
                network: _,
                unit,
                legacy_token_symbol: _,
                rpc_url,
            } => {
                // T6c1 follow-up: dispatch to the real async
                // `wallet_balance` handler (PR #439). Unit-aware formatter
                // (`--unit pol`) lands in a subsequent commit once
                // operator UX testing surfaces the right format (wei is
                // the canonical unit; POL = wei / 1e18 is the conversion).
                let balance = handlers::wallet::wallet_balance(
                    rpc_url.as_deref(),
                    &format!("{:#x}", address),
                )
                .await?;
                println!("{}", format_balance(balance, &unit));
                Ok(())
            }
            WalletAction::Send(args) => {
                // T6c5: dispatch to real `wallet_send_native_v2`.
                // Per-action --rpc-url overrides global --rpc-url flag;
                // --data-dir defaults to platform XDG dir when absent.
                // Pattern-destructure accesses the cli::SendArgs fields
                // (private — destructure works without `pub`).
                let cli::SendArgs {
                    name,
                    password,
                    to,
                    amount,
                    network,
                    unit,
                    batch: _,
                    drain,
                    nonce,
                    gas_limit,
                    fee,
                    max_fee_gwei,
                    priority_fee_gwei,
                    dry_run,
                    wait,
                    rpc_url: action_rpc_url,
                } = args;
                let network = handlers::parse_network(&network)?;
                handlers::validate_wallet_name(&name)?;
                let pw_string = resolve_password(password.as_deref())?;
                let password = Zeroizing::new(pw_string.into_bytes());
                let amount_wei_string = match unit.as_str() {
                    "wei" => amount.clone(),
                    "pol" => {
                        let u = parse_units(&amount, 18).map_err(|e| {
                            Error::InvalidInput(format!("invalid --amount (pol): {e}"))
                        })?;
                        format!("{u}")
                    }
                    other => {
                        return Err(Error::InvalidInput(format!(
                            "unsupported --unit '{other}'; expected 'wei' or 'pol'"
                        )));
                    }
                };
                let data_dir: std::path::PathBuf =
                    cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let rpc_url = action_rpc_url.as_deref().or(cli.rpc_url.as_deref());
                let tx_hash = handlers::wallet::wallet_send_native_v2(
                    &data_dir,
                    rpc_url,
                    network,
                    &name,
                    &password,
                    &format!("{:#x}", to),
                    &amount_wei_string,
                    "wei",
                    nonce,
                    gas_limit,
                    &fee,
                    max_fee_gwei,
                    priority_fee_gwei,
                    drain,
                    dry_run,
                    wait,
                )
                .await?;
                println!(
                    "tx_hash: 0x{}",
                    alloy_primitives::hex::encode(tx_hash.as_slice())
                );
                Ok(())
            }
            WalletAction::SendSpeedup(args) => {
                let cli::SendSpeedupArgs {
                    tx_hash,
                    max_fee_gwei,
                    priority_fee_gwei,
                    name,
                    password,
                    network,
                    rpc_url: action_rpc_url,
                } = args;
                let network = handlers::parse_network(&network)?;
                handlers::validate_wallet_name(&name)?;
                let pw_string = resolve_password(password.as_deref())?;
                let password = Zeroizing::new(pw_string.into_bytes());
                let data_dir: std::path::PathBuf =
                    cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let rpc_url = action_rpc_url.as_deref().or(cli.rpc_url.as_deref());
                // CLI inputs are gwei floats → convert to wei u128.
                let new_max_fee_per_gas = (max_fee_gwei * 1e9) as u128;
                let new_max_priority_fee_per_gas = (priority_fee_gwei * 1e9) as u128;
                let tx_hash = handlers::wallet::wallet_send_speedup_v2(
                    &data_dir,
                    rpc_url,
                    network,
                    &name,
                    &password,
                    &tx_hash,
                    new_max_fee_per_gas,
                    new_max_priority_fee_per_gas,
                )
                .await?;
                println!(
                    "tx_hash: 0x{}",
                    alloy_primitives::hex::encode(tx_hash.as_slice())
                );
                Ok(())
            }
        },
        Command::Tx { action } => match action {
            TxAction::List {
                address,
                network: _,
                since_block,
                limit,
                json,
            } => {
                // T6d-3 (Issue #426 / Story 7): dispatch to the real
                // `handlers::tx::tx_list` handler. Pure arg validation
                // lives in the handler; live `provider.get_logs` scan
                // deferred to T7 operator-driven Amoy smoke per L29.
                handlers::tx::tx_list(&address, since_block, limit, json).await?;
                Ok(())
            }
            TxAction::Get {
                tx_hash,
                network: _,
                json,
                rpc_url: _,
            } => {
                // T6d-3 (Issue #426 / Story 7): dispatch to the real
                // `handlers::tx::tx_get` handler. Defensive B256 parse
                // in the handler; live `provider.get_transaction_by_hash`
                // deferred to T7 operator-driven Amoy smoke per L29.
                handlers::tx::tx_get(&tx_hash, json).await?;
                Ok(())
            }
        },
        Command::Erc20 { action } => match action {
            Erc20Action::Send {
                name,
                password,
                token,
                token_address,
                to,
                amount,
                network,
                gas_limit,
                max_fee_gwei,
                priority_fee_gwei,
                dry_run,
                rpc_url: action_rpc_url,
            } => {
                let network = handlers::parse_network(&network)?;
                let token_addr = match token_address {
                    Some(a) => handlers::erc20::resolve_token_address(&a, network)?,
                    None => handlers::erc20::resolve_token_address(&token, network)?,
                };
                let to_addr = Address::from_str(&to)
                    .map_err(|e| Error::InvalidInput(format!("invalid --to: {e}")))?;
                let amount_raw = U256::from_str_radix(&amount, 10).map_err(|e| {
                    Error::InvalidInput(format!("invalid --amount (erc20, base units wei): {e}"))
                })?;
                handlers::validate_wallet_name(&name)?;
                let pw_string = resolve_password(password.as_deref())?;
                let pw = Zeroizing::new(pw_string.into_bytes());
                let data_dir: std::path::PathBuf =
                    cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let rpc_url = action_rpc_url.as_deref();
                let tx_hash = handlers::erc20::erc20_send(
                    &data_dir,
                    rpc_url,
                    network,
                    &name,
                    &pw,
                    token_addr,
                    to_addr,
                    amount_raw,
                    gas_limit,
                    max_fee_gwei,
                    priority_fee_gwei,
                    dry_run,
                )
                .await?;
                println!(
                    "tx_hash: 0x{}",
                    alloy_primitives::hex::encode(tx_hash.as_slice())
                );
                Ok(())
            }
            Erc20Action::Balance { .. } => {
                // T6d-2 follow-up: Balance handler requires a
                // standalone refactor (cli.rs Balance.address typed
                // String vs value_parser=parse_address returning
                // Address; needs deeper cli.rs surgery than one PR).
                Err(Error::Rpc(
                    "erc20 balance: deferred to T6d-2.1 follow-up (cli.rs Balance.address type conflict)".into(),
                ))
            }
            Erc20Action::List { network, json } => {
                let network = handlers::parse_network(&network)?;
                handlers::erc20::erc20_list(network, json)?;
                Ok(())
            }
            Erc20Action::Register { .. } => {
                // T6d-2 follow-up: XDG-persisted user token registry is
                // heavier scope than one PR.
                Err(Error::Rpc(
                    "erc20 register: deferred to T6d-2.2 follow-up (XDG-persisted user registry)"
                        .into(),
                ))
            }
            Erc20Action::Approve {
                name,
                password,
                token,
                spender,
                amount,
                unlimited,
                network,
                gas_limit,
                max_fee_gwei,
                priority_fee_gwei,
                dry_run,
                rpc_url: action_rpc_url,
            } => {
                let network = handlers::parse_network(&network)?;
                let token_addr = handlers::erc20::resolve_token_address(&token, network)?;
                let spender_addr = Address::from_str(&spender)
                    .map_err(|e| Error::InvalidInput(format!("invalid --spender: {e}")))?;
                let amount_raw = U256::from_str_radix(&amount, 10).map_err(|e| {
                    Error::InvalidInput(format!(
                        "invalid --amount (erc20 approve, base units wei): {e}"
                    ))
                })?;
                handlers::validate_wallet_name(&name)?;
                let pw_string = resolve_password(password.as_deref())?;
                let pw = Zeroizing::new(pw_string.into_bytes());
                let data_dir: std::path::PathBuf =
                    cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let rpc_url = action_rpc_url.as_deref();
                let tx_hash = handlers::erc20::erc20_approve(
                    &data_dir,
                    rpc_url,
                    network,
                    &name,
                    &pw,
                    token_addr,
                    spender_addr,
                    amount_raw,
                    gas_limit,
                    max_fee_gwei,
                    priority_fee_gwei,
                    unlimited,
                    dry_run,
                )
                .await?;
                println!(
                    "tx_hash: 0x{}",
                    alloy_primitives::hex::encode(tx_hash.as_slice())
                );
                Ok(())
            }
        },
        Command::Fee(args) => {
            // T6d-1 (Issue #426 / Story 8): dispatch to the real
            // `handlers::fee::fetch_fee_estimate` async handler. Per-call
            // estimate (no cache) per plan §Q5 — Polygon's 2-second block
            // time makes cached values stale in <3s. --json formatter
            // wired here so operators + T7 smoke can pipe the result.
            let cli::FeeArgs {
                network,
                json,
                rpc_url: action_rpc_url,
            } = args;
            let net = handlers::parse_network(&network)?;
            let rpc_url = action_rpc_url.as_deref();
            let est = handlers::fee::fetch_fee_estimate(rpc_url, net).await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&est).unwrap_or_else(|_| "{}".into())
                );
            } else {
                println!("{}", handlers::fee::format_fee_human(&est));
            }
            Ok(())
        }
        Command::Config { action } => match action {
            ConfigAction::Show { json } => {
                // T6d-3 (Issue #426 / Story 11): dispatch to the real
                // `handlers::config::config_show` handler. Pure
                // resolution — no RPC, no signing. RPC URL credentials
                // redacted via `handlers::config::redact_rpc_url`.
                let out = handlers::config::config_show(
                    cli.rpc_url.as_deref(),
                    cli.data_dir.as_ref(),
                    json,
                )?;
                print!("{out}");
                Ok(())
            }
        },
        Command::Faucet(_) => stub("faucet"),
        Command::SignMessage(_) => {
            // T6d-3 (Issue #426 / Story 18): handler
            // `handlers::sign::sign_message` is implemented (pure EIP-191
            // crypto via `polygon_wallet_core::sign_message`). Dispatch
            // wiring requires `WalletManager::unlock` to derive the
            // `PrivateKeySigner` — deferred to T6 follow-up PR alongside
            // the sign-typed dispatch wiring (single WalletManager
            // unlock helper covers both).
            Err(Error::Rpc(
                "sign-message dispatch deferred to T6 follow-up (WalletManager::unlock wiring)"
                    .into(),
            ))
        }
        Command::SignTyped(_) => {
            // T6d-3 (Issue #426 / Story 27 + Q7): handler
            // `handlers::sign::sign_typed_data` is implemented (Q7
            // chain_id gate at type level). Dispatch wiring requires
            // `WalletManager::unlock` — deferred to T6 follow-up PR.
            // The Q7 gate is testable via `handlers::sign` unit tests;
            // CLI wiring is a 5-line dispatcher addition once the
            // unlock helper lands.
            Err(Error::Rpc(
                "sign-typed dispatch deferred to T6 follow-up (WalletManager::unlock wiring)"
                    .into(),
            ))
        }
    }
}

#[cfg(test)]
mod password_resolution_tests {
    //! Issue #426 / Phase 4 / Batch A — TDD seed for password resolution.
    //!
    //! Mirrors `eth/src/main.rs:769-889` verbatim per design doc §6.1.
    //! Test #1 (`argv_wins_over_env_and_prompt`) is the failing seed; tests
    //! #2-5 land in subsequent TDD cycles within the same commit batch.

    use super::{resolve_password, resolve_password_with};
    use polygon_wallet_core::{Error, Result};

    /// Test seam: serialize tests that touch process-global state
    /// (`POLYGON_PASSWORD` env var). cargo test runs tests in parallel;
    /// without this lock, env mutations from one test would race with
    /// reads in another.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    /// Test #1 (failing seed): argv `--password` wins over env + prompt.
    /// Mirrors `eth/src/main.rs:806`.
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

    /// Test #2: env path used when no argv.
    /// Mirrors `eth/src/main.rs:817`.
    #[test]
    fn env_used_when_no_argv() {
        let r = resolve_password_with(None, Some("env-pw".to_string()), ok_prompt("tty-pw"))
            .expect("env path returns Ok");
        assert_eq!(r, "env-pw");
    }

    /// Test #3: prompt path used when no argv + no env.
    /// Mirrors `eth/src/main.rs:823`.
    #[test]
    fn prompt_used_when_no_argv_no_env() {
        let r =
            resolve_password_with(None, None, ok_prompt("tty-pw")).expect("prompt path returns Ok");
        assert_eq!(r, "tty-pw");
    }

    /// Test #4: empty argv falls through to env (matches btc/src/handlers.rs:86).
    /// Mirrors `eth/src/main.rs:830`.
    #[test]
    fn empty_argv_falls_through_to_env() {
        let r = resolve_password_with(Some(""), Some("env-pw".to_string()), ok_prompt("tty-pw"))
            .expect("empty argv falls through to env");
        assert_eq!(r, "env-pw");
    }

    /// Test #5: empty argv + no env falls through to prompt.
    /// Mirrors `eth/src/main.rs:839`.
    #[test]
    fn empty_argv_no_env_falls_through_to_prompt() {
        let r = resolve_password_with(Some(""), None, ok_prompt("tty-pw"))
            .expect("empty argv + no env falls through to prompt");
        assert_eq!(r, "tty-pw");
    }

    /// Test #6: prompt IO error propagates as `Error::InvalidInput`
    /// (no panic on `/dev/tty` unavailable).
    /// Mirrors `eth/src/main.rs:846`.
    #[test]
    fn prompt_io_error_propagates_as_invalid_input() {
        let r = resolve_password_with(None, None, err_prompt());
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("password"),
                    "InvalidInput message should mention password; got: {msg}"
                );
                assert!(
                    msg.contains("simulated /dev/tty unavailable"),
                    "inner io::Error detail should propagate; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// Test #7 (Batch A item 5 from design doc §6.1): POLYGON_PASSWORD
    /// must be removed from process env after read — defense-in-depth
    /// against subprocess inheritance per L54.
    /// Mirrors `eth/src/main.rs:869-888`. L13 Round 1 fix #6 (review L4):
    /// explicit `remove_var` cleanup removed — the wrapper already
    /// removes before returning; redundant. ETH pattern (eth/src/main.rs:880-882).
    #[test]
    fn resolve_password_reads_and_removes_polygon_password_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("POLYGON_PASSWORD", "env-pw");
        // Empty argv falls through to env path, exercising the env
        // read + remove sequence.
        let result = resolve_password(Some(""));
        assert!(result.is_ok(), "empty argv + POLYGON_PASSWORD env = Ok");
        assert_eq!(result.unwrap(), "env-pw");
        assert!(
            std::env::var("POLYGON_PASSWORD").is_err(),
            "POLYGON_PASSWORD must be removed from process env after read"
        );
    }
}
