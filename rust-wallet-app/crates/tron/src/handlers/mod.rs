//! Shared handler plumbing — plan §Phase 6 Task 5.8.
//!
//! Output discipline (plan §Task 5.8):
//!
//! - STDOUT carries *only* requested data (wallet id, address, balance, JSON).
//! - STDERR carries diagnostics, warnings, prompts, and secrets that must not
//!   be captured by `$(...)` — the mnemonic on `wallet create` above all.
//!
//! Exit codes (plan §Task 5.1, mirroring `btc/src/main.rs`):
//!
//! | code | meaning                            |
//! |------|------------------------------------|
//! | 0    | success                            |
//! | 1    | user abort (declined confirmation) |
//! | 2    | bad input / unsupported request    |
//! | 3    | upstream RPC transport failure     |
//! | 4    | wallet or balance issue            |
//! | 5    | signing / broadcast error          |

pub mod address;
pub mod balance;
pub mod config;
pub mod trc20;
pub mod tx;
pub mod wallet;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use tron_wallet_core::chain::TronGridClient;
use tron_wallet_core::config::Network;
use tron_wallet_core::keys::{DerivationPath, Language, Mnemonic};
use tron_wallet_core::platform::desktop::FileWalletStorage;
use tron_wallet_core::wallet::{WalletId, WalletManager};
use tron_wallet_core::Error as CoreError;
use zeroize::Zeroizing;

use crate::cli::{NetworkArg, UnitArg};

/// CLI-level error. Wraps the core error so the exit-code mapping lives in
/// exactly one place, and adds the two cases the core has no variant for:
/// a declined confirmation, and an argument shape rejected before the core is
/// reached.
#[derive(Debug)]
pub enum CliError {
    /// Propagated from `tron-wallet-core`.
    Core(CoreError),
    /// Operator declined a confirmation prompt.
    Abort,
    /// Argument shape the CLI rejects before reaching the core.
    BadInput(String),
}

impl From<CoreError> for CliError {
    fn from(e: CoreError) -> Self {
        Self::Core(e)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(e) => write!(f, "{e}"),
            Self::Abort => write!(f, "aborted by operator"),
            Self::BadInput(m) => write!(f, "bad input: {m}"),
        }
    }
}

/// Result alias for handlers.
pub type Result<T> = core::result::Result<T, CliError>;

/// Maps an error to the plan's process exit code.
pub fn exit_code(err: &CliError) -> i32 {
    match err {
        CliError::Abort => 1,
        CliError::BadInput(_) => 2,
        CliError::Core(e) => match e {
            // Transport / node reachability.
            CoreError::Node(_) => 3,
            // Wallet lookup, storage, or wrong passphrase.
            CoreError::Wallet(_) | CoreError::Encryption(_) => 4,
            // Signing, envelope construction, malformed node payload.
            CoreError::Signing(_) | CoreError::TransactionBuild(_) | CoreError::NodeResponse(_) => {
                5
            }
            // Everything else is operator input: bad mnemonic, bad address,
            // bad path, cross-network guard, bad config, bad pin.
            _ => 2,
        },
    }
}

/// Resolves the wallet + config directory.
///
/// `--data-dir` (or `TRON_DATA_DIR`) wins; otherwise the platform data dir
/// with a `tron` subdirectory. Created if missing.
pub fn resolve_data_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    let dir = match explicit {
        Some(d) => d,
        None => directories::ProjectDirs::from("", "", "tron")
            .map(|p| p.data_dir().to_path_buf())
            .ok_or_else(|| {
                CliError::BadInput(
                    "cannot resolve a platform data dir; pass --data-dir".to_string(),
                )
            })?,
    };
    std::fs::create_dir_all(&dir).map_err(|e| {
        CliError::Core(CoreError::Config(format!(
            "create data dir {}: {e}",
            dir.display()
        )))
    })?;
    Ok(dir)
}

/// Opens the desktop file-backed wallet store rooted at `data_dir/wallets`.
pub fn open_storage(data_dir: &Path) -> Result<FileWalletStorage> {
    Ok(FileWalletStorage::with_dir(data_dir.join("wallets"))?)
}

/// Parses a `WalletId` from its hex form, mapping a bad id to exit 2.
pub fn parse_wallet_id(raw: &str) -> Result<WalletId> {
    WalletId::from_str(raw).map_err(CliError::Core)
}

/// `NetworkArg` → core `Network`.
pub fn network_of(arg: NetworkArg) -> Network {
    match arg {
        NetworkArg::Mainnet => Network::Mainnet,
        NetworkArg::Shasta => Network::Shasta,
        NetworkArg::Nile => Network::Nile,
        NetworkArg::Local => Network::Local,
    }
}

/// Resolves the RPC client for a command.
///
/// Priority: `--rpc-url` → `--network`'s default URL → the persisted config.
/// No SPKI pin is passed; the pin machinery was stripped from config in
/// `631a90f`, so pinning is not a v0.1 CLI surface.
pub fn open_client(
    data_dir: &Path,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
) -> Result<TronGridClient> {
    let url = match (rpc_url, network) {
        (Some(u), _) => u,
        (None, Some(n)) => tron_wallet_core::config::default_rpc_url(network_of(n)).to_string(),
        (None, None) => config::load(data_dir)?.rpc_url,
    };
    if url.trim().is_empty() {
        return Err(CliError::BadInput(
            "no RPC url: pass --rpc-url, --network, or run `tron config set-rpc <url>`".into(),
        ));
    }
    Ok(TronGridClient::new(&url, None)?)
}

/// The network a command operates on: explicit flag, else persisted config.
pub fn effective_network(data_dir: &Path, network: Option<NetworkArg>) -> Result<Network> {
    match network {
        Some(n) => Ok(network_of(n)),
        None => Ok(config::load(data_dir)?.network),
    }
}

/// Resolves a passphrase: argv/env (folded in by clap `env`) → TTY prompt.
///
/// A passphrase on argv is visible in the process list and shell history, so
/// it earns a warning on STDERR. An empty passphrase is refused outright: an
/// empty-passphrase wallet is indistinguishable from an unencrypted one.
pub fn resolve_password(cli_pw: Option<String>, prompt: &str) -> Result<Zeroizing<String>> {
    if let Some(pw) = cli_pw {
        if pw.is_empty() {
            return Err(CliError::BadInput("passphrase must not be empty".into()));
        }
        eprintln!(
            "warning: passphrase supplied via flag/env is visible to other processes; \
             omit it for the TTY prompt"
        );
        return Ok(Zeroizing::new(pw));
    }
    if !std::io::stdin().is_terminal() {
        return Err(CliError::BadInput(
            "no passphrase and no TTY: pass --password or set TRON_PASSWORD".into(),
        ));
    }
    let pw = rpassword::prompt_password(prompt)
        .map_err(|e| CliError::Core(CoreError::Config(format!("passphrase prompt failed: {e}"))))?;
    if pw.is_empty() {
        return Err(CliError::BadInput("passphrase must not be empty".into()));
    }
    Ok(Zeroizing::new(pw))
}

/// Reads a mnemonic from argv or a file, then validates it.
pub fn resolve_mnemonic(inline: Option<String>, file: Option<PathBuf>) -> Result<Mnemonic> {
    let phrase: Zeroizing<String> = match (inline, file) {
        (Some(p), _) => {
            eprintln!(
                "warning: --mnemonic on argv is visible to other processes; \
                 prefer --mnemonic-file"
            );
            Zeroizing::new(p)
        }
        (None, Some(path)) => Zeroizing::new(
            std::fs::read_to_string(&path)
                .map_err(|e| {
                    CliError::Core(CoreError::Config(format!("read {}: {e}", path.display())))
                })?
                .trim()
                .to_string(),
        ),
        (None, None) => {
            return Err(CliError::BadInput(
                "one of --mnemonic or --mnemonic-file is required".into(),
            ))
        }
    };
    Ok(Mnemonic::from_phrase(&phrase, Language::English)?)
}

/// Unlocks a stored wallet and hands back a mnemonic the caller owns.
///
/// Errors for a raw-key wallet: there is no phrase to return, and the callers
/// of this helper (`address xpub`, `--to-wallet` resolution) all need one.
/// Commands that only need a signing key go through `UnlockedWallet::keypair`
/// instead, which works for both record kinds.
pub fn unlock(data_dir: &Path, id: &str, password: Option<String>) -> Result<Mnemonic> {
    let storage = open_storage(data_dir)?;
    let manager = WalletManager::new(&storage);
    let wallet_id = parse_wallet_id(id)?;
    let pw = resolve_password(password, "wallet passphrase: ")?;
    let unlocked = manager.unlock(wallet_id, &pw)?;
    let mnemonic = unlocked.mnemonic().ok_or_else(|| {
        CliError::Core(CoreError::Wallet(format!(
            "wallet {id} was imported from a raw private key, so it has no mnemonic"
        )))
    })?;
    // Re-validate through the constructor so the returned value owns its
    // phrase rather than borrowing from the `UnlockedWallet` guard.
    Ok(Mnemonic::from_phrase(
        mnemonic.phrase(),
        mnemonic.language(),
    )?)
}

/// Parses a derivation path, defaulting to the account-0 receive path with
/// `index` substituted into the final component.
pub fn derivation_path(explicit: Option<&str>, index: u32) -> Result<DerivationPath> {
    let raw = match explicit {
        Some(p) => p.to_string(),
        None => format!(
            "m/44'/{}'/0'/0/{index}",
            tron_wallet_core::keys::TRON_COIN_TYPE
        ),
    };
    raw.parse::<DerivationPath>()
        .map_err(|e| CliError::Core(CoreError::Derivation(format!("bad path {raw:?}: {e}"))))
}

/// Parses a decimal amount into the smallest unit given `decimals`.
///
/// Hand-rolled rather than via `f64`: `0.1` is not representable in binary
/// floating point, and a wallet that rounds a transfer amount is a bug that
/// costs money.
pub fn parse_decimal_amount(raw: &str, decimals: u32) -> Result<u128> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CliError::BadInput("amount is empty".into()));
    }
    let (int_part, frac_part) = match raw.split_once('.') {
        Some((i, f)) => (i, f),
        None => (raw, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(CliError::BadInput(format!("amount {raw:?} has no digits")));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(CliError::BadInput(format!(
            "amount {raw:?} is not a non-negative decimal"
        )));
    }
    if frac_part.len() as u32 > decimals {
        return Err(CliError::BadInput(format!(
            "amount {raw:?} has more than {decimals} decimal places"
        )));
    }
    let mut digits = String::with_capacity(int_part.len() + decimals as usize);
    digits.push_str(if int_part.is_empty() { "0" } else { int_part });
    digits.push_str(frac_part);
    for _ in 0..(decimals - frac_part.len() as u32) {
        digits.push('0');
    }
    digits
        .parse::<u128>()
        .map_err(|e| CliError::BadInput(format!("amount {raw:?} out of range: {e}")))
}

/// Formats a smallest-unit integer back to a decimal string.
pub fn format_units(value: u128, decimals: u32) -> String {
    if decimals == 0 {
        return value.to_string();
    }
    let scale = 10u128.pow(decimals);
    let whole = value / scale;
    let frac = value % scale;
    if frac == 0 {
        return whole.to_string();
    }
    let frac = format!("{frac:0width$}", width = decimals as usize);
    format!("{whole}.{}", frac.trim_end_matches('0'))
}

/// Renders a SUN amount in the requested unit.
pub fn render_trx(sun: u128, unit: UnitArg) -> String {
    match unit {
        UnitArg::Sun => sun.to_string(),
        UnitArg::Trx => format_units(sun, 6),
    }
}

/// Prompts for a typed `yes` confirmation. Anything else is an abort.
///
/// `yes`, not `y`: the prompt guards mainnet sends, wallet deletion, and
/// unlimited approvals, where a stray keystroke should not be enough.
pub fn confirm(prompt: &str) -> Result<()> {
    use std::io::Write;
    eprint!("{prompt} [type 'yes' to proceed]: ");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| CliError::Core(CoreError::Config(format!("read confirmation: {e}"))))?;
    if line.trim() == "yes" {
        Ok(())
    } else {
        Err(CliError::Abort)
    }
}

/// Resolves a token argument (`USDT` or a contract address) to a contract
/// address, using the network's token table.
pub fn resolve_token(network: Network, token: &str) -> Result<String> {
    if tron_wallet_core::address::Address::is_valid(token) {
        return Ok(token.to_string());
    }
    tron_wallet_core::tokens::by_symbol(network, token)
        .map(|t| t.address.clone())
        .ok_or_else(|| {
            CliError::BadInput(format!(
                "unknown token {token:?} on {}: pass a contract address",
                network.tag()
            ))
        })
}

/// Writes a JSON value to STDOUT, or a plain line when `--json` is off.
pub fn emit(json: bool, value: serde_json::Value, plain: &str) {
    if json {
        println!("{value}");
    } else {
        println!("{plain}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_amount_scales_without_float_rounding() {
        assert_eq!(parse_decimal_amount("1", 6).unwrap(), 1_000_000);
        assert_eq!(parse_decimal_amount("0.1", 6).unwrap(), 100_000);
        assert_eq!(parse_decimal_amount("0.000001", 6).unwrap(), 1);
        assert_eq!(parse_decimal_amount("12.345678", 6).unwrap(), 12_345_678);
    }

    #[test]
    fn decimal_amount_rejects_excess_precision() {
        // Silently truncating here would under-send by a sub-unit amount.
        assert!(matches!(
            parse_decimal_amount("0.0000001", 6),
            Err(CliError::BadInput(_))
        ));
    }

    #[test]
    fn decimal_amount_rejects_non_numeric() {
        assert!(matches!(
            parse_decimal_amount("-1", 6),
            Err(CliError::BadInput(_))
        ));
        assert!(matches!(
            parse_decimal_amount("1e6", 6),
            Err(CliError::BadInput(_))
        ));
    }

    #[test]
    fn format_units_round_trips() {
        for raw in ["1", "0.1", "0.000001", "12.345678"] {
            let sun = parse_decimal_amount(raw, 6).unwrap();
            assert_eq!(format_units(sun, 6), raw, "round-trip failed for {raw}");
        }
    }

    #[test]
    fn exit_codes_match_the_plan_table() {
        assert_eq!(exit_code(&CliError::Abort), 1);
        assert_eq!(exit_code(&CliError::BadInput("x".into())), 2);
        assert_eq!(
            exit_code(&CliError::Core(CoreError::Node("down".into()))),
            3
        );
        assert_eq!(
            exit_code(&CliError::Core(CoreError::Wallet("missing".into()))),
            4
        );
        assert_eq!(
            exit_code(&CliError::Core(CoreError::Encryption("bad pw".into()))),
            4
        );
        assert_eq!(
            exit_code(&CliError::Core(CoreError::Signing("nope".into()))),
            5
        );
        assert_eq!(
            exit_code(&CliError::Core(CoreError::Mnemonic("bad".into()))),
            2
        );
    }

    #[test]
    fn render_trx_honours_unit() {
        assert_eq!(render_trx(1_500_000, UnitArg::Trx), "1.5");
        assert_eq!(render_trx(1_500_000, UnitArg::Sun), "1500000");
    }
}
