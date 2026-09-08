//! clap subcommand tree for the `tron` CLI — plan §Phase 6 Task 5.1.
//!
//! 22 commands across 6 top-level groups: `wallet` (9), `address` (2),
//! `balance` (1 command, 2 forms), `trc20` (4), `tx` (2), `config` (3).
//!
//! Every data-producing command carries `--json`. Diagnostics go to STDERR;
//! only requested data reaches STDOUT (plan §Task 5.8).

use std::path::PathBuf;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, Parser, Subcommand};

pub use tron_wallet_core::config::Network;

/// `tron` — TRON (TRX + TRC-20) wallet CLI.
#[derive(Debug, Parser)]
#[command(name = "tron", version, about = "TRON wallet CLI (TRX + TRC-20)")]
pub struct Cli {
    /// Wallet + config directory. Defaults to the platform data dir.
    #[arg(long, global = true, env = "TRON_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Default RPC URL for every subcommand. Overrides per-command
    /// `--rpc-url`. Plan §Task 7.15 spike invariant: the black-box CLI
    /// tests assert on the top-level `--rpc` shape.
    #[arg(long, global = true, value_name = "URL")]
    pub rpc: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level command groups.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Wallet lifecycle: create, import, inspect, send.
    Wallet(WalletCmd),
    /// Address derivation and extended-public-key export.
    Address(AddressCmd),
    /// One-shot balance lookup for an address.
    Balance(BalanceArgs),
    /// TRC-20 token operations.
    Trc20(Trc20Cmd),
    /// Transaction lookup and confirmation wait.
    Tx(TxCmd),
    /// CLI configuration.
    Config(ConfigCmd),
}

/// Amount unit for TRX-denominated values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum UnitArg {
    /// 1 TRX = 1_000_000 SUN.
    Trx,
    /// Base unit.
    Sun,
}

/// `--words 12|24`. Rejects anything else at parse time (exit 2) rather than
/// letting BIP-39 reject it after the passphrase prompt.
fn word_count_parser() -> impl TypedValueParser<Value = u8> {
    PossibleValuesParser::new(["12", "24"]).map(|s| s.parse::<u8>().expect("possible value is u8"))
}

// ---------------------------------------------------------------- wallet

/// `tron wallet <action>` — 9 subcommands.
#[derive(Debug, Args)]
pub struct WalletCmd {
    #[command(subcommand)]
    pub action: WalletAction,
}

#[derive(Debug, Subcommand)]
pub enum WalletAction {
    /// Generate a mnemonic and persist an encrypted wallet.
    Create {
        /// Mnemonic word count.
        #[arg(long, default_value = "12", value_parser = word_count_parser())]
        words: u8,
        /// Human-readable label (needs wallet metadata — not in v0.1 core).
        #[arg(long)]
        name: Option<String>,
        /// Network this wallet is intended for.
        #[arg(long, value_enum, default_value_t)]
        network: Network,
        /// Encryption passphrase. Prefer `TRON_PASSWORD` or the TTY prompt.
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        /// Emit the wallet id as JSON on STDOUT.
        #[arg(long)]
        json: bool,
    },
    /// Import an existing wallet from a mnemonic or private key.
    Import {
        /// BIP-39 phrase on argv (insecure — prefer `--mnemonic-file`).
        #[arg(long, conflicts_with_all = ["mnemonic_file", "private_key_file"])]
        mnemonic: Option<String>,
        /// File holding the BIP-39 phrase.
        #[arg(long, conflicts_with_all = ["mnemonic", "private_key_file"])]
        mnemonic_file: Option<PathBuf>,
        /// File holding a raw 32-byte hex private key.
        #[arg(long, conflicts_with_all = ["mnemonic", "mnemonic_file"])]
        private_key_file: Option<PathBuf>,
        /// Human-readable label (needs wallet metadata — not in v0.1 core).
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_enum, default_value_t)]
        network: Network,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Decrypt a stored wallet and print its account-0 address.
    Show {
        #[arg(long)]
        id: String,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        /// Derivation path override.
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List stored wallet ids, with names and networks when unlocked.
    List {
        #[arg(long)]
        json: bool,
        /// Show wallets from every network (default: filter by `--network`
        /// when one is given).
        #[arg(long)]
        all_networks: bool,
        /// Passphrase. Without it only ids can be listed — the label and
        /// network live inside the encrypted record.
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        #[arg(long, value_enum)]
        network: Option<Network>,
    },
    /// Delete a stored wallet blob.
    Delete {
        #[arg(long)]
        id: String,
        /// Skip the confirmation prompt.
        #[arg(long)]
        confirm_yes: bool,
    },
    /// Rename a stored wallet.
    Rename {
        #[arg(long)]
        id: String,
        #[arg(long)]
        to: String,
        /// Passphrase: the label is inside the ciphertext, so renaming means
        /// decrypt → edit → re-encrypt.
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
    },
    /// Balance of a stored wallet's account-0 address.
    Balance {
        #[arg(long)]
        wallet_id: Option<String>,
        #[arg(long, conflicts_with = "wallet_id")]
        address: Option<String>,
        /// `USDT` or a TRC-20 contract address.
        #[arg(long)]
        token: Option<String>,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Send TRX.
    Send {
        #[arg(long)]
        wallet_id: Option<String>,
        #[arg(long, conflicts_with_all = ["wallet_id", "mnemonic_file"])]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with_all = ["wallet_id", "mnemonic"])]
        mnemonic_file: Option<PathBuf>,
        #[arg(long)]
        to: Option<String>,
        /// Send to another stored wallet by id.
        #[arg(long, conflicts_with = "to")]
        to_wallet: Option<String>,
        #[arg(long)]
        amount: String,
        #[arg(long, value_enum, default_value_t = UnitArg::Trx)]
        unit: UnitArg,
        #[arg(long)]
        fee_limit: Option<i64>,
        /// Build and print, do not broadcast.
        #[arg(long)]
        dry_run: bool,
        /// Sign and print the envelope, do not broadcast.
        #[arg(long)]
        sign_only: bool,
        /// Block until the transaction confirms.
        #[arg(long)]
        wait: bool,
        /// With `--wait`: seconds to wait before giving up.
        #[arg(long, default_value_t = 90)]
        wait_timeout: u64,
        /// With `--wait`: seconds between confirmation polls.
        #[arg(long, default_value_t = 3)]
        wait_poll_interval: u64,
        /// Skip the mainnet confirmation prompt (for scripts).
        #[arg(long)]
        confirm_yes: bool,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Re-broadcast a stuck transaction with a higher fee limit.
    SendSpeedup {
        #[arg(long)]
        wallet_id: String,
        #[arg(long)]
        txid: String,
        #[arg(long)]
        fee_limit: i64,
        /// Skip the mainnet confirmation prompt (for scripts).
        #[arg(long)]
        confirm_yes: bool,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Derive a T-address from an SEC1-encoded secp256k1 public key.
    ///
    /// Plan §Task 7.15 spike invariant: black-box tests assert on the
    /// `wallet address --pubkey <hex>` round-trip. The hex form is the
    /// uncompressed `04 || X(32) || Y(32)` 65-byte encoding; the leading
    /// `0x` is optional.
    Address {
        /// SEC1-encoded uncompressed secp256k1 public key (65 bytes hex).
        #[arg(long, value_name = "HEX")]
        pubkey: String,
    },
}

// --------------------------------------------------------------- address

/// `tron address <action>` — 2 subcommands.
#[derive(Debug, Args)]
pub struct AddressCmd {
    #[command(subcommand)]
    pub action: AddressAction,
}

#[derive(Debug, Subcommand)]
pub enum AddressAction {
    /// Derive an address from a mnemonic at `--index` (or full `--path`).
    New {
        #[arg(long, conflicts_with = "mnemonic_file")]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with = "mnemonic")]
        mnemonic_file: Option<PathBuf>,
        /// BIP-44 address index; ignored when `--path` is given.
        #[arg(long, default_value_t = 0)]
        index: u32,
        /// Full derivation path override, e.g. `m/44'/195'/0'/0/3`.
        #[arg(long)]
        path: Option<String>,
        /// BIP-39 passphrase (not the the password).
        #[arg(long, default_value = "")]
        bip39_passphrase: String,
        #[arg(long)]
        json: bool,
    },
    /// Export the the extended public key for a stored wallet.
    Xpub {
        #[arg(long)]
        wallet_id: String,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        /// Derivation path for the xpub. Defaults to the account-0 path.
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

// --------------------------------------------------------------- balance

/// `tron balance --address <T-addr> [--token ...]`.
#[derive(Debug, Args)]
pub struct BalanceArgs {
    #[arg(long)]
    pub address: String,
    /// `USDT` or a TRC-20 contract address. Omit for the native TRX balance.
    #[arg(long)]
    pub token: Option<String>,
    #[arg(long, value_enum, default_value_t = UnitArg::Trx)]
    pub unit: UnitArg,
    #[arg(long, value_enum)]
    pub network: Option<Network>,
    #[arg(long)]
    pub rpc_url: Option<String>,
    #[arg(long)]
    pub json: bool,
}

// ----------------------------------------------------------------- trc20

/// `tron trc20 <action>` — 4 subcommands.
#[derive(Debug, Args)]
pub struct Trc20Cmd {
    #[command(subcommand)]
    pub action: Trc20Action,
}

#[derive(Debug, Subcommand)]
pub enum Trc20Action {
    /// Transfer TRC-20 tokens.
    Send {
        #[arg(long)]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with_all = ["mnemonic", "mnemonic_file"])]
        wallet_id: Option<String>,
        #[arg(long, conflicts_with_all = ["wallet_id", "mnemonic"])]
        mnemonic_file: Option<PathBuf>,
        /// `USDT` or a contract address.
        #[arg(long)]
        contract: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: String,
        #[arg(long)]
        fee_limit: Option<i64>,
        /// Build and print an energy estimate, do not broadcast.
        #[arg(long)]
        dry_run: bool,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        /// Skip the mainnet confirmation prompt (for scripts).
        #[arg(long)]
        confirm_yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Approve a spender allowance.
    Approve {
        #[arg(long)]
        mnemonic: Option<String>,
        #[arg(long, conflicts_with_all = ["mnemonic", "mnemonic_file"])]
        wallet_id: Option<String>,
        #[arg(long, conflicts_with_all = ["wallet_id", "mnemonic"])]
        mnemonic_file: Option<PathBuf>,
        #[arg(long)]
        contract: String,
        #[arg(long)]
        spender: String,
        /// Token amount, or `max` for an unlimited allowance.
        #[arg(long)]
        amount: String,
        #[arg(long)]
        fee_limit: Option<i64>,
        #[arg(long, env = "TRON_PASSWORD")]
        password: Option<String>,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        /// Skip the unlimited-approval and mainnet prompts (for scripts).
        #[arg(long)]
        confirm_yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Token balance of an address.
    Balance {
        #[arg(long)]
        address: String,
        #[arg(long)]
        contract: String,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Remaining allowance `--owner` granted `--spender`.
    Allowance {
        #[arg(long)]
        contract: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        spender: String,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read `decimals()` from a TRC-20 contract.
    ///
    /// Bundled-registry first, live `decimals()` fallback for unknown tokens.
    /// V5 §Task 7.5 asserts the on-chain value matches the registry (USDT = 6).
    Decimals {
        #[arg(long)]
        contract: String,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Encode a TRC-20 `transfer` / `approve` call and emit the hex calldata.
    ///
    /// Pure offline transform: no signing, no network, no `--mnemonic`.
    /// The 4-byte selector is concatenated onto the encoded argument block
    /// so callers receive the full 68-byte calldata (selector + 32-byte
    /// address slot + 32-byte uint256 slot).
    ///
    /// Plan §Task 7.15 spike invariant: black-box tests assert on the
    /// `trc20 encode-call {transfer,approve}` wire output.
    EncodeCall {
        /// `transfer` or `approve`.
        #[arg(value_enum)]
        kind: EncodeCallKind,
        /// Recipient (transfer) or spender (approve) — TRON T-address.
        #[arg(long)]
        to: String,
        /// Amount in the token's smallest unit (u256 decimal string).
        #[arg(long)]
        amount: String,
    },
}

/// `transfer` calldata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum EncodeCallKind {
    /// `transfer(address,uint256)` — selector `0xa9059cbb`.
    Transfer,
    /// `approve(address,uint256)` — selector `0x095ea7b3`.
    Approve,
}

// -------------------------------------------------------------------- tx

/// `tron tx <action>` — 2 subcommands.
#[derive(Debug, Args)]
pub struct TxCmd {
    #[command(subcommand)]
    pub action: TxAction,
}

#[derive(Debug, Subcommand)]
pub enum TxAction {
    /// Fetch on-chain info for a txid.
    Get {
        #[arg(long)]
        txid: String,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Poll until a txid confirms or the timeout elapses.
    Wait {
        #[arg(long)]
        txid: String,
        /// Seconds to wait before giving up.
        #[arg(long, default_value_t = 60)]
        timeout: u64,
        /// Seconds between polls.
        #[arg(long, default_value_t = 3)]
        poll_interval: u64,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Re-POST a previously signed envelope. Returns the broadcast receipt;
    /// a duplicate envelope surfaces as `DUP_TRANSACTION_ERROR` on the live
    /// network.
    ///
    /// Plan §Task 7.15 spike invariant: black-box tests assert on the
    /// `tx broadcast --hex <envelope>` path for rebroadcast-idempotency rows.
    Broadcast {
        /// Signed envelope hex (raw proto with signature appended).
        #[arg(long, conflicts_with = "file")]
        hex: Option<String>,
        /// JSON file holding `{txid, signed_envelope_hex}` (mutually
        /// exclusive with `--hex`).
        #[arg(long, conflicts_with = "hex")]
        file: Option<PathBuf>,
        #[arg(long, value_enum)]
        network: Option<Network>,
        #[arg(long)]
        rpc_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

// ---------------------------------------------------------------- config

/// `tron config <action>` — 3 subcommands.
#[derive(Debug, Args)]
pub struct ConfigCmd {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print the effective configuration.
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Persist a new RPC base URL.
    SetRpc {
        /// Base URL, no trailing slash.
        url: String,
    },
    /// Persist the active network (resets `rpc_url` to that network's default).
    SetNetwork {
        #[arg(value_enum)]
        network: Network,
    },
}
