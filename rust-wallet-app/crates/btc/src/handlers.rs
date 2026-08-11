//! Handlers for `btc wallet create` + `btc wallet show` subcommands.
//!
//! **F49 / L28**: wallet_id → STDOUT (scriptable), mnemonic → STDERR
//! (operator-only). Regression test enforces in `tests/cli.rs`.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};

use bitcoin_wallet_core::chain::esplora::{EsploraClient, TlsPolicy};
use bitcoin_wallet_core::chain::esplora_url::EsploraUrl;
use bitcoin_wallet_core::keys::Secret;
use bitcoin_wallet_core::wallet::{create_wallet, show_wallet, WalletId, SUPPORTED_WORD_COUNTS};

use crate::cli::{NetArg, WordCount};

/// Default Esplora URL for `--network testnet` (L29 operator's
/// reference endpoint). Production callers should override via
/// `--esplora-url` with an SPKI-pinned URL per F20.
const DEFAULT_TESTNET_ESPLORA: &str = "https://blockstream.info/testnet/api";

/// Resolve the wallet data directory:
///   explicit `--data-dir` flag > `$BTC_DATA_DIR` env > `$XDG_DATA_HOME`.
///
/// **Important**: we return the **raw** `$XDG_DATA_HOME` (no `btc`
/// suffix). The library's `wallet_path_at(base, ...)` appends
/// `btc/wallets/<network>/<id>.enc` to whatever base it's given,
/// so passing `$XDG_DATA_HOME/btc` here would yield a doubled
/// `btc/btc/` segment. Tests rely on this contract.
pub fn resolve_data_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg));
        }
    }
    let home = std::env::var("HOME").context("HOME not set; required for default data_dir")?;
    Ok(PathBuf::from(home).join(".local").join("share"))
}

/// Prompt for password if not provided via `--password` flag.
/// `rpassword::prompt_password` reads from `/dev/tty` (not stdin)
/// to avoid leaking via piped input.
fn password_or_prompt(flag: Option<String>) -> Result<String> {
    match flag {
        Some(p) if !p.is_empty() => Ok(p),
        _ => rpassword::prompt_password("Wallet password: ").context("password prompt failed"),
    }
}

/// Wrap plaintext password in zeroizing `Secret<Vec<u8>>` for the
/// library API. Wrapped once at the boundary; the plaintext `String`
/// is moved into the `Secret` (no copies survive past this point).
fn secret_password(plaintext: String) -> Secret<Vec<u8>> {
    Secret::new(plaintext.into_bytes())
}

pub async fn handle_create(
    words: WordCount,
    network: NetArg,
    password: Option<String>,
    data_dir: &Path,
) -> Result<()> {
    let n = words.as_usize();
    if !SUPPORTED_WORD_COUNTS.contains(&n) {
        anyhow::bail!("unsupported BIP-39 word count: {n} (supported: {SUPPORTED_WORD_COUNTS:?})");
    }
    let pwd_plain = password_or_prompt(password)?;
    let pwd = secret_password(pwd_plain);
    let network_obj = network.as_network();

    let (wallet_id, mnemonic) =
        create_wallet(data_dir, network_obj, n, &pwd).context("create_wallet failed")?;
    let wallet_id_str = wallet_id.to_string();
    let phrase = mnemonic.expose().to_string();

    // F49 / L28: wallet_id on STDOUT (scriptable), mnemonic on
    // STDERR (operator-only). The mnemonic never lands on STDOUT
    // so it can't leak via shell history, CI capture, or pipes.
    println!("{wallet_id_str}");
    eprintln!("Mnemonic (write this down; never stored in plaintext):");
    eprintln!("{phrase}");
    Ok(())
}

pub async fn handle_show(
    id: String,
    network: NetArg,
    password: Option<String>,
    esplora_url: Option<String>,
    data_dir: &Path,
) -> Result<()> {
    let pwd_plain = password_or_prompt(password)?;
    let pwd = secret_password(pwd_plain);
    let wallet_id = WalletId::from_str(&id).with_context(|| format!("parsing wallet_id {id:?}"))?;
    let network_obj = network.as_network();
    let url = esplora_url.as_deref().unwrap_or(DEFAULT_TESTNET_ESPLORA);
    let esplora_url_typed =
        EsploraUrl::new(url).with_context(|| format!("invalid --esplora-url {url:?}"))?;

    // L29 + F20: caller controls TLS policy. CLI default is
    // SystemRoots for the public testnet endpoint; production
    // deployments override with `EsploraClient::from_config` +
    // `TlsPolicy::Pinned` (CLI does not yet expose this — tracked
    // as v0.1.1 follow-up).
    let client = EsploraClient::new(esplora_url_typed, TlsPolicy::SystemRoots)
        .context("build Esplora client")?;

    let info = show_wallet(data_dir, network_obj, wallet_id, &pwd, &client)
        .await
        .context("show_wallet failed")?;

    // Pretty JSON to STDOUT for piping / scripting.
    let json = serde_json::to_string_pretty(&info).context("serializing wallet info")?;
    println!("{json}");
    Ok(())
}
