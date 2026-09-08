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

use crate::cli::UnitArg;

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

/// `NetworkArg` → core `Network`. Identity now that the two types are the same
/// — kept so existing call sites compile unchanged while `cli::NetworkArg` is
/// mid-removal.
pub fn network_of(arg: Network) -> Network {
    arg
}

/// Default TronGrid hosts that ship with v0.1. Anything else emits the
/// non-default-RPC warning gate (see [`open_client`]).
fn is_default_trongrid_host(host: &str) -> bool {
    matches!(
        host,
        "api.trongrid.io"
            | "api.shasta.trongrid.io"
            | "api.nile.trongrid.io"
            | "localhost"
            | "127.0.0.1"
            | "::1"
    )
}

/// Extracts the host portion from a URL, lowercased and port-stripped, or
/// `None` if the URL cannot be parsed. Used only for the non-default-RPC
/// warning — never as a security boundary (full SPKI pinning is the
/// follow-up).
fn host_of(url: &str) -> Option<String> {
    // `url::Url::parse` would pull a heavyweight dep; the simple scheme +
    // host extraction is enough for the STDERR warning we emit.
    let after_scheme = url.split_once("://")?.1;
    let host_part = after_scheme.split('/').next()?;
    let host_only = host_part.split(':').next()?;
    Some(host_only.to_ascii_lowercase())
}

/// Opens the RPC client for a command.
///
/// Priority: `--rpc-url` → `--network`'s default URL → the persisted config.
/// No SPKI pin is passed from the persisted config; the pin machinery was
/// stripped from config in `631a90f`. The CLI still honours an inline
/// `pinned://<hex-pin>@<host>` form on `--rpc-url` because V7
/// (`tests/v7_spki_pin.rs`) asserts on that wire shape. The pin is
/// extracted here and routed to `TronGridClient::new`; the URL handed to
/// reqwest is the bare `https://<host>` body.
///
/// When the RPC URL targets anything other than the four bundled TronGrid
/// hosts (mainnet, shasta, nile) or a loopback, a STDERR warning is emitted:
/// a signed envelope posted to an unknown host can be exfiltrated and
/// replayed inside its 60-second TAPOS window. We do not refuse, so
/// integration tests and operator scripts that point at a self-hosted node
/// keep working — but the warning is loud.
pub fn open_client(
    data_dir: &Path,
    network: Option<Network>,
    rpc_url: Option<String>,
) -> Result<TronGridClient> {
    let url = match (rpc_url, network) {
        (Some(u), _) => u,
        (None, Some(n)) => tron_wallet_core::config::default_rpc_url(n).to_string(),
        (None, None) => config::load(data_dir)?.rpc_url,
    };
    if url.trim().is_empty() {
        return Err(CliError::BadInput(
            "no RPC url: pass --rpc-url, --network, or run `tron config set-rpc <url>`".into(),
        ));
    }
    if let Some(host) = host_of(&url) {
        if !is_default_trongrid_host(&host) {
            eprintln!(
                "warning: connecting to non-default RPC {host}; \
                 signed envelope exfiltration risk for the 60s replay window"
            );
        }
    }
    let (stripped_url, pin) = parse_pinned_url(&url);
    Ok(TronGridClient::new(&stripped_url, pin)?)
}

/// Parse a `pinned://<hex-pin>@<host>` URL into `(https_url, Some(pin))`.
///
/// Returns `(url, None)` unchanged when the URL does not start with
/// `pinned://`. The hex pin must be 64 lowercase or uppercase hex chars
/// (32 bytes, the SHA-256 SPKI digest length). A malformed pin falls
/// through to the downstream reqwest builder error rather than being
/// silently swallowed — the same exit class as every other URL-shape
/// error.
fn parse_pinned_url(url: &str) -> (String, Option<tron_wallet_core::chain::SpkiPin>) {
    let Some(rest) = url.strip_prefix("pinned://") else {
        return (url.to_string(), None);
    };
    let Some((pin_hex, host_part)) = rest.split_once('@') else {
        return (url.to_string(), None);
    };
    let Some(pin_bytes) = decode_hex(pin_hex) else {
        return (url.to_string(), None);
    };
    if pin_bytes.len() != 32 {
        return (url.to_string(), None);
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&pin_bytes);
    let pin = tron_wallet_core::chain::SpkiPin::from_bytes(arr);
    (format!("https://{host_part}"), Some(pin))
}

/// Local hex decoder — `hex` is not a direct dep of this crate.
///
/// Accepts both lowercase and uppercase, returns `None` on any non-hex
/// char or odd length. Only used to read the SPKI pin prefix inside a
/// `pinned://<pin>@host` URL.
fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = decode_nibble(bytes[i])?;
        let lo = decode_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn decode_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// The network a command operates on: explicit flag, else persisted config.
pub fn effective_network(data_dir: &Path, network: Option<Network>) -> Result<Network> {
    match network {
        Some(n) => Ok(n),
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
///
/// TTY gate: a piped stdin (script context) cannot answer `yes` interactively,
/// and silently treating EOF as "no" is how scripts lose mainnet money. When
/// `--confirm-yes` is set the caller has already accepted the prompt on the
/// operator's behalf, so the TTY is bypassed.
pub fn confirm(prompt: &str, confirm_yes: bool) -> Result<()> {
    if confirm_yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        return Err(CliError::BadInput(
            "typed confirmation requires a TTY; pass --confirm-yes for scripts".into(),
        ));
    }
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

    /// The typed-yes gate must refuse when stdin isn't a TTY. `cargo test`
    /// runs the child process with stdout/stderr captured to a pipe rather
    /// than a terminal, which is the non-TTY case we want to exercise.
    #[test]
    fn confirm_requires_tty_or_flag() {
        let err = confirm("ship to mainnet?", false).expect_err("piped stdin must refuse");
        let msg = match err {
            CliError::BadInput(m) => m,
            other => panic!("expected BadInput, got {other:?}"),
        };
        assert!(msg.contains("TTY"), "error must mention TTY, got {msg:?}");
        assert!(
            msg.contains("--confirm-yes"),
            "error must mention --confirm-yes, got {msg:?}"
        );
    }

    /// `--confirm-yes` short-circuits the TTY gate, so a piped stdin is
    /// enough to authorise a mainnet send.
    #[test]
    fn confirm_bypass_when_flag_is_set() {
        assert!(confirm("ship to mainnet?", true).is_ok());
    }

    #[test]
    fn host_of_extracts_lowercased_authority() {
        assert_eq!(
            host_of("https://api.trongrid.io/foo"),
            Some("api.trongrid.io".into())
        );
        assert_eq!(
            host_of("https://API.SHASTA.trongrid.io:443/v1"),
            Some("api.shasta.trongrid.io".into())
        );
        assert_eq!(host_of("not a url"), None);
    }

    #[test]
    fn is_default_trongrid_host_lists_loopback_and_public_endpoints() {
        for h in [
            "api.trongrid.io",
            "api.shasta.trongrid.io",
            "api.nile.trongrid.io",
            "localhost",
            "127.0.0.1",
        ] {
            assert!(is_default_trongrid_host(h), "{h} should be default");
        }
        for h in ["attacker.example", "evil.trongrid.io.attacker.example"] {
            assert!(!is_default_trongrid_host(h), "{h} must NOT be default");
        }
    }
}
