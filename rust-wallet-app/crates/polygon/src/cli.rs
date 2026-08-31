//! `polygon` CLI argument types — Issue #426 / T6b sub-task (L25 split).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §3.4 (clap subcommand tree) + §5.2 (clap derive types).
//!
//! T6b = scaffold only. Handler bodies land in T6c/T6d (per L25 sub-task
//! split). Cross-cutting flags wired here; per-handler stubs in `main.rs::run()`
//! dispatch to `Error::Rpc("deferred past T6b — landing in T6c/T6d")`.
//!
//! Mirror of `rust-wallet-app/crates/eth/src/main.rs:48-372` (single-file
//! clap derive pattern) + `btc/src/cli.rs:13-189` (per-action struct
//! split). The 7-file handler split design (per design doc §3.3) is deferred
//! to T6c when per-handler impls land.

use alloy_primitives::Address;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use zeroize::Zeroizing;

/// clap value_parser for Address-typed flags (mirrors eth/src/main.rs:44-46).
fn parse_address(s: &str) -> Result<Address, String> {
    s.parse::<Address>()
        .map_err(|e| format!("invalid address: {e}"))
}

/// Secret-string newtype for `--mnemonic` (and future `--private-key`).
///
/// Wraps `Zeroizing<String>` so the heap copy zeroizes on drop,
/// matching the `eth-wallet-core::unlock -> Zeroizing<Mnemonic>`
/// precedent (`evm-wallet-core/src/wallet.rs:534`) and the
/// Bitcoin-sibling pattern at `bitcoin-wallet-core/src/keys/secret.rs:16`
/// (`Secret<T>(T)` with private field + explicit accessor).
///
/// Design choices (per L12 cluster findings on #446):
/// - **Private inner field** — prevents `format!("{:?}", &mnemonic.0)`
///   reach-around that would print the raw phrase (Zeroizing<String>
///   inherits String's Debug).
/// - **No `Clone`** — sister `Secret<T>` precedent has no Clone;
///   duplication defeats zeroize-on-drop.
/// - **Custom `Debug`** — unconditional redaction; no panic surface.
/// - **Infallible `FromStr`** — BIP-39 validation deferred to the lib's
///   `import_wallet_for_network(name, &str, ...)` which surfaces
///   `Error::InvalidInput` for bad wordlists / counts. Loading the
///   2048-word BIP-39 English wordlist at clap-parse time is rejected
///   as non-paying-cost.
///
/// Residual (documented, NOT defended here):
/// - The clap-owned `&str` borrow passed to `FromStr` lives until `Cli::drop`
///   at end of `main` and is not zeroized.
/// - `--mnemonic <PHRASE>` on argv is visible to other processes on the host
///   (see security-audit H-1); L54-style warning / argv deprecation is a
///   separate scope.
///
/// `#446` follow-up. Out of scope: `--private-key` flag hardening → #447.
///
/// `Clone` is required by clap's `Subcommand` derive (`TypedValueParser`
/// bound). The cloned copy is itself `Zeroizing<String>` and zeroes on
/// drop, so the footgun surface (L12 review flagged the derive as a
/// leak vector) is bounded — each Clone carries an independent
/// zeroize-on-drop contract. In practice no caller clones the parsed
/// value (single-pass: parse → dispatch → drop), so the derive is
/// present-but-unused at the call-graph level.
#[derive(Clone)]
pub struct SecretMnemonic(Zeroizing<String>);

// Compile-time witness: the inner type zeroizes on drop. Sister precedent:
// `bitcoin-wallet-core/src/keys/mnemonic.rs:313-315`.
#[allow(dead_code)]
fn _assert_inner_zeroizes_on_drop() {
    fn _accepts<T: zeroize::ZeroizeOnDrop>() {}
    _accepts::<Zeroizing<String>>();
}

impl SecretMnemonic {
    /// Construct from an owned `String`. The caller transfers the heap
    /// buffer ownership; the original `String` is dropped (no zeroize)
    /// — `Zeroizing::new` only zeroes the new buffer on drop.
    pub fn new(s: String) -> Self {
        Self(Zeroizing::new(s))
    }

    /// Borrowed accessor returning `&Zeroizing<String>`. Bounded lifetime:
    /// the returned reference lives only as long as `&self`.
    pub fn expose(&self) -> &Zeroizing<String> {
        &self.0
    }
}

impl std::fmt::Debug for SecretMnemonic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretMnemonic([redacted])")
    }
}

impl std::str::FromStr for SecretMnemonic {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SecretMnemonic::new(s.to_string()))
    }
}

/// Top-level CLI. `name = "polygon"`, version from Cargo.toml, about string.
#[derive(Parser, Debug)]
#[command(
    name = "polygon",
    version,
    about = "Polygon PoS wallet CLI (alloy v1.8.x)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Default RPC URL override (env: `POLYGON_RPC_URL`).
    #[arg(long, global = true, env = "POLYGON_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Wallet-store base directory override (env: `POLYGON_DATA_DIR`).
    #[arg(long, global = true, env = "POLYGON_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
}

/// Top-level command enum — 9 variants per design §3.4.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Wallet commands (create/import/list/show/delete/balance/sync/send/speed-up).
    Wallet {
        #[command(subcommand)]
        action: WalletAction,
    },
    /// Transaction queries (list/get).
    Tx {
        #[command(subcommand)]
        action: TxAction,
    },
    /// ERC-20 token commands (send/balance/list/register/approve).
    Erc20 {
        #[command(subcommand)]
        action: Erc20Action,
    },
    /// Query current fee parameters (gas price, base fee).
    Fee(FeeArgs),
    /// Display resolved CLI configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Print Amoy faucet URL + drip-to instructions (Story 30).
    Faucet(FaucetArgs),
    /// Sign an EIP-191 personal message (Story 18).
    SignMessage(SignMessageArgs),
    /// Sign EIP-712 typed data with chain_id validation (Story 27, Q7 gate).
    SignTyped(SignTypedArgs),
    /// Print version + exit.
    Version,
}

/// Wallet subcommand actions (9 variants per design §3.4).
#[derive(Subcommand, Debug)]
pub enum WalletAction {
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long, default_value = "m/44'/60'/0'/0/0")]
        derivation_path: String,
        #[arg(long, default_value_t = 0)]
        account_index: u32,
        #[arg(long)]
        legacy_token_symbol: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    Import {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(
            long,
            conflicts_with = "private_key",
            conflicts_with = "private_key_file"
        )]
        mnemonic: Option<SecretMnemonic>,
        /// Hex private key (0x-prefixed or bare). Visible to sibling
        /// processes via `/proc/<pid>/cmdline` — prefer `--private-key-file`
        /// for operator-supplied secrets. Wired in #469; was a dead
        /// field per the T6c4 follow-up ("`--private-key` import path
        /// is deferred"). Conflict-class: same as `--private-key-file`
        /// per L12 H-1 sister finding from PR #456.
        #[arg(long, conflicts_with = "mnemonic", conflicts_with = "private_key_file")]
        private_key: Option<String>,
        /// Mode-0600 file path containing the raw PK bytes (no `0x`
        /// prefix). Closes the L12 H-1 argv-exposure hole for PK import
        /// (sister class to the `--mnemonic` argv finding closed by
        /// PR #456). File contents read into `Zeroizing<Vec<u8>>` and
        /// zeroized on drop; path is not zeroized. Per #469.
        #[arg(long, conflicts_with = "mnemonic", conflicts_with = "private_key")]
        private_key_file: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        account_index: u32,
        #[arg(long)]
        legacy_token_symbol: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    List {
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    Show {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        addresses: bool,
        #[arg(long)]
        export: bool,
        #[arg(long)]
        json: bool,
    },
    Delete {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
    },
    Balance {
        #[arg(long, value_parser = parse_address)]
        address: Address,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long, default_value = "pol")]
        unit: String,
        #[arg(long)]
        legacy_token_symbol: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    Sync {
        #[arg(long, value_parser = parse_address)]
        address: Address,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        rpc_url: Option<String>,
        /// Output Transfer events as JSON (`Vec<TxSummary>`).
        /// JSON fields: `block_number` (u64), `tx_hash` (0x-hex B256),
        /// `from` (0x-hex Address), `to` (0x-hex Address), `value`
        /// (decimal U256). Default = human-readable text (one line per
        /// Transfer).
        #[arg(long)]
        json: bool,
    },
    Send(SendArgs),
    /// RBF / speed-up replace-by-fee (Story 17).
    SendSpeedup(SendSpeedupArgs),
}

/// `wallet send` args (13 fields per design §3.4).
#[derive(clap::Args, Debug)]
pub struct SendArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub to: Address,
    #[arg(long)]
    pub amount: String,
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long, default_value = "pol")]
    pub unit: String,
    #[arg(long)]
    pub batch: Option<String>,
    #[arg(long)]
    pub drain: bool,
    #[arg(long)]
    pub nonce: Option<u64>,
    #[arg(long)]
    pub gas_limit: Option<u64>,
    /// Fee tier: fastest | half_hour | hour | economy (default half_hour).
    #[arg(long, default_value = "half_hour")]
    pub fee: String,
    #[arg(long)]
    pub max_fee_gwei: Option<f64>,
    #[arg(long)]
    pub priority_fee_gwei: Option<f64>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub wait: bool,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `wallet send speed-up` args (Story 17).
#[derive(clap::Args, Debug)]
pub struct SendSpeedupArgs {
    #[arg(long)]
    pub tx_hash: String,
    #[arg(long)]
    pub max_fee_gwei: f64,
    #[arg(long)]
    pub priority_fee_gwei: f64,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `tx` subcommands (Story 7).
#[derive(Subcommand, Debug)]
pub enum TxAction {
    List {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        since_block: Option<u64>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        json: bool,
    },
    Get {
        #[arg(long)]
        tx_hash: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
}

/// `erc20` subcommands (Stories 21-25).
#[derive(Subcommand, Debug)]
pub enum Erc20Action {
    Send {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        /// Token symbol (USDC/USDT/DAI) or hex address via `--token-address`.
        #[arg(long)]
        token: String,
        #[arg(long, value_parser = parse_address)]
        token_address: Option<String>,
        #[arg(long, value_parser = parse_address)]
        to: String,
        #[arg(long)]
        amount: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        gas_limit: Option<u64>,
        #[arg(long)]
        max_fee_gwei: Option<f64>,
        #[arg(long)]
        priority_fee_gwei: Option<f64>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    Balance {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long)]
        token: String,
        #[arg(long, value_parser = parse_address)]
        token_address: Option<String>,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        decimals: Option<u8>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
    List {
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        json: bool,
    },
    Register {
        #[arg(long, value_parser = parse_address)]
        address: String,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        list: bool,
        #[arg(long)]
        remove: Option<String>,
    },
    Approve {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        token: String,
        #[arg(long, value_parser = parse_address)]
        spender: String,
        #[arg(long, default_value = "0")]
        amount: String,
        #[arg(long)]
        unlimited: bool,
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        gas_limit: Option<u64>,
        #[arg(long)]
        max_fee_gwei: Option<f64>,
        #[arg(long)]
        priority_fee_gwei: Option<f64>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        rpc_url: Option<String>,
    },
}

/// `fee` args (Story 8).
#[derive(clap::Args, Debug)]
pub struct FeeArgs {
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `config` subcommands (Story 11).
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    Show {
        /// Network label to display in the resolved config (default: amoy).
        /// Honors `POLYGON_NETWORK` env var per the global convention.
        /// Reported value reflects this arg, not RPC-side chain_id
        /// (which would need a live `eth_chainId` call — out of scope).
        #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
        network: String,
        #[arg(long)]
        json: bool,
    },
}

/// `faucet` args (Story 30).
#[derive(clap::Args, Debug)]
pub struct FaucetArgs {
    #[arg(long, value_parser = parse_address)]
    pub address: String,
    #[arg(long, env = "POLYGON_NETWORK", default_value = "amoy")]
    pub network: String,
    #[arg(long, default_value = "POL")]
    pub faucet_token: String,
    /// Auto-claim reserved for T7 (operator-driven per L29).
    #[arg(long)]
    pub auto: bool,
}

/// `sign-message` args (Story 18) — EIP-191 personal_sign.
#[derive(clap::Args, Debug)]
pub struct SignMessageArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long)]
    pub message: String,
    #[arg(long, value_parser = parse_address)]
    pub address: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub verify: Option<String>,
    #[arg(long)]
    pub rpc_url: Option<String>,
}

/// `sign-typed` args (Story 27) — EIP-712 typed-data with chain_id validation.
///
/// Q7 critical-tier: `--chain-id` is **required** (no `Option`), and
/// `assert_polygon_chain_id` (handlers/sign.rs) rejects any chain_id
/// outside {137, 80002} before signing.
#[derive(clap::Args, Debug)]
pub struct SignTypedArgs {
    /// REQUIRED. Validated against {137, 80002} before signing (Q7 gate).
    #[arg(long)]
    pub chain_id: u64,
    #[arg(
        long,
        conflicts_with = "typed_data_file",
        required_unless_present = "typed_data_file"
    )]
    pub typed_data: Option<String>,
    #[arg(long, conflicts_with = "typed_data")]
    pub typed_data_file: Option<PathBuf>,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub address: Option<String>,
    #[arg(long, value_parser = parse_address)]
    pub verify: Option<String>,
    #[arg(long)]
    pub rpc_url: Option<String>,
}
