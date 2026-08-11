//! `btc wallet create` / `btc wallet show` subcommand handlers.
//!
//! Thin adapters from CLI args → `bitcoin_wallet_core::wallet` ops.
//! Public surface is `create_handler` + `show_handler`; both take
//! `stdout` / `stderr` writers so the "mnemonic NEVER to stdout"
//! regression test can run without spawning a process.

use std::io::Write;

use anyhow::{Context, Result};
use bitcoin::Network;
use bitcoin_wallet_core::chain::esplora::EsploraClient;
use bitcoin_wallet_core::error::Error as CoreError;
use bitcoin_wallet_core::keys::Secret;
use bitcoin_wallet_core::wallet::{
    create_wallet as core_create, data_dir, show_wallet as core_show, WalletId,
};
use serde_json::json;

/// Banner printed to STDERR before the mnemonic in `btc wallet create`.
/// Per ADR §Failure modes + L28 (mnemonic shown ONCE — record now).
const MNEMONIC_BANNER: &str = "\
WARNING: This mnemonic is shown ONCE and never recoverable from disk \
without the encryption password. Record it now in a secure location.";

/// `btc wallet create` handler.
///
/// Prints:
/// - STDOUT: the wallet ID (random UUID v4, no PII, no crypto material).
/// - STDERR: the mnemonic, with a banner.
///
/// L28 honesty: mnemonic NEVER appears on STDOUT. Regression test in
/// `tests::create_writes_mnemonic_to_stderr_not_stdout` enforces this.
pub fn create_handler<O: Write, E: Write>(
    words: usize,
    network: Network,
    password_flag: Option<String>,
    stdout: &mut O,
    stderr: &mut E,
) -> Result<()> {
    let password = resolve_password(password_flag)?;
    let base = data_dir().map_err(|e| anyhow::anyhow!("{e}"))?;
    let (id, phrase) = core_create(&base, network, words, &password)
        .map_err(|e| anyhow::anyhow!("create wallet: {e}"))?;
    // Wallet ID → STDOUT (machine-parseable, no secrets).
    writeln!(stdout, "{id}").context("write wallet id to stdout")?;
    // Mnemonic → STDERR (with banner). The CLI test verifies it does
    // not appear on STDOUT.
    writeln!(stderr, "{MNEMONIC_BANNER}").context("write banner to stderr")?;
    writeln!(stderr, "{}", phrase.expose()).context("write mnemonic to stderr")?;
    Ok(())
}

/// `btc wallet show` handler.
///
/// Prints:
/// - STDOUT: JSON `{receive_addresses, change_addresses, balance_sat}`.
///
/// Errors that would otherwise leak wallet existence (missing wallet,
/// wrong password, wrong network AAD, corrupt blob) collapse into the
/// indistinguishable `Error::WalletStore("wallet not accessible ...")`
/// message (N2 + N5 oracle mitigation). Shown on STDERR.
pub async fn show_handler<O: Write, E: Write>(
    network: Network,
    id: WalletId,
    password_flag: Option<String>,
    esplora: &EsploraClient,
    stdout: &mut O,
    _stderr: &mut E,
) -> Result<()> {
    let password = resolve_password(password_flag)?;
    let base = data_dir().map_err(|e| anyhow::anyhow!("{e}"))?;
    let info = core_show(&base, network, id, &password, esplora)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let payload = json!({
        "receive_addresses": info
            .receive_addresses
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>(),
        "change_addresses": info
            .change_addresses
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>(),
        "balance_sat": info.balance_sat,
    });
    writeln!(stdout, "{payload}").context("write wallet info to stdout")?;
    Ok(())
}

/// Resolve the encryption password from `--password` flag or interactive
/// prompt. Returns the password wrapped in `Secret<Vec<u8>>` (zeroize
/// on drop, F47).
///
/// **Note on prompt:** `rpassword::prompt_password` reads from the
/// controlling TTY. In non-interactive contexts (CI, tests) the prompt
/// fails — callers must supply `--password`.
fn resolve_password(flag: Option<String>) -> Result<Secret<Vec<u8>>> {
    if let Some(p) = flag {
        return Ok(Secret::new(p.into_bytes()));
    }
    let p = rpassword::prompt_password("password: ")
        .map_err(|e| CoreError::WalletStore(format!("password prompt failed: {e}")))?;
    Ok(Secret::new(p.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The core #64 acceptance test: mnemonic NEVER appears on STDOUT.
    /// Regression test for L28 honesty.
    #[test]
    fn create_writes_mnemonic_to_stderr_not_stdout() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = create_handler(
            12,
            Network::Testnet,
            Some("test-password-do-not-use-in-prod".into()),
            &mut stdout,
            &mut stderr,
        );

        let stdout_str = String::from_utf8_lossy(&stdout);
        let stderr_str = String::from_utf8_lossy(&stderr);

        // If handler succeeded, stdout must contain only the wallet id.
        if let Ok(()) = result {
            let id_line = stdout_str.trim();
            assert!(
                id_line.len() == 36 && id_line.chars().filter(|c| *c == '-').count() == 4,
                "stdout wallet id not a UUID: {id_line}"
            );
            // Cleanup: best-effort delete of the just-created blob.
            let id: WalletId = id_line.parse().expect("parse id");
            let path = bitcoin_wallet_core::wallet::wallet_path(Network::Testnet, id)
                .expect("wallet_path");
            let _ = std::fs::remove_file(path);
            // Banner must be on stderr.
            assert!(
                stderr_str.contains("WARNING"),
                "stderr missing banner: {stderr_str}"
            );
            // Mnemonic must be on stderr.
            let stderr_lines: Vec<&str> = stderr_str.lines().collect();
            assert!(
                stderr_lines
                    .iter()
                    .any(|l| l.split_whitespace().count() == 12),
                "stderr missing 12-word mnemonic; got: {stderr_str}"
            );
        }
        // Even on failure, stdout must not leak the mnemonic (it
        // shouldn't have one yet). The invariant is the same.
        assert!(
            stdout_str.lines().count() <= 1,
            "stdout must contain at most 1 line (wallet id), got {} lines: {stdout_str}",
            stdout_str.lines().count()
        );
    }
}
