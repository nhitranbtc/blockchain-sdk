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
    use cli::{Command, Erc20Action, TxAction, WalletAction};
    use polygon_wallet_core::Error;

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
            WalletAction::Create { .. } => stub("wallet create"),
            WalletAction::Import { .. } => stub("wallet import"),
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
                let balance =
                    handlers::wallet::wallet_balance(rpc_url.as_deref(), &address).await?;
                println!("{}", format_balance(balance, &unit));
                Ok(())
            }
            WalletAction::Sync { .. } => stub("wallet sync"),
            WalletAction::Send(_) => stub("wallet send"),
            WalletAction::SendSpeedup(_) => stub("wallet send speed-up"),
        },
        Command::Tx { action } => match action {
            TxAction::List { .. } => stub("tx list"),
            TxAction::Get { .. } => stub("tx get"),
        },
        Command::Erc20 { action } => match action {
            Erc20Action::Send { .. } => stub("erc20 send"),
            Erc20Action::Balance { .. } => stub("erc20 balance"),
            Erc20Action::List { .. } => stub("erc20 list"),
            Erc20Action::Register { .. } => stub("erc20 register"),
            Erc20Action::Approve { .. } => stub("erc20 approve"),
        },
        Command::Fee(_) => stub("fee"),
        Command::Config { .. } => stub("config"),
        Command::Faucet(_) => stub("faucet"),
        Command::SignMessage(_) => stub("sign-message"),
        Command::SignTyped(_) => stub("sign-typed"),
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
