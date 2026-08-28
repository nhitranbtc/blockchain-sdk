//! `polygon` CLI binary — Issue #426 / Phase 4 of #416.
//!
//! Minimal TDD scaffold for Batch A (password resolution tests). Full clap
//! tree + handler dispatch land in subsequent batches per
//! `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`.
//!
//! L13 Round 1 fix #6: per-helper `#[allow(dead_code)]` annotations cover
//! the critical-tier helper functions (`resolve_password`,
//! `resolve_password_with`, `prompt_password`) that the minimal
//! `fn main` does not yet dispatch to. Each helper has unit tests (TDD
//! Batch A green); the dispatch wiring lands in the next T6 follow-up
//! commit alongside the clap tree. Honest state: tests pass, dispatch
//! not wired. Helpers in submodules (`handlers::{mod,sign,erc20}`)
//! carry their own `#[allow(dead_code)]` per-function.

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
    // TDD scaffold: full dispatch (Cli::parse + match arms + tokio runtime)
    // lands in subsequent batches. Today's main returns success so the
    // binary builds + the password tests can run.
    std::process::ExitCode::SUCCESS
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
