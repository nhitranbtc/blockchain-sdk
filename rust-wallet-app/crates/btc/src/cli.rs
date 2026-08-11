//! CLI argument types for `btc` binary (PR-2, #70).
//!
//! **Manual `Debug` impls redact password** (L12 CRITICAL #2 from
//! prior session) — auto-derived `Debug` on password-bearing structs
//! leaks the secret into `tracing::debug!(?cli)` output.

use std::fmt;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Top-level CLI args. `data_dir` is global so all subcommands
/// inherit the override (and the `BTC_DATA_DIR` env fallback).
#[derive(Parser)]
#[command(name = "btc", version, about = "Bitcoin wallet CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    /// Data directory override (default: `$XDG_DATA_HOME/btc`).
    #[arg(long, global = true, env = "BTC_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    Wallet(WalletAction),
}

#[derive(clap::Args)]
pub struct WalletAction {
    #[command(subcommand)]
    pub action: WalletActionKind,
}

#[derive(Subcommand)]
pub enum WalletActionKind {
    /// Create a new wallet, encrypt its mnemonic, persist to disk.
    Create {
        /// BIP-39 word count.
        #[arg(long, value_enum)]
        words: WordCount,
        /// Bitcoin network.
        #[arg(long, value_enum)]
        network: NetArg,
        /// Wallet encryption password (omit to prompt securely).
        #[arg(long)]
        password: Option<String>,
    },
    /// Decrypt a wallet, sync from Esplora, print addresses + balance.
    Show {
        /// Wallet ID (UUID v4) printed by `wallet create`.
        id: String,
        /// Bitcoin network the wallet was created for.
        #[arg(long, value_enum)]
        network: NetArg,
        /// Wallet password (omit to prompt securely).
        #[arg(long)]
        password: Option<String>,
        /// Esplora base URL (default: blockstream.info testnet).
        #[arg(long)]
        esplora_url: Option<String>,
        /// Esplora leaf cert SPKI pin (64-char hex, SHA-256 of the
        /// SubjectPublicKeyInfo). When set, the Esplora client is built
        /// via `EsploraClient::from_config` with `TlsPolicy::Pinned`
        /// (F20 enforcement). Required for mainnet/signet/regtest
        /// production endpoints. Env: `BTC_ESPLORA_SPKI_PIN`.
        #[arg(long, env = "BTC_ESPLORA_SPKI_PIN")]
        esplora_spki_pin: Option<String>,
    },
}

/// BIP-39 mnemonic word counts supported by the library
/// (`SUPPORTED_WORD_COUNTS`). Enforced at CLI boundary.
///
/// `#[value(name = "12")]` overrides the auto-derived `w12` /
/// `w15` / ... so callers pass the conventional integer
/// (`--words 12`, not `--words w12`). Aliases `w12`/etc. kept
/// for shell-completion discoverability.
#[derive(Copy, Clone, ValueEnum, Debug)]
pub enum WordCount {
    #[value(name = "12", alias = "w12")]
    W12,
    #[value(name = "15", alias = "w15")]
    W15,
    #[value(name = "18", alias = "w18")]
    W18,
    #[value(name = "21", alias = "w21")]
    W21,
    #[value(name = "24", alias = "w24")]
    W24,
}

impl WordCount {
    pub fn as_usize(self) -> usize {
        match self {
            Self::W12 => 12,
            Self::W15 => 15,
            Self::W18 => 18,
            Self::W21 => 21,
            Self::W24 => 24,
        }
    }
}

/// CLI-facing network enum. Maps to `bitcoin::Network` discriminant
/// used by the library for the AAD bound + path layout.
#[derive(Copy, Clone, ValueEnum, Debug)]
pub enum NetArg {
    Bitcoin,
    Testnet,
    Testnet4,
    Signet,
    Regtest,
}

impl NetArg {
    pub fn as_network(self) -> bitcoin::Network {
        match self {
            Self::Bitcoin => bitcoin::Network::Bitcoin,
            Self::Testnet => bitcoin::Network::Testnet,
            Self::Testnet4 => bitcoin::Network::Testnet4,
            Self::Signet => bitcoin::Network::Signet,
            Self::Regtest => bitcoin::Network::Regtest,
        }
    }
}

// --- Manual Debug impls (L12 CRITICAL #2: redact password) ---

impl fmt::Debug for Cli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cli")
            .field("command", &self.command)
            .field("data_dir", &self.data_dir)
            .finish()
    }
}

impl fmt::Debug for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wallet(w) => f.debug_tuple("Wallet").field(w).finish(),
        }
    }
}

impl fmt::Debug for WalletAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WalletAction")
            .field("action", &self.action)
            .finish()
    }
}

impl fmt::Debug for WalletActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Destructure to drop `password` from the printed fields.
        // The `_` pattern forces exhaustiveness — adding a new field
        // here forces a manual decision about redaction.
        match self {
            Self::Create {
                words,
                network,
                password: _,
            } => f
                .debug_struct("Create")
                .field("words", words)
                .field("network", network)
                .field("password", &"<redacted>")
                .finish(),
            Self::Show {
                id,
                network,
                password: _,
                esplora_url,
                esplora_spki_pin,
            } => f
                .debug_struct("Show")
                .field("id", id)
                .field("network", network)
                .field("password", &"<redacted>")
                .field("esplora_url", esplora_url)
                .field("esplora_spki_pin", esplora_spki_pin)
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_create_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "testnet",
            "--password",
            "hunter2",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    #[test]
    fn parse_create_rejects_missing_words() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--network",
            "testnet",
            "--password",
            "hunter2",
        ]);
        assert!(cli.is_err(), "missing --words should fail to parse");
    }

    #[test]
    fn parse_show_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "show",
            "abc-uuid",
            "--network",
            "testnet",
            "--password",
            "hunter2",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    #[test]
    fn parse_show_rejects_missing_network() {
        let cli =
            Cli::try_parse_from(["btc", "wallet", "show", "abc-uuid", "--password", "hunter2"]);
        assert!(cli.is_err(), "missing --network should fail to parse");
    }

    #[test]
    fn parse_show_accepts_spki_pin_flag() {
        let pin_hex = "0".repeat(64);
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "show",
            "abc-uuid",
            "--network",
            "testnet",
            "--password",
            "hunter2",
            "--esplora-spki-pin",
            &pin_hex,
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    #[test]
    fn parse_show_accepts_spki_pin_env() {
        let pin_hex = "f".repeat(64);
        // env-vars + try_parse_from: clap reads env when the flag is
        // not present on the command line. Set via std::env::set_var;
        // restore after to avoid leaking across tests.
        // SAFETY: tests run single-threaded under cargo test by
        // default for binary crates. If run with --test-threads>1,
        // env races are possible but each test sets+unsets its own
        // var.
        // SAFETY: env mutation in tests is a known pattern; cargo
        // test defaults to --test-threads=1 unless overridden.
        unsafe {
            std::env::set_var("BTC_ESPLORA_SPKI_PIN", &pin_hex);
        }
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "show",
            "abc-uuid",
            "--network",
            "testnet",
            "--password",
            "hunter2",
        ]);
        unsafe {
            std::env::remove_var("BTC_ESPLORA_SPKI_PIN");
        }
        assert!(cli.is_ok(), "expected parse ok via env, got: {cli:?}");
        let cli = cli.unwrap();
        let Commands::Wallet(WalletAction {
            action: WalletActionKind::Show {
                esplora_spki_pin, ..
            },
        }) = cli.command
        else {
            panic!("expected Show subcommand");
        };
        assert_eq!(esplora_spki_pin.as_deref(), Some(pin_hex.as_str()));
    }

    /// L12 CRITICAL #2: password MUST NOT appear in Debug output
    /// for either subcommand. Tracing / log capture is the
    /// most likely leak vector.
    #[test]
    fn debug_redacts_password_in_create() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "testnet",
            "--password",
            "hunter2",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("hunter2"),
            "password leaked in Debug: {debug}"
        );
        assert!(
            debug.contains("redacted"),
            "redaction marker missing: {debug}"
        );
    }

    #[test]
    fn debug_redacts_password_in_show() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "show",
            "abc-uuid",
            "--network",
            "testnet",
            "--password",
            "hunter2",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("hunter2"),
            "password leaked in Debug: {debug}"
        );
    }

    #[test]
    fn word_count_maps_to_usize() {
        assert_eq!(WordCount::W12.as_usize(), 12);
        assert_eq!(WordCount::W15.as_usize(), 15);
        assert_eq!(WordCount::W18.as_usize(), 18);
        assert_eq!(WordCount::W21.as_usize(), 21);
        assert_eq!(WordCount::W24.as_usize(), 24);
    }

    #[test]
    fn net_arg_maps_to_bitcoin_network() {
        assert!(matches!(
            NetArg::Testnet.as_network(),
            bitcoin::Network::Testnet
        ));
        assert!(matches!(
            NetArg::Bitcoin.as_network(),
            bitcoin::Network::Bitcoin
        ));
        assert!(matches!(
            NetArg::Testnet4.as_network(),
            bitcoin::Network::Testnet4
        ));
    }
}
