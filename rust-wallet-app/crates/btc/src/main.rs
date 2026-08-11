//! `btc` — minimal CLI for the `bitcoin-wallet-core` library.
//!
//! Subcommands:
//!
//! - `btc wallet create --words <N> --network <NET> [--password <PWD>]` —
//!   generate a fresh BIP-39 mnemonic, persist encrypted wallet, print
//!   `wallet_id` to STDOUT and the mnemonic to STDERR (with banner).
//! - `btc wallet show <ID> --network <NET> [--password <PWD>] [--esplora-url <URL>]` —
//!   load + decrypt wallet, sync from Esplora, print addresses + balance
//!   to STDOUT (JSON).
//!
//! Per ADR 0001 (`docs/superpowers/adrs/2026-08-11-adr-0001-btc-wallet-store.md`)
//! the wallet lives at `$XDG_DATA_HOME/btc/wallets/<network>/<id>.enc`,
//! encrypted with AES-GCM-AEAD with `bitcoin::Network` discriminant bound
//! via AAD (closes N5 cross-network footgun).
//!
//! **L28 honesty (Issue #64 acceptance):** the mnemonic NEVER appears on
//! STDOUT — it goes to STDERR with a banner. Regression test enforces
//! this in `wallet::tests::create_writes_mnemonic_to_stderr_not_stdout`.

use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result};
use bitcoin::Network;
use bitcoin_wallet_core::chain::esplora::{EsploraClient, TlsPolicy};
use bitcoin_wallet_core::wallet::WalletId;
use clap::{Parser, Subcommand, ValueEnum};

mod wallet;

/// Default Esplora URL per network. Operators can override via
/// `--esplora-url`. Default testnet = blockstream.info (public testnet
/// Esplora, `TlsPolicy::SystemRoots` acceptable for demo per F20).
fn default_esplora_url(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => "https://blockstream.info/api",
        Network::Testnet => "https://blockstream.info/testnet/api",
        Network::Signet => "https://mempool.space/signet/api",
        Network::Regtest => "http://127.0.0.1:3002",
        Network::Testnet4 => "https://mempool.space/testnet4/api",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum NetArg {
    Bitcoin,
    Testnet,
    Testnet4,
    Signet,
    Regtest,
}

impl From<NetArg> for Network {
    fn from(n: NetArg) -> Self {
        match n {
            NetArg::Bitcoin => Network::Bitcoin,
            NetArg::Testnet => Network::Testnet,
            NetArg::Testnet4 => Network::Testnet4,
            NetArg::Signet => Network::Signet,
            NetArg::Regtest => Network::Regtest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WordsArg {
    W12,
    W15,
    W18,
    W21,
    W24,
}

impl From<WordsArg> for usize {
    fn from(w: WordsArg) -> Self {
        match w {
            WordsArg::W12 => 12,
            WordsArg::W15 => 15,
            WordsArg::W18 => 18,
            WordsArg::W21 => 21,
            WordsArg::W24 => 24,
        }
    }
}

#[derive(Parser)]
#[command(name = "btc", version, about = "Bitcoin wallet CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// Manual Debug impls redact the `password` field of `WalletAction`.
// Defense-in-depth: any future `dbg!`, `tracing::debug!`, or panic
// message that touches `Cli` cannot leak the operator's password.
// (L12 review CRITICAL #2.)
impl std::fmt::Debug for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cli")
            .field("command", &self.command)
            .finish()
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Wallet persistence subcommands (Task 54d / Issue #64).
    Wallet {
        #[command(subcommand)]
        action: WalletAction,
    },
}

impl std::fmt::Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Commands::Wallet { action } => {
                f.debug_struct("Wallet").field("action", action).finish()
            }
        }
    }
}

#[derive(Subcommand)]
enum WalletAction {
    /// Generate a new wallet, persist encrypted blob, print wallet ID.
    Create {
        /// BIP-39 word count: 12, 15, 18, 21, or 24.
        #[arg(long, value_enum, default_value_t = WordsArg::W12)]
        words: WordsArg,
        /// Bitcoin network.
        #[arg(long, value_enum, default_value_t = NetArg::Testnet)]
        network: NetArg,
        /// Encryption password (omit to prompt securely).
        #[arg(long)]
        password: Option<String>,
    },
    /// Load a wallet, sync from Esplora, print addresses + balance.
    Show {
        /// Wallet ID (UUID v4).
        id: String,
        /// Bitcoin network (must match the network the wallet was
        /// created for; mismatch surfaces as indistinguishable
        /// "wallet not accessible" error).
        #[arg(long, value_enum, default_value_t = NetArg::Testnet)]
        network: NetArg,
        /// Encryption password (omit to prompt securely).
        #[arg(long)]
        password: Option<String>,
        /// Esplora URL (default = blockstream.info for the network).
        #[arg(long)]
        esplora_url: Option<String>,
    },
}

impl std::fmt::Debug for WalletAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Redact password — never print it via Debug.
        match self {
            WalletAction::Create {
                words,
                network,
                password: _,
            } => f
                .debug_struct("Create")
                .field("words", words)
                .field("network", network)
                .field("password", &"<redacted>")
                .finish(),
            WalletAction::Show {
                id,
                network,
                password: _,
                esplora_url,
            } => f
                .debug_struct("Show")
                .field("id", id)
                .field("network", network)
                .field("password", &"<redacted>")
                .field("esplora_url", esplora_url)
                .finish(),
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    match run(cli, &mut stdout, &mut stderr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Errors go to stderr (NOT stdout — stdout stays parseable).
            let _ = writeln!(stderr, "error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// CLI runner — takes parsed args + writers, dispatches to handlers.
/// `pub(crate)` so tests in `wallet` module can call directly without
/// spawning a process.
pub(crate) fn run<O: Write, E: Write>(cli: Cli, stdout: &mut O, stderr: &mut E) -> Result<()> {
    match cli.command {
        Commands::Wallet { action } => match action {
            WalletAction::Create {
                words,
                network,
                password,
            } => wallet::create_handler(words.into(), network.into(), password, stdout, stderr),
            WalletAction::Show {
                id,
                network,
                password,
                esplora_url,
            } => {
                let id: WalletId = id.parse().context("invalid wallet id")?;
                let url =
                    esplora_url.unwrap_or_else(|| default_esplora_url(network.into()).to_string());
                let esplora = build_esplora_client(&url)?;
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("build tokio runtime")?;
                rt.block_on(wallet::show_handler(
                    network.into(),
                    id,
                    password,
                    &esplora,
                    stdout,
                    stderr,
                ))?;
                Ok(())
            }
        },
    }
}

fn build_esplora_client(url: &str) -> Result<EsploraClient> {
    let parsed = bitcoin_wallet_core::chain::esplora_url::EsploraUrl::new(url)
        .map_err(|e| anyhow::anyhow!("invalid esplora url: {e}"))?;
    EsploraClient::new(parsed, TlsPolicy::SystemRoots)
        .map_err(|e| anyhow::anyhow!("build esplora client: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_arg_maps_to_bitcoin_network() {
        assert_eq!(Network::from(NetArg::Bitcoin), Network::Bitcoin);
        assert_eq!(Network::from(NetArg::Testnet), Network::Testnet);
        assert_eq!(Network::from(NetArg::Testnet4), Network::Testnet4);
        assert_eq!(Network::from(NetArg::Signet), Network::Signet);
        assert_eq!(Network::from(NetArg::Regtest), Network::Regtest);
    }

    #[test]
    fn words_arg_maps_to_count() {
        assert_eq!(usize::from(WordsArg::W12), 12);
        assert_eq!(usize::from(WordsArg::W15), 15);
        assert_eq!(usize::from(WordsArg::W18), 18);
        assert_eq!(usize::from(WordsArg::W21), 21);
        assert_eq!(usize::from(WordsArg::W24), 24);
    }

    #[test]
    fn default_esplora_urls_match_network() {
        assert!(default_esplora_url(Network::Bitcoin).contains("blockstream.info/api"));
        assert!(default_esplora_url(Network::Testnet).contains("testnet/api"));
        assert!(default_esplora_url(Network::Signet).contains("signet"));
    }
}
