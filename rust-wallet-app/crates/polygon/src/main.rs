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

use handlers::map_wallet_err;
use handlers::wallet::read_pk_file;
use zeroize::Zeroizing;

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
            // Pass `frac` as U256 (not `frac.to_string()`) — the `{:018}`
            // zero-pad flag is numeric-only and silently no-ops on String.
            // Issue #522: amoy faucet drips <1 POL rendered as 0.37 POL
            // instead of 0.00037 POL because the leading zeros were dropped.
            let mut s = format!("{}.{:018}", whole, frac);
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

/// Resolve a wallet name to (wallet_id, network). Sign dispatch takes
/// no `--network`; same name may exist across networks — surface
/// ambiguity as `InvalidInput` (exit 2) rather than silently picking.
///
/// Implementation: iterate every known `Network` variant and call
/// `WalletManager::lookup_by_name(name, network)`. Per-network lookup
/// is the canonical helper at `evm-wallet-core/src/wallet.rs:456`.
/// (`WalletManager::list_wallets` is a known lib gap — it only
/// iterates Ethereum variants, missing Polygon wallets; tracked
/// separately as a small-deferred follow-up issue.)
fn resolve_wallet_by_name(
    wm: &evm_wallet_core::WalletManager,
    name: &str,
) -> polygon_wallet_core::Result<(uuid::Uuid, evm_wallet_core::Network)> {
    use evm_wallet_core::WalletError;
    use polygon_wallet_core::Error;
    let mut found: Vec<(uuid::Uuid, evm_wallet_core::Network)> = Vec::new();
    for network in evm_wallet_core::Network::all() {
        match wm.lookup_by_name(name, network) {
            Ok(wallet_id) => found.push((wallet_id, network)),
            Err(WalletError::NotFoundByName { .. }) => continue,
            Err(e) => return Err(map_wallet_err(e)),
        }
    }
    match found.len() {
        0 => Err(Error::InvalidInput(format!(
            "wallet '{name}' not found in any network"
        ))),
        1 => Ok(found[0]),
        _ => {
            // List colliding networks so the operator can pass --network explicitly.
            let nets: Vec<String> = found.iter().map(|(_, n)| format!("{n:?}")).collect();
            Err(Error::InvalidInput(format!(
                "wallet '{name}' exists in {} networks: [{}]; pass --network explicitly to disambiguate",
                found.len(),
                nets.join(", "),
            )))
        }
    }
}

/// Common unlock path for both sign dispatchers (EIP-191 + EIP-712).
/// Kills the 13-line duplication between `dispatch_sign_message` and
/// `dispatch_sign_typed` flagged by L12 cluster (type-design + code-review).
///
/// Pipeline: name validation → L54 password chain → keystore open →
/// `WalletManager::lookup_by_name` → `unlock_signer` (returns
/// `Zeroizing<[u8;32]>`) → `PrivateKeySigner::from_slice`.
///
/// Returns the already-scoped `PrivateKeySigner` (drops at fn exit,
/// zeroizing the raw secret via the wrapper) + the resolved `Network`
/// (unused today; reserved for v0.2 EIP-712 domain-chainId cross-check
/// per `handlers/sign.rs:113-124` deferred work).
fn unlock_wallet_by_name(
    name: &str,
    password: Option<&str>,
    data_dir: &std::path::Path,
) -> polygon_wallet_core::Result<(
    alloy_signer_local::PrivateKeySigner,
    evm_wallet_core::Network,
)> {
    use polygon_wallet_core::Error;
    handlers::validate_wallet_name(name)?;
    let pw_string = resolve_password(password)?;
    let password = Zeroizing::new(pw_string.into_bytes());
    let wm =
        evm_wallet_core::WalletManager::open_at(data_dir.to_path_buf()).map_err(map_wallet_err)?;
    let (wallet_id, network) = resolve_wallet_by_name(&wm, name)?;
    let secret = wm
        .unlock_signer(wallet_id, &password)
        .map_err(map_wallet_err)?;
    // `PrivateKeySigner::from_slice` fails iff the unlocked 32 bytes
    // aren't a valid k256 scalar (all-zero, >= curve order) — that is
    // **keystore corruption**, not a caller-input error. Map to
    // `Error::Rpc` per the exit-code table at `handlers/mod.rs:53-58`
    // (filesystem/serialization/corruption → Rpc exit 3). L12 type-design
    // nit finding — `InvalidInput` (exit 2) would mislead the operator
    // into "wrong password" retries when the actual problem is a bad
    // keystore file.
    let signer =
        alloy_signer_local::PrivateKeySigner::from_slice(secret.as_ref()).map_err(|e| {
            Error::Rpc(format!(
                "keystore corruption: PrivateKeySigner::from_slice failed: {e}"
            ))
        })?;
    Ok((signer, network))
}

/// Pass through the optional `--verify` Address flag.
///
/// The CLI flag is already typed `Option<Address>` via clap's
/// `value_parser = parse_address` (see `cli.rs:498-499,500-501`), so
/// the legacy string-reparse helper is a no-op pass-through.
fn parse_verify_flag(
    verify: Option<alloy_primitives::Address>,
) -> polygon_wallet_core::Result<Option<alloy_primitives::Address>> {
    Ok(verify)
}

/// CLI dispatch helper for `polygon sign-message` (Story 18, EIP-191).
/// Returns the 65-byte signature as `0x`-prefixed hex.
fn dispatch_sign_message(
    args: &cli::SignMessageArgs,
    data_dir: &std::path::Path,
) -> polygon_wallet_core::Result<String> {
    let (signer, _network) = unlock_wallet_by_name(&args.name, args.password.as_deref(), data_dir)?;
    let verify = parse_verify_flag(args.verify)?;
    handlers::sign::sign_message(&signer, args.message.as_bytes(), verify)
}

/// CLI dispatch helper for `polygon sign-typed` (Story 27, EIP-712 +
/// Q7 critical-tier chain_id gate).
///
/// Q7 gate fires FIRST (before any unlock / parse) so a bad `--chain-id`
/// surfaces immediately, not after a successful unlock. L12 code-review
/// finding: original order had `--verify` parse failure masking the
/// Q7 rejection when both flags were wrong.
fn dispatch_sign_typed(
    args: &cli::SignTypedArgs,
    data_dir: &std::path::Path,
) -> polygon_wallet_core::Result<String> {
    use polygon_wallet_core::Error;
    // Q7 gate first (cross-chain replay defense — must precede all other work).
    handlers::sign::assert_polygon_chain_id(args.chain_id)?;
    let (signer, _network) = unlock_wallet_by_name(&args.name, args.password.as_deref(), data_dir)?;
    let verify = parse_verify_flag(args.verify)?;
    // typed-data source: inline JSON (`--typed-data`) OR file
    // (`--typed-data-file`). clap `conflicts_with` enforces mutual
    // exclusion; `required_unless_present` (cli.rs) enforces at-least-one.
    // L12 convergent finding: previous dispatch silently dropped the
    // file-path arm even though the CLI flag was declared.
    let typed_data_json: String =
        match (args.typed_data.as_deref(), args.typed_data_file.as_deref()) {
            (Some(s), _) => s.to_string(),
            (None, Some(p)) => std::fs::read_to_string(p).map_err(|e| {
                Error::InvalidInput(format!("read --typed-data-file {}: {e}", p.display()))
            })?,
            (None, None) => {
                return Err(Error::InvalidInput(
                    "--typed-data or --typed-data-file required".into(),
                ));
            }
        };
    // Empty string slips past `Option<String>` clap validation; lib-side
    // rejection then surfaces as `Error::Rpc` instead of caller-side
    // `InvalidInput` the CLI contract promises.
    if typed_data_json.trim().is_empty() {
        return Err(Error::InvalidInput("--typed-data must not be empty".into()));
    }
    handlers::sign::sign_typed_data(&signer, &typed_data_json, args.chain_id, verify)
}

/// T6c1 follow-up: `run()` is now `async fn` so it can drive the
/// `wallet_balance` async handler (and future async handlers — T6c3
/// `wallet_sync`, T6c5 `wallet_send_*`). `main()` wraps `run()` in a
/// tokio current-thread runtime via `block_on` (mirrors `eth/src/main.rs:487-491`).
async fn run(cli: cli::Cli) -> polygon_wallet_core::Result<()> {
    use alloy_primitives::{utils::parse_units, U256};
    use cli::{Command, ConfigAction, Erc20Action, TxAction, WalletAction};
    use polygon_wallet_core::Error;
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
                private_key,
                private_key_file,
                account_index: _,
                legacy_token_symbol: _,
                rpc_url: _,
            } => {
                // #469 dispatch — three import sources (mutually
                // exclusive via clap `conflicts_with`, enforced at parse
                // time):
                //   1. `--mnemonic` (existing path)        → wallet_import
                //   2. `--private-key <hex>` (newly wired) → wallet_import_private_key_for_network
                //   3. `--private-key-file <path>` (new)   → wallet_import_private_key_for_network
                //      via handlers::wallet::read_pk_file (mode-0600
                //      + Zeroizing<Vec<u8>> wrap; mode check skipped
                //      on non-Unix).
                //
                // clap already rejects combinations, so the match here
                // is an exhaustive single-Some guard (with the
                // none-of-the-above error as the negative case).
                // Sister invariant to L12 H-1 finding closed by PR
                // #456 for `--mnemonic`.
                let net = handlers::parse_network(&network)?;
                let pw_string = resolve_password(password.as_deref())?;
                let password = Zeroizing::new(pw_string.into_bytes());
                let data_dir: std::path::PathBuf =
                    cli.data_dir.clone().unwrap_or_else(default_data_dir);
                let created = match (mnemonic, private_key, private_key_file) {
                    (Some(ref phrase), None, None) => {
                        // Mnemonic path (PR #456). Restored per #502 —
                        // the prior (Some(_),_,_) catch-all below made
                        // this tuple unreachable and silently killed
                        // mnemonic-based wallet import on the parent
                        // branch after the Phase 2 squash (#497 eb360c1).
                        // Sister to the (None,Some(hex),None) arm;
                        // SecretMnemonic wraps the phrase for zero-on-drop.
                        handlers::wallet::wallet_import(&data_dir, &name, &password, net, phrase)?
                    }
                    (None, Some(hex), None) => {
                        // Wired path (was dead pre-#469 per the T6c4
                        // follow-up comment). Hex-decode + Zeroizing
                        // wrap matches the file variant's invariants.
                        let bytes = alloy_primitives::hex::decode(hex.trim_start_matches("0x"))
                            .map_err(|e| {
                                Error::InvalidInput(format!("--private-key hex decode failed: {e}"))
                            })?;
                        let pk_bytes = Zeroizing::new(bytes);
                        handlers::wallet::wallet_import_private_key_for_network(
                            &data_dir, &name, &password, net, &pk_bytes,
                        )?
                    }
                    (None, None, Some(path)) => {
                        handlers::wallet::wallet_import_private_key_for_network(
                            &data_dir,
                            &name,
                            &password,
                            net,
                            &read_pk_file(&path)?,
                        )?
                    }
                    (None, None, None) => {
                        return Err(Error::InvalidInput(
                            "one of --mnemonic, --private-key, --private-key-file required".into(),
                        ));
                    }
                    // clap `conflicts_with` already rejects every other
                    // (mnemonic × private_key × private_key_file)
                    // combination at parse time; the single `_` catch-
                    // all is the defense-in-depth net for programmatic
                    // callers that bypass clap.
                    _ => {
                        return Err(Error::InvalidInput(
                            "exactly one of --mnemonic / --private-key / --private-key-file allowed".into(),
                        ));
                    }
                };
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
                handlers::tx::tx_list(&address.to_string(), since_block, limit, json).await?;
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
                    Some(a) => handlers::erc20::resolve_token_address(&a.to_string(), network)?,
                    None => handlers::erc20::resolve_token_address(&token, network)?,
                };
                let to_addr = to;
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
            Erc20Action::Balance {
                address,
                token,
                token_address,
                network,
                all: _,
                decimals,
                json,
                rpc_url: action_rpc_url,
            } => {
                // T6d-2.1 (Issue #523): dispatch to the real
                // `handlers::erc20::erc20_balance` handler.
                // `token_address: Option<Address>` takes precedence over
                // the symbol form when supplied (mirrors `Send` arm at
                // main.rs:724-727). Per-action `--rpc-url` overrides
                // the global `--rpc-url`. `--all` deferred (issue body
                // lists it as deferrable); `--decimals <N>` skips the
                // secondary `decimals()` eth_call when supplied.
                let net = handlers::parse_network(&network)?;
                let token_addr = match token_address {
                    Some(a) => handlers::erc20::resolve_token_address(&a.to_string(), net)?,
                    None => handlers::erc20::resolve_token_address(&token, net)?,
                };
                let holder_addr = address;
                let rpc_url = action_rpc_url.as_deref().or(cli.rpc_url.as_deref());
                let result =
                    handlers::erc20::erc20_balance(rpc_url, net, holder_addr, token_addr, decimals)
                        .await?;
                if json {
                    let payload = serde_json::json!({
                        "holder": format!("{:#x}", result.holder),
                        "token": format!("{:#x}", result.token),
                        "decimals": result.decimals,
                        "raw": result.raw.to_string(),
                        "formatted": result.formatted(),
                    });
                    println!(
                        "{}",
                        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into())
                    );
                } else {
                    println!(
                        "{} (raw: {}, decimals: {}, holder: {:#x}, token: {:#x})",
                        result.formatted(),
                        result.raw,
                        result.decimals,
                        result.holder,
                        result.token,
                    );
                }
                Ok(())
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
                let spender_addr = spender;
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
            ConfigAction::Show { network, json } => {
                // T6d-3 (Issue #426 / Story 11): dispatch to the real
                // `handlers::config::config_show` handler. Pure
                // resolution — no RPC, no signing. RPC URL credentials
                // redacted via `handlers::config::redact_rpc_url`.
                let out = handlers::config::config_show(
                    cli.rpc_url.as_deref(),
                    cli.data_dir.as_ref(),
                    &network,
                    json,
                )?;
                print!("{out}");
                Ok(())
            }
        },
        Command::Faucet(_) => stub("faucet"),
        Command::SignMessage(args) => {
            // T6d-3 follow-up (Issue #459): wire dispatch via
            // `dispatch_sign_message`. Sister pattern at
            // `eth/src/main.rs::Command::Sign` per #350/#351.
            let data_dir: std::path::PathBuf =
                cli.data_dir.clone().unwrap_or_else(default_data_dir);
            let sig = dispatch_sign_message(&args, &data_dir)?;
            println!("{sig}");
            Ok(())
        }
        Command::SignTyped(args) => {
            // T6d-3 follow-up (Issue #459): wire dispatch via
            // `dispatch_sign_typed`. Q7 chain_id gate fires inside
            // `handlers::sign::sign_typed_data` before any signing.
            let data_dir: std::path::PathBuf =
                cli.data_dir.clone().unwrap_or_else(default_data_dir);
            let sig = dispatch_sign_typed(&args, &data_dir)?;
            println!("{sig}");
            Ok(())
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

#[cfg(test)]
mod format_balance_tests {
    //! Issue #522 — `format_balance("pol", wei)` returned 1000× too large.
    //!
    //! Root cause: the `{:018}` zero-pad flag is numeric-only; passing
    //! `frac.to_string()` (a String) silently dropped the leading zeros,
    //! so 369,999,998,677,000 wei rendered as `0.369999998677000 POL`
    //! instead of `0.000369999998677 POL`. Sister formatter in
    //! `handlers/erc20.rs:144` uses `{:0>w$}` (alignment), which DOES
    //! pad Strings — main.rs formatter was the bug class.
    //!
    //! AC vectors from #522:
    //! - raw 0x15083567cf008 = 369,999,998,677,000 wei → "0.000369999998677 POL"
    //! - 1 POL (10^18 wei) → "1.0 POL"
    //! - 0 wei → "0.0 POL"
    //! - wei unit → raw U256 + " wei"
    //! - unknown unit → falls back to wei

    use super::format_balance;
    use alloy_primitives::U256;

    /// #522 primary vector: small-balance display matches oracle.
    /// Catches the regression where `{:018}` was applied to a String
    /// instead of a numeric type, stripping leading zeros and shifting
    /// the decimal three places.
    #[test]
    fn format_balance_pol_amoy_faucet_drip_matches_rpc_oracle() {
        let raw_wei = U256::from(369_999_998_677_000u64);
        let got = format_balance(raw_wei, "pol");
        assert_eq!(
            got, "0.000369999998677 POL",
            "369,999,998,677,000 wei must render as 0.000369999998677 POL (1000× regression catch); got {got}"
        );
    }

    /// Whole-POL balance: 10^18 wei must render as "1.0 POL", NOT
    /// "1.000000000000000000 POL" (the untrimmed raw).
    #[test]
    fn format_balance_pol_one_wei_trims_to_one_point_zero() {
        let one_pol = U256::from(1_000_000_000_000_000_000u128);
        let got = format_balance(one_pol, "pol");
        assert_eq!(got, "1.0 POL", "1 POL must trim to 1.0; got {got}");
    }

    /// Zero balance: 0 wei with `pol` unit must render "0.0 POL"
    /// (preserves the "always one decimal" rule).
    #[test]
    fn format_balance_pol_zero_wei_renders_zero_point_zero() {
        let got = format_balance(U256::ZERO, "pol");
        assert_eq!(got, "0.0 POL", "0 wei pol must be 0.0; got {got}");
    }

    /// `wei` unit returns the raw U256 + " wei" suffix, no division.
    #[test]
    fn format_balance_wei_unit_returns_raw_with_suffix() {
        let raw = U256::from(369_999_998_677_000u64);
        let got = format_balance(raw, "wei");
        assert_eq!(
            got, "369999998677000 wei",
            "wei unit must echo raw + suffix; got {got}"
        );
    }

    /// Unknown unit falls back to wei (per the fn-level doc at
    /// `format_balance` — defensive default; catches accidental contract
    /// changes that drop the fallback).
    #[test]
    fn format_balance_unknown_unit_falls_back_to_wei() {
        let raw = U256::from(42u64);
        let got = format_balance(raw, "gwei");
        assert_eq!(
            got, "42 wei",
            "unknown unit must fall back to wei; got {got}"
        );
    }
}

#[cfg(test)]
mod sign_dispatch_tests {
    //! Issue #459 (T6d-3 follow-up): wire `polygon sign-message` + `polygon
    //! sign-typed` dispatch via `WalletManager::unlock_signer` (sister
    //! pattern at `evm-wallet-core/src/wallet.rs:600` per #350).
    //!
    //! Lock-down coverage:
    //! 1. Happy path: SignMessage round-trip (create wallet, dispatch returns signature)
    //! 2. Sad path: SignMessage wrong password returns InvalidPassword
    //! 3. Happy path: SignTyped chain_id=137 passes Q7 gate
    //! 4. Sad path: SignTyped chain_id=1 rejected at Q7 gate

    use super::{dispatch_sign_message, dispatch_sign_typed};
    use crate::cli;
    use polygon_wallet_core::Error;

    /// Unique tempdir under $TMPDIR — avoids the lifetime-managed
    /// `tempfile::TempDir` to keep main.rs dev-dep surface minimal.
    fn unique_tempdir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("polygon-test-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    /// Create a fresh keystore at `data_dir` with one wallet named `name`
    /// under the given `Network`. Drops the `WalletManager` so dispatch
    /// opens it fresh (mirrors real CLI invocation).
    fn fixture_wallet(
        name: &str,
        password: &str,
        network: evm_wallet_core::Network,
        tag: &str,
    ) -> std::path::PathBuf {
        let data_dir = unique_tempdir(tag);
        let wm = evm_wallet_core::WalletManager::open_at(data_dir.clone()).expect("open_at");
        let created = wm
            .create_wallet_for_network(name, password.as_bytes(), network)
            .expect("create_wallet_for_network");
        assert_eq!(created.name, name);
        assert_eq!(created.network, network);
        drop(wm);
        data_dir
    }

    /// Test #1 (failing seed per L13 step 9 TDD red): dispatch returns
    /// `0x`-prefixed 65-byte hex signature for a valid wallet + password.
    /// Sister test at `handlers/sign.rs:259` covers the handler in
    /// isolation; this test exercises the full dispatch chain
    /// (resolve_wallet_by_name → unlock_signer → signer scope → handler).
    #[test]
    fn dispatch_sign_message_returns_signature_for_valid_wallet() {
        let password = "correct-horse-battery-staple";
        let data_dir = fixture_wallet(
            "w",
            password,
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Amoy),
            "happy",
        );
        let args = cli::SignMessageArgs {
            name: "w".into(),
            password: Some(password.into()),
            message: "hello, polygon".into(),
            address: None,
            verify: None,
            rpc_url: None,
        };
        let sig = dispatch_sign_message(&args, &data_dir).expect("dispatch ok");
        assert!(sig.starts_with("0x"), "must be 0x-prefixed; got {sig}");
        assert_eq!(
            sig.len(),
            132,
            "0x + 130 hex chars = 65 raw bytes; got {} chars",
            sig.len()
        );
    }

    /// Test #2: wrong password surfaces as a caller-side error (exit 2).
    /// `unlock_signer` re-derives the keystore key and AEAD-decrypts with
    /// the supplied password; tag mismatch → `WalletError::Crypto(...)`.
    /// `map_wallet_err` (handlers/mod.rs:66) translates Crypto →
    /// `Error::InvalidInput` (exit 2, per #455 L12 cluster mapping:
    /// caller-side errors → InvalidInput). Assert the variant + that
    /// the message names the crypto path so a future swap to a generic
    /// `Error::Rpc` doesn't silently mask auth-tag failures.
    #[test]
    fn dispatch_sign_message_wrong_password_returns_invalid_input() {
        let data_dir = fixture_wallet(
            "w",
            "correct-horse-battery-staple",
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Amoy),
            "wrong-pw",
        );
        let args = cli::SignMessageArgs {
            name: "w".into(),
            password: Some("bogus-password".into()),
            message: "anything".into(),
            address: None,
            verify: None,
            rpc_url: None,
        };
        let r = dispatch_sign_message(&args, &data_dir);
        match r {
            Err(Error::InvalidInput(msg)) => assert!(
                msg.contains("crypto") || msg.contains("AES"),
                "wrong-password message must name the crypto path (no silent \
                 masking to Rpc); got {msg:?}"
            ),
            other => panic!("wrong password must surface as InvalidInput (exit 2); got {other:?}"),
        }
    }

    /// Test #3: SignTyped with chain_id=137 passes the Q7 gate.
    /// Result is `Ok(sig)` once the alloy `eip712` feature lands, OR
    /// `Err(Error::Rpc(_))` while the lib is stubbed (per signer.rs:164
    /// deferral notice). Either way the Q7 gate MUST NOT fire.
    #[test]
    fn dispatch_sign_typed_chain_id_137_passes_gate() {
        let password = "correct-horse-battery-staple";
        let data_dir = fixture_wallet(
            "w",
            password,
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Mainnet),
            "typed-ok",
        );
        let args = cli::SignTypedArgs {
            chain_id: 137,
            typed_data: Some(r#"{"types":{}}"#.into()),
            typed_data_file: None,
            name: "w".into(),
            password: Some(password.into()),
            address: None,
            verify: None,
            rpc_url: None,
        };
        match dispatch_sign_typed(&args, &data_dir) {
            Ok(sig) => assert!(
                sig.starts_with("0x") && sig.len() == 132,
                "gate-passed result must be 0x + 130 hex chars; got {sig}"
            ),
            Err(Error::Rpc(_)) => {} // honest lib-side deferral
            other => panic!("chain_id=137 must pass Q7 gate; got {other:?}"),
        }
    }

    /// Test #4: SignTyped with chain_id=1 (Ethereum mainnet) rejected
    /// at the Q7 gate — cross-chain replay defense. Mirrors the
    /// handler-level test at `handlers/sign.rs:315` but exercises the
    /// full dispatch chain (no shortcut through the handler).
    #[test]
    fn dispatch_sign_typed_chain_id_1_rejected_at_gate() {
        let password = "correct-horse-battery-staple";
        let data_dir = fixture_wallet(
            "w",
            password,
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Amoy),
            "typed-1",
        );
        let args = cli::SignTypedArgs {
            chain_id: 1,
            typed_data: Some(r#"{"types":{}}"#.into()),
            typed_data_file: None,
            name: "w".into(),
            password: Some(password.into()),
            address: None,
            verify: None,
            rpc_url: None,
        };
        let r = dispatch_sign_typed(&args, &data_dir);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=1 must be rejected at Q7 gate; got {r:?}"
        );
    }

    /// Test #5 (L12 code-review finding #2): resolve_wallet_by_name
    /// surfaces cross-network ambiguity as `InvalidInput` AND lists the
    /// colliding networks so the operator can pass `--network` explicitly.
    /// Two wallets named "dup" under Sepolia + Amoy.
    #[test]
    fn resolve_wallet_by_name_ambiguous_returns_invalid_input_with_networks() {
        use super::resolve_wallet_by_name;
        let data_dir = unique_tempdir("ambiguous");
        let wm = evm_wallet_core::WalletManager::open_at(data_dir.clone()).expect("open_at");
        wm.create_wallet_for_network(
            "dup",
            b"pw-eth",
            evm_wallet_core::Network::Ethereum(evm_wallet_core::EthereumChain::Sepolia),
        )
        .expect("create eth");
        wm.create_wallet_for_network(
            "dup",
            b"pw-poly",
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Amoy),
        )
        .expect("create polygon");
        drop(wm);
        let wm2 = evm_wallet_core::WalletManager::open_at(data_dir).expect("reopen");
        let r = resolve_wallet_by_name(&wm2, "dup");
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("2 networks"),
                    "ambiguity msg must state the count; got {msg}"
                );
                assert!(
                    msg.contains("Sepolia") || msg.contains("Ethereum"),
                    "ambiguity msg must list the colliding networks (Sepolia); got {msg}"
                );
                assert!(
                    msg.contains("Amoy") || msg.contains("Polygon"),
                    "ambiguity msg must list the colliding networks (Amoy); got {msg}"
                );
            }
            other => panic!("expected InvalidInput ambiguity error; got {other:?}"),
        }
    }

    /// Test #6 (L12 code-review finding #3 part A): `dispatch_sign_message`
    /// `--verify` happy round-trip — caller passes the signer's own
    /// address, dispatch returns the signature without error.
    #[test]
    fn dispatch_sign_message_verify_happy_round_trips() {
        let password = "correct-horse-battery-staple";
        let data_dir = fixture_wallet(
            "w",
            password,
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Amoy),
            "verify-happy",
        );
        // Recover the signer address via unlock_signer round-trip.
        let wm = evm_wallet_core::WalletManager::open_at(data_dir.clone())
            .expect("open_at to look up addr");
        let (wallet_id, _net) = super::resolve_wallet_by_name(&wm, "w").expect("resolve");
        let password_z = zeroize::Zeroizing::new(password.as_bytes().to_vec());
        let secret = wm
            .unlock_signer(wallet_id, &password_z)
            .expect("unlock signer for addr");
        let signer = alloy_signer_local::PrivateKeySigner::from_slice(secret.as_ref())
            .expect("signer from slice");
        let signer_addr = signer.address();
        drop(wm);
        drop(signer);

        let args = cli::SignMessageArgs {
            name: "w".into(),
            password: Some(password.into()),
            message: "verify-happy-test".into(),
            address: None,
            verify: Some(signer_addr),
            rpc_url: None,
        };
        let sig = dispatch_sign_message(&args, &data_dir).expect("--verify happy round-trip");
        assert!(sig.starts_with("0x"));
        assert_eq!(sig.len(), 132);
    }

    /// Test #7 (L12 code-review finding #3 part B): `--verify` mismatch
    /// surfaces as `Error::InvalidInput` (exit 2, caller-side). Sister
    /// test at `handlers/sign.rs:299-309` covers the handler in
    /// isolation; this exercises the dispatch path end-to-end.
    #[test]
    fn dispatch_sign_message_verify_mismatch_returns_invalid_input() {
        let data_dir = fixture_wallet(
            "w",
            "correct-horse-battery-staple",
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Amoy),
            "verify-mismatch",
        );
        let args = cli::SignMessageArgs {
            name: "w".into(),
            password: Some("correct-horse-battery-staple".into()),
            message: "verify-mismatch-test".into(),
            address: None,
            verify: Some(
                "0x0000000000000000000000000000000000000000"
                    .parse::<alloy_primitives::Address>()
                    .expect("hard-coded verify addr should parse"),
            ),
            rpc_url: None,
        };
        let r = dispatch_sign_message(&args, &data_dir);
        match r {
            Err(Error::InvalidInput(msg)) => assert!(
                msg.contains("verify") || msg.contains("mismatch"),
                "--verify mismatch must surface as InvalidInput naming the verify path; got {msg:?}"
            ),
            other => panic!("--verify mismatch must return InvalidInput (exit 2); got {other:?}"),
        }
    }

    /// Test #8 (L12 code-review finding #4): `polygon sign-typed --verify`
    /// returns `InvalidInput` per the handler deferral notice at
    /// `handlers/sign.rs:159-163`. Pinning the contract so a future
    /// refactor that drops the `verify_address.is_some()` short-circuit
    /// doesn't silently change exit code from 2 to 3.
    #[test]
    fn dispatch_sign_typed_verify_returns_invalid_input() {
        let password = "correct-horse-battery-staple";
        let data_dir = fixture_wallet(
            "w",
            password,
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Mainnet),
            "typed-verify",
        );
        let args = cli::SignTypedArgs {
            chain_id: 137,
            typed_data: Some(r#"{"types":{}}"#.into()),
            typed_data_file: None,
            name: "w".into(),
            password: Some(password.into()),
            address: None,
            verify: Some(
                "0x0000000000000000000000000000000000000000"
                    .parse::<alloy_primitives::Address>()
                    .expect("hard-coded verify addr should parse"),
            ),
            rpc_url: None,
        };
        let r = dispatch_sign_typed(&args, &data_dir);
        match r {
            // Lib is real: short-circuit returns InvalidInput naming the
            // deferred state.
            Err(Error::InvalidInput(msg)) => assert!(
                msg.contains("deferred") || msg.contains("verify"),
                "sign-typed --verify deferral must name the deferred state; got {msg:?}"
            ),
            // Lib is stubbed (current state per signer.rs:164): lib-side
            // deferral surfaces as Rpc with eip712 prefix. Q7 gate DID
            // NOT fire — that's the contract we're verifying here.
            Err(Error::Rpc(msg)) => assert!(
                msg.contains("eip712"),
                "lib-side deferral must name the eip712 path; got {msg}"
            ),
            other => panic!(
                "sign-typed --verify must surface deferral (InvalidInput or Rpc); got {other:?}"
            ),
        }
    }

    /// Test #9 (L12 HIGH fix): `polygon sign-typed --typed-data-file`
    /// reads the file and proceeds through the dispatch chain. The
    /// previous dispatch silently dropped this CLI flag — operators who
    /// passed `--typed-data-file path/to/permit2.json` (canonical way to
    /// ship large EIP-712 payloads exceeding argv limits) hit a
    /// misleading `--typed-data or --typed-data-file required` error.
    #[test]
    fn dispatch_sign_typed_typed_data_file_path() {
        let password = "correct-horse-battery-staple";
        let data_dir = fixture_wallet(
            "w",
            password,
            evm_wallet_core::Network::Polygon(evm_wallet_core::PolygonChain::Amoy),
            "typed-file",
        );
        let td_path =
            std::env::temp_dir().join(format!("polygon-test-typed-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&td_path, r#"{"types":{}}"#).expect("write typed-data file");
        let args = cli::SignTypedArgs {
            chain_id: 137,
            typed_data: None,
            typed_data_file: Some(td_path.clone()),
            name: "w".into(),
            password: Some(password.into()),
            address: None,
            verify: None,
            rpc_url: None,
        };
        let result = dispatch_sign_typed(&args, &data_dir);
        // Cleanup (best-effort — tempdirs leak; tracked in L12 code-review #6).
        let _ = std::fs::remove_file(&td_path);
        match result {
            Ok(sig) => assert!(
                sig.starts_with("0x") && sig.len() == 132,
                "file-path dispatch must produce a 0x+130-hex sig; got {sig}"
            ),
            Err(Error::Rpc(_)) => {} // honest lib-side deferral
            other => panic!("typed-data-file dispatch must pass Q7 gate; got {other:?}"),
        }
    }
}
