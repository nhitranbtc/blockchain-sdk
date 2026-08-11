//! Handlers for `btc wallet create` + `btc wallet show` subcommands.
//!
//! **F49 / L28**: wallet_id → STDOUT (scriptable), mnemonic → STDERR
//! (operator-only). Regression test enforces in `tests/cli.rs`.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};

use bitcoin_wallet_core::chain::esplora::{EsploraClient, TlsPolicy};
use bitcoin_wallet_core::chain::esplora_url::EsploraUrl;
use bitcoin_wallet_core::chain::spki::SpkiPin;
use bitcoin_wallet_core::config::WalletConfig;
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

/// Parse a 64-char hex SPKI pin string into `SpkiPin`.
///
/// **Format**: SHA-256 of the leaf cert's SubjectPublicKeyInfo, as 64
/// lowercase or uppercase hex chars. The CLI accepts this format
/// (operator-friendly) and converts to 32 raw bytes internally; the
/// library's `SpkiPin::from_base64` would require base64 encoding
/// instead — we deliberately use hex for CLI ergonomics (operators
/// copy SPKI hashes from TLS inspection tools that typically render
/// hex).
///
/// # Errors
///
/// `anyhow::Error` with `context` on:
/// - wrong length (not 64 chars)
/// - non-hex characters
pub fn parse_spki_pin_hex(s: &str) -> Result<SpkiPin> {
    if s.len() != 64 {
        anyhow::bail!(
            "SPKI pin must be 64 hex chars (SHA-256 of SubjectPublicKeyInfo); got {} chars",
            s.len()
        );
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).context("SPKI pin contains non-ASCII byte")?;
        bytes[i] = u8::from_str_radix(pair, 16)
            .with_context(|| format!("SPKI pin contains non-hex pair {pair:?} at position {i}"))?;
    }
    Ok(SpkiPin::from_bytes(bytes))
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
    esplora_spki_pin: Option<String>,
    data_dir: &Path,
) -> Result<()> {
    let pwd_plain = password_or_prompt(password)?;
    let pwd = secret_password(pwd_plain);
    let wallet_id = WalletId::from_str(&id).with_context(|| format!("parsing wallet_id {id:?}"))?;
    let network_obj = network.as_network();
    let url = esplora_url.as_deref().unwrap_or(DEFAULT_TESTNET_ESPLORA);
    let esplora_url_typed =
        EsploraUrl::new(url).with_context(|| format!("invalid --esplora-url {url:?}"))?;

    // F20 enforcement: when the operator passes `--esplora-spki-pin`
    // (or `BTC_ESPLORA_SPKI_PIN` env), route through
    // `EsploraClient::from_config` which applies `TlsPolicy::Pinned`.
    // When unset, fall back to `EsploraClient::new(url, SystemRoots)`
    // (PR-2 default — testnet-suitable; mainnet/signet/regtest without
    // a pin will fail at network level per ADR 0001 / F20).
    let client = match esplora_spki_pin {
        Some(pin_hex) => {
            let pin = parse_spki_pin_hex(&pin_hex)
                .with_context(|| format!("parsing --esplora-spki-pin ({pin_hex:?})"))?;
            // `db_path` is unused for show (only the lib's bdk_file_store
            // integration needs it, deferred per F14). Use a placeholder
            // path so the WalletConfig builds cleanly. Show does not
            // touch the sidecar DB.
            let mut cfg = WalletConfig::testnet(url, data_dir.join("placeholder.sqlite"))
                .with_esplora_spki_pin(pin);
            // Override the testnet default with the actual requested
            // network. WalletConfig has dedicated constructors for
            // Bitcoin/Signet/Regtest but not Testnet4, so we mutate
            // the pub `network` field (allowed despite
            // `#[non_exhaustive]`).
            cfg.network = network_obj;
            EsploraClient::from_config(&cfg).context("build pinned Esplora client")?
        }
        None => EsploraClient::new(esplora_url_typed, TlsPolicy::SystemRoots)
            .context("build Esplora client (SystemRoots)")?,
    };

    let info = show_wallet(data_dir, network_obj, wallet_id, &pwd, &client)
        .await
        .context("show_wallet failed")?;

    // Pretty JSON to STDOUT for piping / scripting.
    let json = serde_json::to_string_pretty(&info).context("serializing wallet info")?;
    println!("{json}");
    Ok(())
}
