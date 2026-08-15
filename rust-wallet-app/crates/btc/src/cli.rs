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
    /// List transactions touching the wallet's keychain (Issue #120
    /// / Story 7 / Task 27 `chain::explorer`). Stateless like
    /// `btc wallet sync`/`balance` — takes a mnemonic, syncs from
    /// Esplora, then prints deduped txids + block-explorer URLs.
    TxList(TxListAction),
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

/// `btc tx-list` args — Issue #120 / Story 7. Read-only Esplora
/// scan + tx graph enumeration; mirrors `btc wallet sync` arg
/// shape (mnemonic + network + esplora-url + pin-spki + limit + json).
///
/// CLI command name is `tx-list` (kebab-case) instead of nested
/// `tx list` to avoid clap-subcommand conflict with any future
/// `tx <sub>` siblings.
#[derive(clap::Args)]
#[command(name = "tx-list")]
pub struct TxListAction {
    /// BIP-39 mnemonic phrase (12/15/18/21/24 words).
    #[arg(long, env = "BTC_WALLET_MNEMONIC")]
    pub mnemonic: String,
    /// Bitcoin network.
    #[arg(long, value_enum)]
    pub network: NetArg,
    /// Esplora base URL (HTTPS-only per F36).
    #[arg(long)]
    pub esplora_url: String,
    /// Esplora SPKI pin (required for non-regtest per F20).
    #[arg(long, env = "BTC_ESPLORA_SPKI_PIN")]
    pub pin_spki: Option<String>,
    /// Cap the number of txs printed (default: all).
    #[arg(long)]
    pub limit: Option<u32>,
    /// Output as JSON instead of human-readable text.
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

impl fmt::Debug for TxListAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Story 7 / Issue #120 — mnemonic is secret material
        // (L12 CRITICAL #2 pattern). Redact it from Debug output.
        // The remaining fields (network + esplora_url + pin_spki
        // + limit + json) are not secret.
        f.debug_struct("TxListAction")
            .field("mnemonic", &"<redacted>")
            .field("network", &self.network)
            .field("esplora_url", &self.esplora_url)
            .field("pin_spki", &self.pin_spki)
            .field("limit", &self.limit)
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
        /// Address type (BIP-44/49/84/86) — Story 20 / Issue #132.
        /// `legacy` (P2PKH `m`/`n`), `nested-segwit` (BIP-49 P2SH-P2WPKH
        /// `2`), `native-segwit` (BIP-84 P2WPKH `tb1q...` — default),
        /// `taproot` (BIP-86 P2TR `tb1p...`). The chosen type drives
        /// the descriptor template at wallet creation; persisted blobs
        /// encode the type in the descriptor shape.
        #[arg(long = "type", value_enum, default_value_t = AddressTypeArg::NativeSegwit)]
        address_type: AddressTypeArg,
        /// Confirmation text for mainnet (Story 10). When `--network
        /// mainnet`, this flag must be passed with the exact text
        /// `yes` (lowercase). Any other value, or absence, aborts with
        /// exit code 1. Defends against accidental mainnet wallet
        /// creation (real BTC funds at risk).
        #[arg(long, value_name = "yes")]
        confirm_yes: Option<String>,
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
        /// Path to the bdk_file_store SQLite-like store (Story 12 /
        /// Issue #130 PR3-CLI). When set, `btc wallet show` writes
        /// the wallet's `ChangeSet` to this file after syncing. The
        /// next invocation with the same `--db-path` reloads the
        /// persisted state instead of re-syncing from Esplora.
        /// Default: not persisted (in-memory only).
        #[arg(long)]
        db_path: Option<std::path::PathBuf>,
    },
    /// List all wallets in `<data_dir>/wallets/<network>/`. One wallet
    /// ID per line; `--json` outputs a JSON array. Empty data dir
    /// prints `(no wallets)` and exits 0 (Story 9 AC).
    List {
        /// Bitcoin network to list wallets for.
        #[arg(long, value_enum)]
        network: NetArg,
        /// JSON output (array of `{id}` strings).
        #[arg(long)]
        json: bool,
    },
    /// Delete the wallet at `<data_dir>/wallets/<network>/<id>.enc`.
    /// Errors if the wallet does not exist (Story 9 AC).
    Delete {
        /// Wallet ID (UUID v4) printed by `wallet create`.
        id: String,
        /// Bitcoin network the wallet was created for.
        #[arg(long, value_enum)]
        network: NetArg,
    },
    /// Rename the wallet blob in-place. Errors if source is missing
    /// OR target already exists (Story 9 AC).
    Rename {
        /// Source wallet ID.
        #[arg(long)]
        id: String,
        /// Target wallet ID (must be a valid UUID v4).
        #[arg(long)]
        to: String,
        /// Bitcoin network the wallet was created for.
        #[arg(long, value_enum)]
        network: NetArg,
    },
    /// Bump fee on a previously-broadcast tx (RBF, Story 17 / Issue #140).
    /// Replaces the original tx with one that pays a strictly higher fee.
    /// BIP-125 sequence auto-bumped by bdk.
    BumpFee {
        /// BIP-39 mnemonic phrase (12/15/18/21/24 words).
        /// **SECURITY**: see `Sync::mnemonic` for shell-history caveats.
        #[arg(long, env = "BTC_WALLET_MNEMONIC")]
        mnemonic: String,
        /// Bitcoin network.
        #[arg(long, value_enum)]
        network: NetArg,
        /// Original txid to bump (64-char hex). Must be an
        /// unconfirmed tx in the wallet's tx graph.
        #[arg(long, value_name = "TXID", value_parser = parse_txid)]
        txid: bitcoin::Txid,
        /// New fee rate in sat/vB. MUST exceed the original tx's
        /// effective fee rate (RBF rule 3); exit 4 otherwise.
        #[arg(long = "fee-rate", alias = "fee-rate-sat-per-vb")]
        fee_rate_sat_per_vb: u64,
        /// Esplora base URL (HTTPS-only per F36).
        #[arg(long)]
        esplora_url: String,
        /// Esplora SPKI pin. See `Sync::pin_spki` for F20 enforcement.
        #[arg(long, env = "BTC_ESPLORA_SPKI_PIN")]
        pin_spki: Option<String>,
        /// Confirmation text for mainnet (Story 10 + Story 17).
        /// When `--network mainnet`, this flag must be passed with
        /// the exact text `yes` (lowercase).
        #[arg(long, value_name = "yes")]
        confirm_yes: Option<String>,
        /// Build + sign without broadcasting. Output is a base64 PSBT.
        #[arg(long)]
        dry_run: bool,
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
    /// Send satoshis to a recipient address (Story 5 / Issue #118,
    /// extended by Stories 13-14 / Issue #131).
    /// Full tx lifecycle: sync → build → sign → broadcast → return txid.
    /// Default fee rate is 1 sat/vB (Story 6 #119 adds --fee-rate override).
    ///
    /// Three send modes (mutually exclusive at parse layer):
    /// 1. Single-recipient: `--address <addr> --amount-sat <n>` (Story 5)
    /// 2. Multi-recipient: `--to <addr>:<amount>` (repeatable, 1-20 entries, Story 13)
    /// 3. Drain: `--drain-to <addr>` (Story 14)
    ///
    /// `--dry-run` builds + signs without broadcasting (Stories 13/14).
    /// `--exclude-utxo <txid:vout>` is repeatable (Story 14).
    ///
    /// `--confirm-yes yes` is required for mainnet (Story 10 precedent
    /// extended to send: defends against operator typo draining
    /// real BTC funds).
    ///
    /// **SECURITY**: mnemonic is the wallet's key material — see
    /// `Sync::mnemonic` for shell-history caveats. Recipient addresses
    /// are validated against `--network` at the handler layer
    /// (`require_network_address`).
    Send {
        /// BIP-39 mnemonic phrase (12/15/18/21/24 words).
        /// **SECURITY**: see `Sync::mnemonic` for shell-history caveats.
        #[arg(long, env = "BTC_WALLET_MNEMONIC")]
        mnemonic: String,
        /// Bitcoin network.
        #[arg(long, value_enum)]
        network: NetArg,
        /// Single-recipient address (Story 5 mode).
        #[arg(long, conflicts_with_all = ["to", "drain_to"])]
        address: Option<String>,
        /// Single-recipient amount in satoshis (Story 5 mode).
        #[arg(long, conflicts_with_all = ["to", "drain_to"])]
        amount_sat: Option<u64>,
        /// Multi-recipient list (Story 13): one or more `addr:amount`
        /// pairs. Repeatable; up to MAX_RECIPIENTS (20) entries
        /// (BDK recommended safe max, enforced at handler layer
        /// — clap `num_args` on `Vec<T>` is not honored in this
        /// clap 4 derive version). Excludes `--address`,
        /// `--amount-sat`, and `--drain-to`.
        #[arg(
            long = "to",
            value_name = "addr:amount",
            value_parser = parse_to_arg,
            conflicts_with_all = ["address", "amount_sat", "drain_to"],
        )]
        to: Vec<ToArg>,
        /// Drain all spendable UTXOs to this single address
        /// (Story 14). Excludes `--address`, `--amount-sat`, and
        /// `--to`. UTXOs listed in `--exclude-utxo` are skipped.
        #[arg(
            long = "drain-to",
            value_name = "addr",
            value_parser = parse_address_arg,
            conflicts_with_all = ["address", "amount_sat", "to"],
        )]
        drain_to: Option<bitcoin::Address<bitcoin::address::NetworkUnchecked>>,
        /// Exclude a UTXO from coin selection (Story 14). Repeatable.
        /// Format: `<txid>:<vout>` (64-char hex txid).
        #[arg(long = "exclude-utxo", value_name = "txid:vout", value_parser = parse_outpoint_arg)]
        exclude_utxo: Vec<bitcoin::OutPoint>,
        /// Build + sign without broadcasting (Stories 13/14).
        /// Output is a base64-encoded PSBT instead of a txid.
        #[arg(long)]
        dry_run: bool,
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
        /// Coin-selection algorithm (Story 15 / Issue #139).
        /// `bnb` (default — BranchAndBound with SingleRandomDraw),
        /// `knapsack` (largest-first greedy, BDK's
        /// `LargestFirstCoinSelection`), or `oldest`
        /// (`OldestFirstCoinSelection`). Single-recipient mode
        /// only; `--drain-to` and `--to` are mutually exclusive at
        /// parse layer (per L12 H1 fix — silently dropping the flag
        /// in those modes was a documented-but-false contract).
        #[arg(
            long,
            value_enum,
            default_value_t = CoinSelection::Bnb,
            conflicts_with_all = ["drain_to", "to"],
        )]
        coin_selection: CoinSelection,
        /// Manual UTXO selection (Story 16 / Issue #139). One or
        /// more `<txid>:<vout>` outpoints the operator wants to
        /// fund the send with. Repeatable. Outpoints not tracked by
        /// the wallet fail at build time (bdk `add_utxo` error,
        /// sanitized to "add_utxo failed: UnknownUtxo").
        /// Single-recipient mode only; `--drain-to` and `--to`
        /// are mutually exclusive (L12 H1 + H3 fix).
        #[arg(
            long = "input",
            value_name = "txid:vout",
            value_parser = parse_input_arg,
            conflicts_with_all = ["drain_to", "to"],
        )]
        input: Vec<bitcoin::OutPoint>,
        /// When `--input` is set, restrict coin selection to ONLY
        /// those outpoints (bdk `manually_selected_only`). The tx
        /// fails if selected UTXOs don't cover amount + fee (no
        /// auto-append). Story 16 strict mode. Requires `--input`
        /// (L12 M2 fix — silently no-op'ing without input is the
        /// wrong default for a spend command).
        #[arg(long, requires = "input", conflicts_with_all = ["drain_to", "to"])]
        manual_selection_only: bool,
        /// Confirmation text for mainnet (Story 10 + Stories 13/14).
        /// When `--network mainnet`, this flag must be passed with
        /// the exact text `yes` (lowercase). Defends against
        /// accidental mainnet spend (real BTC funds at risk).
        #[arg(long, value_name = "yes")]
        confirm_yes: Option<String>,
    },
}

/// User-facing coin-selection algorithm menu (Story 15 / Issue #139).
///
/// Mirrors the lib-layer enum (kept separate to keep the lib free
/// of clap — adding clap as a lib dep would be heavy). The lib
/// has the real semantics (maps to bdk 3.1 `CoinSelectionAlgorithm`
/// impls). The 1:1 identity `From` impl below bridges them.
///
/// Maps to bdk 3.1's `CoinSelectionAlgorithm` impls:
/// - `Bnb` → `BranchAndBoundCoinSelection<SingleRandomDraw>` (BDK default)
/// - `Knapsack` → `LargestFirstCoinSelection` (greedy largest-first; bdk
///   3.1 has no standalone `Knapsack` impl)
/// - `Oldest` → `OldestFirstCoinSelection` (picks oldest-block UTXOs first)
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum CoinSelection {
    Bnb,
    Knapsack,
    Oldest,
}

impl From<CoinSelection> for bitcoin_wallet_core::tx::builder::CoinSelection {
    fn from(c: CoinSelection) -> Self {
        match c {
            CoinSelection::Bnb => Self::Bnb,
            CoinSelection::Knapsack => Self::Knapsack,
            CoinSelection::Oldest => Self::Oldest,
        }
    }
}

/// BDK recommended safe max recipients per multi-recipient tx
/// (Story 13 / Issue #138). Re-exported from
/// `bitcoin_wallet_core::tx::builder::MAX_RECIPIENTS` so the clap
/// `num_args` bound can reference it without a cross-crate const
/// import. Kept in sync manually (verified by compile).
pub const MAX_RECIPIENTS: usize = 20;

/// Multi-recipient entry (`--to addr:amount`). Story 13.
///
/// **Network unchecked by design** — `address` is
/// `Address<NetworkUnchecked>`. The handler layer's
/// `require_network_address` is the single authoritative gate
/// (matches `--network`); the parser is intentionally permissive
/// so any network's address parses through and only the handler
/// rejects wrong-network combinations.
#[derive(Clone, Debug)]
pub struct ToArg {
    pub address: bitcoin::Address<bitcoin::address::NetworkUnchecked>,
    pub amount_sat: u64,
}

/// Parse `--to addr:amount`. Address is stored unchecked;
/// handler does `require_network(--network)`. Amount must be u64.
fn parse_to_arg(s: &str) -> Result<ToArg, String> {
    let (addr_str, amount_str) = s
        .rsplit_once(':')
        .ok_or_else(|| format!("--to expects `addr:amount` format, got `{s}`"))?;
    let address = addr_str
        .parse::<bitcoin::Address<bitcoin::address::NetworkUnchecked>>()
        .map_err(|e| format!("--to address parse: {e}"))?;
    let amount_sat = amount_str
        .parse::<u64>()
        .map_err(|e| format!("--to amount parse: {e}"))?;
    Ok(ToArg {
        address,
        amount_sat,
    })
}

/// Parse `--drain-to <addr>` (no amount). Address is unchecked;
/// handler does the network check.
fn parse_address_arg(
    s: &str,
) -> Result<bitcoin::Address<bitcoin::address::NetworkUnchecked>, String> {
    s.parse::<bitcoin::Address<bitcoin::address::NetworkUnchecked>>()
        .map_err(|e| format!("--drain-to address parse: {e}"))
}

/// Parse `--exclude-utxo <txid>:<vout>`. txid is 64-char hex; vout is u32.
fn parse_outpoint_arg(s: &str) -> Result<bitcoin::OutPoint, String> {
    parse_outpoint_for("--exclude-utxo", s)
}

/// Parse `--input <txid>:<vout>` (Story 16). Same format as
/// `--exclude-utxo`; parameterized flag name for accurate errors
/// (L12 M4 fix).
fn parse_input_arg(s: &str) -> Result<bitcoin::OutPoint, String> {
    parse_outpoint_for("--input", s)
}

fn parse_outpoint_for(flag: &str, s: &str) -> Result<bitcoin::OutPoint, String> {
    let (txid_str, vout_str) = s
        .split_once(':')
        .ok_or_else(|| format!("{flag} expects `<txid>:<vout>` format, got `{s}`"))?;
    let txid = txid_str
        .parse::<bitcoin::Txid>()
        .map_err(|e| format!("{flag} txid parse: {e}"))?;
    let vout = vout_str
        .parse::<u32>()
        .map_err(|e| format!("{flag} vout must be u32, got `{vout_str}` ({e})"))?;
    Ok(bitcoin::OutPoint::new(txid, vout))
}

/// Parse `--txid <64-char-hex>` (Story 17 bump-fee).
fn parse_txid(s: &str) -> Result<bitcoin::Txid, String> {
    s.parse::<bitcoin::Txid>()
        .map_err(|e| format!("--txid parse: {e}"))
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

/// CLI-side address-type enum (Story 20 / Issue #132). Mirrors the
/// lib's `AddressType` (clap `ValueEnum` lives in the binary crate
/// — same separation pattern as `CoinSelection`).
#[derive(Copy, Clone, ValueEnum, Debug, PartialEq, Eq)]
pub enum AddressTypeArg {
    /// BIP-44 P2PKH (`m`/`n` on testnet).
    Legacy,
    /// BIP-49 P2SH-P2WPKH (`2` on testnet).
    NestedSegwit,
    /// BIP-84 P2WPKH (`tb1q...`) — default.
    NativeSegwit,
    /// BIP-86 P2TR (`tb1p...`).
    Taproot,
}

impl From<AddressTypeArg> for bitcoin_wallet_core::keys::AddressType {
    fn from(a: AddressTypeArg) -> Self {
        match a {
            AddressTypeArg::Legacy => Self::Legacy,
            AddressTypeArg::NestedSegwit => Self::NestedSegwit,
            AddressTypeArg::NativeSegwit => Self::NativeSegwit,
            AddressTypeArg::Taproot => Self::Taproot,
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
            Self::TxList(tl) => f.debug_tuple("TxList").field(tl).finish(),
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
                address_type,
                confirm_yes,
            } => f
                .debug_struct("Create")
                .field("words", words)
                .field("network", network)
                .field("password", &"<redacted>")
                .field("address_type", address_type)
                .field(
                    "confirm_yes",
                    &confirm_yes
                        .as_ref()
                        .map(|_| "<provided>")
                        .unwrap_or("<absent>"),
                )
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
                db_path,
            } => f
                .debug_struct("Show")
                .field("id", id)
                .field("network", network)
                .field("password", &"<redacted>")
                .field("esplora_url", esplora_url)
                .field("esplora_spki_pin", esplora_spki_pin)
                .field("db_path", db_path)
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
            // Story 5 / Issue #118 (extended Stories 13-14): same
            // redaction policy as Sync/Balance (mnemonic is secret
            // material — never appear in Debug). Recipient addresses
            // and amounts are not secret; --exclude-utxo outpoints
            // are public chain data; drain_to address is public chain
            // data; confirm_yes presence is surfaced (value not — to
            // avoid leaking the confirmation token in traces).
            Self::Send {
                mnemonic: _,
                network,
                address,
                amount_sat,
                to,
                drain_to,
                exclude_utxo,
                dry_run,
                esplora_url,
                pin_spki,
                fee_rate_sat_per_vb,
                coin_selection,
                input,
                manual_selection_only,
                confirm_yes,
            } => f
                .debug_struct("Send")
                .field("mnemonic", &"<redacted>")
                .field("network", network)
                .field("address", address)
                .field("amount_sat", amount_sat)
                .field("to", &to.len())
                .field(
                    "drain_to",
                    &drain_to
                        .as_ref()
                        .map(|_| "<provided>")
                        .unwrap_or("<absent>"),
                )
                .field("exclude_utxo", &exclude_utxo.len())
                .field("dry_run", dry_run)
                .field("esplora_url", esplora_url)
                .field("pin_spki", pin_spki)
                .field("fee_rate_sat_per_vb", fee_rate_sat_per_vb)
                .field("coin_selection", coin_selection)
                .field("input", &input.len())
                .field("manual_selection_only", manual_selection_only)
                .field(
                    "confirm_yes",
                    &confirm_yes
                        .as_ref()
                        .map(|_| "<provided>")
                        .unwrap_or("<absent>"),
                )
                .finish(),
            Self::List { network, json } => f
                .debug_struct("List")
                .field("network", network)
                .field("json", json)
                .finish(),
            Self::Delete { id, network } => f
                .debug_struct("Delete")
                .field("id", id)
                .field("network", network)
                .finish(),
            Self::Rename { id, to, network } => f
                .debug_struct("Rename")
                .field("id", id)
                .field("to", to)
                .field("network", network)
                .finish(),
            // Story 17 / Issue #140: RBF bump-fee. Mnemonic redacted;
            // txid is public chain data (printed in Debug).
            Self::BumpFee {
                mnemonic: _,
                network,
                txid,
                fee_rate_sat_per_vb,
                esplora_url,
                pin_spki,
                confirm_yes,
                dry_run,
            } => f
                .debug_struct("BumpFee")
                .field("mnemonic", &"<redacted>")
                .field("network", network)
                .field("txid", txid)
                .field("fee_rate_sat_per_vb", fee_rate_sat_per_vb)
                .field("esplora_url", esplora_url)
                .field("pin_spki", pin_spki)
                .field("dry_run", dry_run)
                .field(
                    "confirm_yes",
                    &confirm_yes
                        .as_ref()
                        .map(|_| "<provided>")
                        .unwrap_or("<absent>"),
                )
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

    /// Story 20 / Issue #132: `--type native-segwit` is the default
    /// (no `--type` flag required for the historical default).
    #[test]
    fn parse_create_defaults_address_type_to_native_segwit() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "testnet",
        ])
        .unwrap();
        let Commands::Wallet(WalletAction {
            action: WalletActionKind::Create { address_type, .. },
        }) = cli.command
        else {
            panic!("expected Create subcommand");
        };
        assert_eq!(address_type, AddressTypeArg::NativeSegwit);
    }

    /// Story 20: `--type legacy` parses.
    #[test]
    fn parse_create_accepts_legacy_address_type() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "testnet",
            "--type",
            "legacy",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 20: `--type taproot` parses.
    #[test]
    fn parse_create_accepts_taproot_address_type() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "testnet",
            "--type",
            "taproot",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 20: invalid address type value exits at parse.
    #[test]
    fn parse_create_rejects_invalid_address_type() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "testnet",
            "--type",
            "p2pkh",
        ]);
        assert!(
            cli.is_err(),
            "invalid --type value must fail at parse (exit 2)"
        );
    }

    /// Story 10: Bitcoin network (mainnet) requires `--confirm-yes yes`.
    #[test]
    fn parse_create_bitcoin_network_accepts_confirm_yes() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "bitcoin",
            "--confirm-yes",
            "yes",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 10: testnet does NOT require confirm (no real BTC at risk).
    #[test]
    fn parse_create_testnet_accepts_without_confirm() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "create",
            "--words",
            "12",
            "--network",
            "testnet",
        ]);
        assert!(cli.is_ok(), "testnet must parse without confirm_yes");
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

    /// Story 5 / Issue #118 (updated Stories 13-14): missing `--address`
    /// no longer fails at parse layer — args are now optional (the
    /// mode is resolved by which flag is set: `--address` + `--amount-sat`
    /// for single, `--to` for multi, `--drain` for drain). Handler
    /// layer rejects when no mode is selected. Parse accepts the bare
    /// mnemonic + network + esplora-url.
    #[test]
    fn parse_send_accepts_without_address_or_to() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_ok(),
            "missing --address/--to must parse (handler rejects); got: {cli:?}"
        );
    }

    /// Story 5 / Issue #118 (updated): same as above for `--amount-sat`.
    /// Handler layer validates the single-recipient pair together.
    #[test]
    fn parse_send_accepts_without_amount_sat() {
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
        assert!(
            cli.is_ok(),
            "missing --amount-sat must parse (handler rejects); got: {cli:?}"
        );
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

    // ========== Story 7 / Issue #120: btc tx list ==========

    /// Story 7: `btc tx list` parses required args (mnemonic,
    /// network, esplora-url). Full happy-path coverage is
    /// operator-driven per L29 (live testnet).
    #[test]
    fn parse_tx_list_accepts_required_args() {
        let cli = Cli::try_parse_from([
            "btc",
            "tx-list",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 7: optional `--limit N` and `--json` flags parse cleanly.
    #[test]
    fn parse_tx_list_accepts_limit_and_json_flags() {
        let cli = Cli::try_parse_from([
            "btc",
            "tx-list",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
            "--limit",
            "10",
            "--json",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 7 (L12 CRITICAL #2): mnemonic MUST NOT appear in
    /// Debug output for `tx list`. Tracing / log capture is the
    /// most likely leak vector.
    #[test]
    fn debug_redacts_mnemonic_in_tx_list() {
        let cli = Cli::try_parse_from([
            "btc",
            "tx-list",
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

    // ========== Story 13-14 / Issue #131 / #138: btc wallet send extensions ==========

    /// Story 13 / Issue #138: `--to addr:amount` parses (repeatable,
    /// single entry).
    #[test]
    fn parse_send_accepts_to_repeatable() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--to",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx:10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 14 / Issue #138: `--drain-to <addr>` parses (drain
    /// mode, no amount — separate flag from `--to`).
    #[test]
    fn parse_send_accepts_drain_to_flag() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--drain-to",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 14 / Issue #138: `--drain-to` + `--to` are mutually
    /// exclusive (different intent; parser guards).
    #[test]
    fn parse_send_rejects_drain_to_and_to_together() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--drain-to",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--to",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx:1",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_err(), "--drain-to + --to must be mutually exclusive");
    }

    /// Story 10 + 13/14: `--network mainnet --confirm-yes yes` parses
    /// for non-dry-run sends. Handler enforces the gate.
    #[test]
    fn parse_send_accepts_confirm_yes_for_mainnet() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "bitcoin",
            "--address",
            "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--amount-sat",
            "10000",
            "--confirm-yes",
            "yes",
            "--esplora-url",
            "https://blockstream.info/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 13 / Issue #138: `--to` accepts up to MAX_RECIPIENTS
    /// (20) entries.
    #[test]
    fn parse_send_accepts_to_with_max_recipients() {
        let mut args: Vec<String> = vec![
            "btc".into(),
            "wallet".into(),
            "send".into(),
            "--mnemonic".into(),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".into(),
            "--network".into(),
            "testnet".into(),
            "--esplora-url".into(),
            "https://blockstream.info/testnet/api".into(),
        ];
        for i in 0..20 {
            args.push("--to".into());
            args.push(format!(
                "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx:{}",
                1000 + i
            ));
        }
        let cli = Cli::try_parse_from(args);
        assert!(
            cli.is_ok(),
            "20 --to entries must parse (MAX_RECIPIENTS); got: {cli:?}"
        );
    }

    /// Story 13 / Issue #138: `--to` with 21 entries (over
    /// MAX_RECIPIENTS) parses through clap but the handler layer
    /// enforces the cap (matches the lib `build_multi_recipient_tx`
    /// check — clap's `num_args` on `Vec<T>` isn't honored in this
    /// clap 4 derive version). The lib-layer test
    /// `build_multi_recipient_tx_rejects_over_20_recipients`
    /// pins the actual cap.
    #[test]
    fn parse_send_accepts_to_with_over_max_recipients_handler_enforces() {
        let mut args: Vec<String> = vec![
            "btc".into(),
            "wallet".into(),
            "send".into(),
            "--mnemonic".into(),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".into(),
            "--network".into(),
            "testnet".into(),
            "--esplora-url".into(),
            "https://blockstream.info/testnet/api".into(),
        ];
        for i in 0..21 {
            args.push("--to".into());
            args.push(format!(
                "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx:{}",
                1000 + i
            ));
        }
        let cli = Cli::try_parse_from(args);
        assert!(
            cli.is_ok(),
            "21 --to entries parse (handler enforces MAX_RECIPIENTS cap); got: {cli:?}"
        );
    }

    /// Story 14 / Issue #138: `--exclude-utxo` with invalid txid
    /// (non-hex chars) fails at parse layer.
    #[test]
    fn parse_send_rejects_invalid_exclude_utxo() {
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
            "--exclude-utxo",
            "not-a-valid-txid:0",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_err(),
            "invalid txid in --exclude-utxo must fail at parse"
        );
    }

    /// Story 13 / Issue #138 (L13 H1 fix): `--to` accepts a mainnet
    /// address even when `--network testnet` is set (parser is
    /// permissive; handler does cross-network rejection).
    #[test]
    fn parse_send_accepts_mainnet_to_with_testnet_network() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--to",
            "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh:10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_ok(),
            "mainnet address in --to must parse (handler rejects); got: {cli:?}"
        );
    }

    /// Story 14 / Issue #138 (L13 H1 fix): `--drain-to` accepts a
    /// mainnet address even when `--network testnet` is set.
    #[test]
    fn parse_send_accepts_mainnet_drain_to_with_testnet_network() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--drain-to",
            "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_ok(),
            "mainnet address in --drain-to must parse (handler rejects); got: {cli:?}"
        );
    }

    /// Story 15 / Issue #139: `--coin-selection bnb` is the default.
    #[test]
    fn parse_send_default_coin_selection_is_bnb() {
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
        let Commands::Wallet(WalletAction {
            action: WalletActionKind::Send { coin_selection, .. },
        }) = cli.command
        else {
            panic!("expected Send subcommand");
        };
        assert_eq!(coin_selection, CoinSelection::Bnb);
    }

    /// Story 15 / Issue #139: `--coin-selection knapsack` parses.
    #[test]
    fn parse_send_accepts_coin_selection_knapsack() {
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
            "--coin-selection",
            "knapsack",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 15 / Issue #139: invalid coin-selection value fails at parse.
    #[test]
    fn parse_send_rejects_invalid_coin_selection() {
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
            "--coin-selection",
            "magic",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_err(),
            "invalid --coin-selection value must fail to parse (exit 2 per Story 15 AC)"
        );
    }

    /// Story 16 / Issue #139: `--input <txid:vout>` is repeatable.
    #[test]
    fn parse_send_accepts_input_repeatable() {
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
            "--input",
            "0000000000000000000000000000000000000000000000000000000000000001:0",
            "--input",
            "0000000000000000000000000000000000000000000000000000000000000002:1",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 16 / Issue #139: `--manual-selection-only` parses.
    #[test]
    fn parse_send_accepts_manual_selection_only() {
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
            "--input",
            "0000000000000000000000000000000000000000000000000000000000000001:0",
            "--manual-selection-only",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 15+16 / Issue #139: `--coin-selection` + `--drain-to`
    /// are mutually exclusive.
    #[test]
    fn parse_send_rejects_coin_selection_and_drain_to_together() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--drain-to",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--coin-selection",
            "bnb",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_err(),
            "--coin-selection + --drain-to must be mutually exclusive"
        );
    }

    /// Story 16 / Issue #139: `--input` + `--drain-to` are
    /// mutually exclusive (drain picks all UTXOs).
    #[test]
    fn parse_send_rejects_input_and_drain_to_together() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--drain-to",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--input",
            "0000000000000000000000000000000000000000000000000000000000000001:0",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_err(),
            "--input + --drain-to must be mutually exclusive"
        );
    }

    /// Story 14 / Issue #138: `--exclude-utxo <txid:vout>` parses
    /// (repeatable).
    #[test]
    fn parse_send_accepts_exclude_utxo() {
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
            "--exclude-utxo",
            "0000000000000000000000000000000000000000000000000000000000000001:0",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 13 / Issue #138: `--dry-run` parses.
    #[test]
    fn parse_send_accepts_dry_run_flag() {
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
            "--dry-run",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_ok(), "expected parse ok, got: {cli:?}");
    }

    /// Story 13 / Issue #138: `--to` and `--address` are mutually
    /// exclusive (parse-layer guard).
    #[test]
    fn parse_send_rejects_to_and_address_together() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--to",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx:10000",
            "--address",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(cli.is_err(), "--to + --address must be mutually exclusive");
    }

    /// Story 14 / Issue #138: `--drain` conflicts with `--address` /
    /// `--amount-sat`.
    #[test]
    fn parse_send_rejects_drain_and_address_together() {
        let cli = Cli::try_parse_from([
            "btc",
            "wallet",
            "send",
            "--mnemonic",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "--network",
            "testnet",
            "--drain",
            "--address",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "--amount-sat",
            "10000",
            "--esplora-url",
            "https://blockstream.info/testnet/api",
        ]);
        assert!(
            cli.is_err(),
            "--drain + --address must be mutually exclusive"
        );
    }
}
