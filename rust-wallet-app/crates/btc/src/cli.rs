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
    /// BIP-137 message sign/verify (stateless; F21 typed MessageHash).
    /// PR for Issue #61 / Task 54a.
    Message(MessageAction),
    /// Encrypt a UTF-8 file with a password (F5 Argon2id + F6 AES-256-GCM).
    /// Output is a `MnemonicCipherBlob` (salt(16) || nonce(12) || ct || tag(16)).
    /// PR for Issue #62 / Task 54b.
    Encrypt(EncryptAction),
    /// Decrypt a `MnemonicCipherBlob` produced by `btc encrypt`.
    /// Errors on wrong password / tampered / truncated blob / non-UTF8
    /// plaintext (all surface as `Error::MnemonicCipher`).
    Decrypt(DecryptAction),
    /// Diagnostic config (Issue #100 / Story 11).
    /// `btc config show` prints / JSON-dumps the resolved CLI config
    /// (data dir, Esplora URL, network, loaded wallets) for debugging
    /// "why is this connecting to the wrong place" scenarios.
    Config(ConfigAction),
    /// Display current fee estimates from Esplora (Issue #121 /
    /// Story 8 / Task 14 `get_fee_estimates` portion). Read-only
    /// query against the configured `--esplora-url`; no wallet
    /// required. Pretty table by default; `--json` for
    /// machine-readable output.
    FeeEstimates(FeeEstimatesAction),
}

#[derive(clap::Args)]
pub struct WalletAction {
    #[command(subcommand)]
    pub action: WalletActionKind,
}

/// `btc message sign|verify` subcommands.
#[derive(clap::Args)]
pub struct MessageAction {
    #[command(subcommand)]
    pub action: MessageActionKind,
}

/// `btc encrypt` args — Issue #62 / Task 54b.
#[derive(clap::Args)]
pub struct EncryptAction {
    /// Encryption password. If omitted, prompts via `/dev/tty` (uses
    /// `rpassword`). **SECURITY**: `--password <PWD>` is visible in
    /// shell history, `ps`, and `/proc/<pid>/cmdline`. Prefer omitting
    /// the flag for interactive use, or set `BTC_ENCRYPT_PASSWORD`.
    #[arg(long, env = "BTC_ENCRYPT_PASSWORD", conflicts_with_all = ["password_file", "password_stdin"])]
    pub password: Option<String>,
    /// Read password bytes from a file path (Issue #84). Use case:
    /// k8s secrets mounted as files, systemd LoadCredential=,
    /// vault-agent sidecar. **SECURITY**: file must be mode `0o600`
    /// (not world/group-readable) and not a symlink — both checks
    /// happen at handler layer. Mutually exclusive with `--password`
    /// and `--password-stdin`.
    #[arg(long, conflicts_with_all = ["password", "password_stdin"])]
    pub password_file: Option<PathBuf>,
    /// Read password bytes from stdin (Issue #84). Use case: CI
    /// pipelines piping secrets from a wrapper. Reads all of stdin,
    /// trims trailing whitespace. Mutually exclusive with `--password`
    /// and `--password-file`.
    #[arg(long, conflicts_with_all = ["password", "password_file"])]
    pub password_stdin: bool,
    /// Input file (UTF-8 plaintext).
    #[arg(long)]
    pub r#in: PathBuf,
    /// Output file (binary blob).
    #[arg(long)]
    pub out: PathBuf,
}

/// `btc decrypt` args — Issue #62 / Task 54b.
#[derive(clap::Args)]
pub struct DecryptAction {
    /// Decryption password (must match the value used at encrypt time).
    /// If omitted, prompts via `/dev/tty`. See `EncryptAction::password`
    /// for the security caveats of the `--password` flag form.
    #[arg(long, env = "BTC_DECRYPT_PASSWORD", conflicts_with_all = ["password_file", "password_stdin"])]
    pub password: Option<String>,
    /// Read password bytes from a file path (Issue #84). Mirror of
    /// `EncryptAction::password_file`. Same security checks apply.
    #[arg(long, conflicts_with_all = ["password", "password_stdin"])]
    pub password_file: Option<PathBuf>,
    /// Read password bytes from stdin (Issue #84). Mirror of
    /// `EncryptAction::password_stdin`.
    #[arg(long, conflicts_with_all = ["password", "password_file"])]
    pub password_stdin: bool,
    /// Input file (binary blob).
    #[arg(long)]
    pub r#in: PathBuf,
    /// Output file (UTF-8 plaintext).
    #[arg(long)]
    pub out: PathBuf,
}

/// `btc fee-estimates` args — Issue #121 / Story 8. Read-only
/// Esplora fee estimator; mirrors `btc wallet sync` arg shape.
#[derive(clap::Args)]
pub struct FeeEstimatesAction {
    /// Bitcoin network (used for default Esplora URL when
    /// `--esplora-url` is not provided).
    #[arg(long, value_enum)]
    pub network: NetArg,
    /// Esplora base URL (HTTPS-only per F36; defaults to network's
    /// canonical endpoint if omitted).
    #[arg(long)]
    pub esplora_url: Option<String>,
    /// Esplora SPKI pin. Required for non-regtest networks per
    /// F20; regtest is exempted.
    #[arg(long, env = "BTC_ESPLORA_SPKI_PIN")]
    pub pin_spki: Option<String>,
    /// Output as JSON instead of a pretty table.
    #[arg(long)]
    pub json: bool,
}

/// `btc config` args — Issue #100 / Story 11.
#[derive(clap::Args)]
pub struct ConfigAction {
    #[command(subcommand)]
    pub action: ConfigActionKind,
}

#[derive(Subcommand)]
pub enum ConfigActionKind {
    /// Show the resolved CLI configuration (data dir, Esplora URL,
    /// network, list of loaded wallets). Exit 0 always.
    Show {
        /// Output as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
}

impl fmt::Debug for ConfigAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No secrets in this variant; safe to debug.
        f.debug_struct("ConfigAction")
            .field("action", &self.action)
            .finish()
    }
}

impl fmt::Debug for ConfigActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Show { json } => f.debug_struct("Show").field("json", json).finish(),
        }
    }
}

impl fmt::Debug for FeeEstimatesAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `pin_spki` is secret-adjacent (reveals which TLS leaf the
        // operator trusts) but not the secret itself — it's the
        // SHA-256 of the SubjectPublicKeyInfo. We surface it in
        // Debug output so tracing operators can see which pin
        // pattern is active (mirrors `Sync`/`Balance` redaction
        // policy).
        f.debug_struct("FeeEstimatesAction")
            .field("network", &self.network)
            .field("esplora_url", &self.esplora_url)
            .field("pin_spki", &self.pin_spki)
            .field("json", &self.json)
            .finish()
    }
}

#[derive(Subcommand)]
pub enum MessageActionKind {
    /// Sign a message with the BIP-137 scheme. Prints a base64-encoded
    /// recoverable signature (`header || compact(64)`).
    Sign {
        /// BIP-39 mnemonic phrase (12/15/18/21/24 words, space-separated).
        /// **SECURITY**: visible in shell history. Recommend
        /// `btc message sign --mnemonic "$(cat /tmp/mnemonic.txt)" ...`
        /// or pass via `read -s` + env wrapper for interactive use.
        #[arg(long)]
        mnemonic: String,
        /// Bitcoin network (mainnet/signet/regtest/testnet/testnet4).
        #[arg(long, value_enum)]
        network: NetArg,
        /// Bitcoin address. v0.1 (Issue #61 scope): must match the
        /// first external receive address derived from the mnemonic
        /// at `m/44'/coin'/0'/0/0`. Signing from non-default addresses
        /// deferred to v0.1.1.
        #[arg(long)]
        address: String,
        /// Message text to sign. Quoted on the command line.
        message: String,
    },
    /// Verify a BIP-137 signature. Exits 0 if valid, 1 if invalid.
    Verify {
        /// Bitcoin address that allegedly signed the message.
        #[arg(long)]
        address: String,
        /// Message text that was signed.
        message: String,
        /// Base64-encoded BIP-137 signature (output of `btc message sign`).
        signature: String,
    },
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
    /// Import an existing BIP-39 mnemonic, encrypt, persist to disk
    /// (Issue #99 / Story 2). Generates a new WalletId; the phrase
    /// is not echoed back (caller already has it).
    ///
    /// **Note:** BIP-39 passphrase support was removed (security review
    /// found the flag accepted `--passphrase` but the lib hardcoded
    /// empty passphrase at derivation — broken-security-control).
    /// v0.1 stores mnemonic phrase only. Passphrase support is a
    /// follow-up that threads through `import_wallet` → `build_bdk_wallet`.
    Import {
        /// BIP-39 mnemonic phrase (12/15/18/21/24 words, space-separated).
        /// **SECURITY**: visible in shell history if passed as
        /// `--mnemonic "..."` — prefer piping via `read -s` or env.
        #[arg(long, env = "BTC_WALLET_MNEMONIC")]
        mnemonic: String,
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
    /// Statelessly sync a BIP-39 mnemonic against an Esplora server
    /// (Issue #63 / Task 54c). Prints UTXO count + total sats. No
    /// wallet persistence — mnemonic lives in process memory only.
    Sync {
        /// BIP-39 mnemonic phrase (12/15/18/21/24 words). **SECURITY**:
        /// visible in shell history if passed as `--mnemonic "..."` —
        /// prefer piping via `read -s` or env (`BTC_WALLET_MNEMONIC`).
        #[arg(long, env = "BTC_WALLET_MNEMONIC")]
        mnemonic: String,
        /// Bitcoin network.
        #[arg(long, value_enum)]
        network: NetArg,
        /// Esplora base URL (HTTPS-only per F36).
        #[arg(long)]
        esplora_url: String,
        /// Esplora SPKI pin (64-char hex, SHA-256 of leaf cert).
        /// Required for non-regtest networks (F20 enforcement). Regtest
        /// localhost behind stunnel may pass `--pin-spki` to lock the
        /// TLS path; or omit for `--network regtest` only.
        #[arg(long, env = "BTC_ESPLORA_SPKI_PIN")]
        pin_spki: Option<String>,
    },
    /// Statelessly fetch the confirmed balance for a BIP-39 mnemonic
    /// (Issue #63 / Task 54c). Prints sats to STDOUT. No wallet
    /// persistence — mnemonic lives in process memory only.
    Balance {
        /// BIP-39 mnemonic phrase (12/15/18/21/24 words). **SECURITY**:
        /// see `Sync::mnemonic` for shell-history caveats.
        #[arg(long, env = "BTC_WALLET_MNEMONIC")]
        mnemonic: String,
        /// Bitcoin network.
        #[arg(long, value_enum)]
        network: NetArg,
        /// Esplora base URL (HTTPS-only per F36).
        #[arg(long)]
        esplora_url: String,
        /// Esplora SPKI pin. See `Sync::pin_spki` for F20 enforcement.
        #[arg(long, env = "BTC_ESPLORA_SPKI_PIN")]
        pin_spki: Option<String>,
    },
    /// Send satoshis to a recipient address (Story 5 / Issue #118).
    /// Full tx lifecycle: sync → build → sign → broadcast → return txid.
    /// Default fee rate is 1 sat/vB (Story 6 #119 adds --fee-rate override).
    ///
    /// **SECURITY**: mnemonic is the wallet's key material — see
    /// `Sync::mnemonic` for shell-history caveats. The recipient address
    /// must be a valid address on the `--network` (cross-network
    /// rejection enforced at handler layer).
    Send {
        /// BIP-39 mnemonic phrase (12/15/18/21/24 words).
        /// **SECURITY**: see `Sync::mnemonic` for shell-history caveats.
        #[arg(long, env = "BTC_WALLET_MNEMONIC")]
        mnemonic: String,
        /// Bitcoin network.
        #[arg(long, value_enum)]
        network: NetArg,
        /// Recipient Bitcoin address (must match `--network`).
        #[arg(long)]
        address: String,
        /// Amount to send in satoshis (u64). Must exceed dust limit
        /// (546 sat for native segwit P2WPKH).
        #[arg(long)]
        amount_sat: u64,
        /// Esplora base URL (HTTPS-only per F36).
        #[arg(long)]
        esplora_url: String,
        /// Esplora SPKI pin. See `Sync::pin_spki` for F20 enforcement.
        #[arg(long, env = "BTC_ESPLORA_SPKI_PIN")]
        pin_spki: Option<String>,
        /// Fee rate in sat/vB (Story 6 / Issue #119). Must be `>= 1`
        /// (0 is invalid — txs without fee don't relay). Default
        /// (`None`): 1 sat/vB (conservative; Story 8 #121 will
        /// fetch Esplora estimates when `--fee-rate` is omitted).
        #[arg(long = "fee-rate", alias = "fee-rate-sat-per-vb")]
        fee_rate_sat_per_vb: Option<u64>,
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
            Self::Message(m) => f.debug_tuple("Message").field(m).finish(),
            Self::Encrypt(e) => f.debug_tuple("Encrypt").field(e).finish(),
            Self::Decrypt(d) => f.debug_tuple("Decrypt").field(d).finish(),
            Self::Config(c) => f.debug_tuple("Config").field(c).finish(),
            Self::FeeEstimates(fe) => f.debug_tuple("FeeEstimates").field(fe).finish(),
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

impl fmt::Debug for MessageAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageAction")
            .field("action", &self.action)
            .finish()
    }
}

impl fmt::Debug for EncryptAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // L12 CRITICAL #2: redact password (secret material) — None or Some.
        let pwd_display = if self.password.is_some() {
            "<redacted>"
        } else {
            "<unset — will prompt>"
        };
        // password_file path is secret-adjacent (reveals where
        // secrets live on disk). Issue #84.
        let pwd_file_display = if self.password_file.is_some() {
            "<redacted — path>"
        } else {
            "<unset>"
        };
        // password_stdin boolean is not secret (no value to leak).
        f.debug_struct("EncryptAction")
            .field("password", &pwd_display)
            .field("password_file", &pwd_file_display)
            .field("password_stdin", &self.password_stdin)
            .field("in", &self.r#in)
            .field("out", &self.out)
            .finish()
    }
}

impl fmt::Debug for DecryptAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // L12 CRITICAL #2: redact password (secret material).
        let pwd_display = if self.password.is_some() {
            "<redacted>"
        } else {
            "<unset — will prompt>"
        };
        let pwd_file_display = if self.password_file.is_some() {
            "<redacted — path>"
        } else {
            "<unset>"
        };
        f.debug_struct("DecryptAction")
            .field("password", &pwd_display)
            .field("password_file", &pwd_file_display)
            .field("password_stdin", &self.password_stdin)
            .field("in", &self.r#in)
            .field("out", &self.out)
            .finish()
    }
}

impl fmt::Debug for MessageActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // L12 CRITICAL #2: redact mnemonic (secret material) + signature
        // (could leak via logs).
        match self {
            Self::Sign {
                mnemonic: _,
                network,
                address,
                message,
            } => f
                .debug_struct("Sign")
                .field("mnemonic", &"<redacted>")
                .field("network", network)
                .field("address", address)
                .field("message", message)
                .finish(),
            Self::Verify {
                address,
                message,
                signature: _,
            } => f
                .debug_struct("Verify")
                .field("address", address)
                .field("message", message)
                .field("signature", &"<redacted>")
                .finish(),
        }
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
            Self::Import {
                mnemonic: _,
                network,
                password: _,
            } => f
                .debug_struct("Import")
                .field("mnemonic", &"<redacted>")
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
            // L12 CRITICAL #2 (Issue #63): mnemonic is secret material.
            // Redact both `mnemonic` fields; show everything else.
            Self::Sync {
                mnemonic: _,
                network,
                esplora_url,
                pin_spki,
            } => f
                .debug_struct("Sync")
                .field("mnemonic", &"<redacted>")
                .field("network", network)
                .field("esplora_url", esplora_url)
                .field("pin_spki", pin_spki)
                .finish(),
            Self::Balance {
                mnemonic: _,
                network,
                esplora_url,
                pin_spki,
            } => f
                .debug_struct("Balance")
                .field("mnemonic", &"<redacted>")
                .field("network", network)
                .field("esplora_url", esplora_url)
                .field("pin_spki", pin_spki)
                .finish(),
            // Story 5 / Issue #118: same redaction policy as Sync/Balance
            // (mnemonic is secret material — never appear in Debug).
            Self::Send {
                mnemonic: _,
                network,
                address,
                amount_sat,
                esplora_url,
                pin_spki,
                fee_rate_sat_per_vb,
            } => f
                .debug_struct("Send")
                .field("mnemonic", &"<redacted>")
                .field("network", network)
                .field("address", address)
                .field("amount_sat", amount_sat)
                .field("esplora_url", esplora_url)
                .field("pin_spki", pin_spki)
                .field("fee_rate_sat_per_vb", fee_rate_sat_per_vb)
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
    fn parse_sync_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "sync",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    #[test]
    fn parse_balance_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "balance",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    #[test]
    fn parse_sync_rejects_missing_mnemonic() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "sync",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_err(), "missing --mnemonic should fail to parse");
    }

    #[test]
    fn parse_balance_rejects_missing_mnemonic() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "balance",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_err(), "missing --mnemonic should fail to parse");
    }

    /// Story 5 / Issue #118: `btc wallet send` parses required args.
    /// Full coverage of the happy path is in handler tests (which
    /// would require a funded wallet — covered by integration tests).
    #[test]
    fn parse_send_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--address",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--amount-sat",
            "10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 5 / Issue #118: missing `--address` must fail to parse
    /// (handler-layer cross-network check would also catch this, but
    /// the parse-layer guard gives a faster error).
    #[test]
    fn parse_send_rejects_missing_address() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--amount-sat",
            "10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_err(), "missing --address should fail to parse");
    }

    /// Story 5 / Issue #118: missing `--amount-sat` must fail to
    /// parse.
    #[test]
    fn parse_send_rejects_missing_amount() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--address",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_err(), "missing --amount-sat should fail to parse");
    }

    /// Story 5 / Issue #118 (L12 CRITICAL #2): mnemonic MUST NOT
    /// appear in Debug output for the new `send` subcommand.
    /// Tracing / log capture is the most likely leak vector.
    #[test]
    fn debug_redacts_mnemonic_in_send() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--address",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--amount-sat",
            "10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("abandon"),
            "mnemonic leaked in Debug: {debug}"
        );
        assert!(
            debug.contains("redacted"),
            "redaction marker missing: {debug}"
        );
    }

    /// Story 6 / Issue #119: `btc wallet send --fee-rate <sat/vb>`
    /// parses the fee-rate flag. Validation (>= 1) is at handler
    /// layer (clap `value_parser` would reject 0 — but we surface
    /// the error in `handle_wallet_send` for a friendlier message).
    #[test]
    fn parse_send_accepts_fee_rate_flag() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--address",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--amount-sat",
            "10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
            "--fee-rate",
            "5",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 6 / Issue #119 (L13 step 14 defensive test): Debug
    /// output for `send` does not echo the fee rate in a way that
    /// would change log-capture behavior. (Fee rate is not secret
    /// material — only mnemonic + password are. This test pins the
    /// existing redaction pattern.)
    #[test]
    fn debug_includes_fee_rate_in_send() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--address",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--amount-sat",
            "10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
            "--fee-rate",
            "42",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("abandon"),
            "mnemonic leaked in Debug: {debug}"
        );
        assert!(
            debug.contains("42"),
            "fee_rate_sat_per_vb should appear in Debug; got {debug}"
        );
    }

    /// Issue #63 SPKI enforcement at parse layer: the `--pin-spki`
    /// alias (`--esplora-spki-pin`) must be available on the new
    /// subcommands. Surface-level parse test — runtime enforcement
    /// (non-regtest + no pin = fail) is a handler-layer test.
    #[test]
    fn parse_sync_accepts_pin_spki_flag() {
        let pin_hex = "0".repeat(64);
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "sync",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
            "--pin-spki",
            &pin_hex,
        ]);
        assert!(
            cli.is_ok(),
            "expected parse ok with --pin-spki, got: {cli:?}"
        );
    }

    #[test]
    fn parse_balance_accepts_pin_spki_flag() {
        let pin_hex = "0".repeat(64);
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "balance",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
            "--pin-spki",
            &pin_hex,
        ]);
        assert!(
            cli.is_ok(),
            "expected parse ok with --pin-spki, got: {cli:?}"
        );
    }

    /// Issue #62 L12 CRITICAL #2: mnemonic MUST NOT appear in Debug
    /// output for sync/balance subcommands (same redaction policy as
    /// `message sign`). Tracing / log capture is the most likely leak
    /// vector.
    #[test]
    fn debug_redacts_mnemonic_in_sync() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "sync",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("abandon"),
            "mnemonic leaked in Debug: {debug}"
        );
        assert!(
            debug.contains("redacted"),
            "redaction marker missing: {debug}"
        );
    }

    #[test]
    fn debug_redacts_mnemonic_in_balance() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "balance",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("abandon"),
            "mnemonic leaked in Debug: {debug}"
        );
        assert!(
            debug.contains("redacted"),
            "redaction marker missing: {debug}"
        );
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
    fn parse_encrypt_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password",
            "hunter2",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Issue #84: --password-file path (scripted/automation supply).
    /// Reads password bytes from the file path instead of an env var
    /// or /dev/tty prompt. Use case: k8s secrets mounted as files,
    /// systemd LoadCredential=, vault-agent sidecar.
    #[test]
    fn parse_encrypt_accepts_password_file() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password-file",
            "/run/secrets/btc-pwd",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Issue #84: --password-stdin (CI pipeline supply).
    /// Reads password bytes from stdin.
    #[test]
    fn parse_encrypt_accepts_password_stdin() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password-stdin",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Issue #84: --password and --password-file are mutually
    /// exclusive (operator must pick exactly one supply path).
    #[test]
    fn parse_encrypt_rejects_password_and_password_file() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password",
            "hunter2",
            "--password-file",
            "/run/secrets/btc-pwd",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
        ]);
        assert!(
            cli.is_err(),
            "--password + --password-file must be mutually exclusive"
        );
    }

    /// Issue #84: --password and --password-stdin are mutually exclusive.
    #[test]
    fn parse_encrypt_rejects_password_and_password_stdin() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password",
            "hunter2",
            "--password-stdin",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
        ]);
        assert!(
            cli.is_err(),
            "--password + --password-stdin must be mutually exclusive"
        );
    }

    /// Issue #84: --password-file and --password-stdin are mutually
    /// exclusive.
    #[test]
    fn parse_encrypt_rejects_password_file_and_password_stdin() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password-file",
            "/run/secrets/btc-pwd",
            "--password-stdin",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
        ]);
        assert!(
            cli.is_err(),
            "--password-file + --password-stdin must be mutually exclusive"
        );
    }

    #[test]
    fn parse_decrypt_accepts_password_file() {
        let cli = Cli::try_parse_from([
            "btc",
            "decrypt",
            "--password-file",
            "/run/secrets/btc-pwd",
            "--in",
            "/tmp/cipher.enc",
            "--out",
            "/tmp/recovered.txt",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    #[test]
    fn parse_decrypt_accepts_password_stdin() {
        let cli = Cli::try_parse_from([
            "btc",
            "decrypt",
            "--password-stdin",
            "--in",
            "/tmp/cipher.enc",
            "--out",
            "/tmp/recovered.txt",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// L12 CRITICAL #2: --password-file path is secret-adjacent
    /// (it reveals where secrets live on disk). Redact in Debug
    /// output to avoid leaking in tracing/log capture.
    #[test]
    fn debug_redacts_password_file_path_in_encrypt() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password-file",
            "/run/secrets/btc-pwd",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("/run/secrets"),
            "password_file path leaked in Debug: {debug}"
        );
        assert!(
            debug.contains("redacted"),
            "redaction marker missing: {debug}"
        );
    }

    #[test]
    fn debug_redacts_password_file_path_in_decrypt() {
        let cli = Cli::try_parse_from([
            "btc",
            "decrypt",
            "--password-file",
            "/run/secrets/btc-pwd",
            "--in",
            "/tmp/cipher.enc",
            "--out",
            "/tmp/recovered.txt",
        ])
        .unwrap();
        let debug = format!("{cli:?}");
        assert!(
            !debug.contains("/run/secrets"),
            "password_file path leaked in Debug: {debug}"
        );
    }

    #[test]
    fn parse_decrypt_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "decrypt",
            "--password",
            "hunter2",
            "--in",
            "/tmp/cipher.enc",
            "--out",
            "/tmp/recovered.txt",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    #[test]
    fn parse_encrypt_rejects_missing_in() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password",
            "hunter2",
            "--out",
            "/tmp/cipher.enc",
        ]);
        assert!(cli.is_err(), "missing --in should fail to parse");
    }

    /// L12 CRITICAL #2: password MUST NOT appear in Debug output
    /// for encrypt/decrypt subcommands. Tracing / log capture is the
    /// most likely leak vector.
    #[test]
    fn debug_redacts_password_in_encrypt() {
        let cli = Cli::try_parse_from([
            "btc",
            "encrypt",
            "--password",
            "hunter2",
            "--in",
            "/tmp/plain.txt",
            "--out",
            "/tmp/cipher.enc",
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
    fn debug_redacts_password_in_decrypt() {
        let cli = Cli::try_parse_from([
            "btc",
            "decrypt",
            "--password",
            "hunter2",
            "--in",
            "/tmp/cipher.enc",
            "--out",
            "/tmp/recovered.txt",
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

    // ========== Story 8 / Issue #121: btc fee-estimates ==========

    /// Story 8: `btc fee-estimates` parses required args. Full coverage
    /// of the happy path is in handler tests (which would require a
    /// live Esplora — covered by L29 operator-driven gate).
    #[test]
    fn parse_fee_estimates_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "fee-estimates",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 8: `--json` flag parses cleanly.
    #[test]
    fn parse_fee_estimates_accepts_json_flag() {
        let cli = Cli::try_parse_from([
            "btc",
            "fee-estimates",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
            "--json",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 8: missing `--network` must fail to parse.
    #[test]
    fn parse_fee_estimates_rejects_missing_network() {
        let cli = Cli::try_parse_from([
            "btc",
            "fee-estimates",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_err(), "missing --network should fail to parse");
    }
}
