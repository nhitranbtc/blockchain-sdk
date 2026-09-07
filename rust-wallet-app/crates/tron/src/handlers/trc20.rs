//! `tron trc20` handlers — plan §Phase 6 Task 5.5.
//!
//! All four subcommands are wired against the core: `balance` and `allowance`
//! are constant-contract reads, `send` and `approve` go through
//! `tx::submit_trc20{,_approve}`.
//!
//! The unlimited-approval confirmation lives here: an allowance near
//! `U256::MAX` lets the spender drain the balance at any future time, which is
//! the most common way TRC-20 holders lose funds.

use std::path::{Path, PathBuf};

use ethereum_types::U256;
use tron_wallet_core::address::Address;
use tron_wallet_core::config::Network;
use tron_wallet_core::keys::{derive_keypair, SECRET_KEY_LEN};
use tron_wallet_core::tx::submit::{self, SubmitOptions};
use tron_wallet_core::wallet::{WalletKind, WalletManager};
use zeroize::Zeroizing;

use super::{
    confirm, derivation_path, effective_network, emit, format_units, open_client, open_storage,
    parse_decimal_amount, parse_wallet_id, resolve_mnemonic, resolve_password, resolve_token,
    CliError, Result,
};
use crate::cli::Network as NetworkArg;
use crate::handlers::wallet::{fee_limit_sun, report_broadcast};

/// Decimals for a contract: the bundled registry first, the live `decimals()`
/// call as a fallback for tokens the registry does not carry.
async fn decimals_of(
    client: &tron_wallet_core::chain::TronGridClient,
    network: Network,
    contract: &str,
    owner: &str,
) -> Result<u8> {
    if let Some(token) = tron_wallet_core::tokens::by_address(network, contract) {
        return Ok(token.decimals);
    }
    Ok(tron_wallet_core::trc20::decimals(client, contract, owner).await?)
}

/// Shared `balanceOf` reader used by both `trc20 balance` and
/// `wallet balance --token`.
pub async fn print_balance(
    data_dir: &Path,
    owner: String,
    token: String,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    let net = effective_network(data_dir, network)?;
    let contract = resolve_token(net, &token)?;
    let client = open_client(data_dir, network, rpc_url)?;
    let raw = tron_wallet_core::trc20::balance_of(&client, &contract, &owner).await?;
    let decimals = decimals_of(&client, net, &contract, &owner).await?;
    let (raw_str, formatted) = render_amount(raw, decimals);

    emit(
        json,
        serde_json::json!({
            "address": owner,
            "contract": contract,
            "decimals": decimals,
            "raw": raw_str,
            "amount": formatted,
        }),
        formatted.as_deref().unwrap_or(&raw_str),
    );
    Ok(())
}

/// `U256` can exceed `u128`; a 2^128 balance is pathological but formatting must
/// not panic on it, so the human-readable form is omitted rather than wrapped.
fn render_amount(raw: U256, decimals: u8) -> (String, Option<String>) {
    let raw_str = raw.to_string();
    let formatted = raw_str
        .parse::<u128>()
        .ok()
        .map(|v| format_units(v, u32::from(decimals)));
    (raw_str, formatted)
}

/// `tron trc20 balance`.
pub async fn balance(
    data_dir: &Path,
    address: String,
    contract: String,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    print_balance(data_dir, address, contract, network, rpc_url, json).await
}

/// `tron trc20 allowance` — remaining spender allowance.
pub async fn allowance(
    data_dir: &Path,
    contract: String,
    owner: String,
    spender: String,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    let net = effective_network(data_dir, network)?;
    let contract = resolve_token(net, &contract)?;
    let client = open_client(data_dir, network, rpc_url)?;
    let raw = tron_wallet_core::trc20::allowance(&client, &contract, &owner, &spender).await?;
    let decimals = decimals_of(&client, net, &contract, &owner).await?;
    let (raw_str, formatted) = render_amount(raw, decimals);

    emit(
        json,
        serde_json::json!({
            "contract": contract,
            "owner": owner,
            "spender": spender,
            "decimals": decimals,
            "raw": raw_str,
            "amount": formatted,
            "unlimited": is_unlimited_approval(raw),
        }),
        formatted.as_deref().unwrap_or(&raw_str),
    );
    Ok(())
}

/// Resolve the signing secret + owner address from `--wallet-id`,
/// `--mnemonic`, or `--mnemonic-file`.
///
/// Returns the secret bytes rather than a `KeyPair` for the same reason
/// `handlers::wallet::signer` does: `KeyPair` is no longer `Clone`,
/// so we cannot hand an owned `KeyPair` back from a borrowed raw-key one
/// (PR #545 review finding G).
fn signer(
    data_dir: &Path,
    wallet_id: Option<String>,
    mnemonic: Option<String>,
    mnemonic_file: Option<PathBuf>,
    password: Option<String>,
) -> Result<(String, Zeroizing<[u8; SECRET_KEY_LEN]>)> {
    let path = derivation_path(None, 0)?;
    match (wallet_id, mnemonic, mnemonic_file) {
        (Some(id), _, _) => {
            let storage = open_storage(data_dir)?;
            let manager = WalletManager::new(&storage);
            let wallet_id = parse_wallet_id(&id)?;
            let password = resolve_password(password, "wallet passphrase: ")?;
            let unlocked = manager.unlock(wallet_id, &password)?;
            let owner = match unlocked.kind() {
                WalletKind::Mnemonic => {
                    let keypair = unlocked.keypair(&path)?;
                    Address::from_public_key(keypair.public_key())?.to_base58()
                }
                WalletKind::PrivateKey => {
                    let keypair = unlocked.raw_keypair()?;
                    Address::from_public_key(keypair.public_key())?.to_base58()
                }
            };
            let secret = match unlocked.kind() {
                WalletKind::Mnemonic => {
                    let keypair = unlocked.keypair(&path)?;
                    keypair.secret_bytes().clone()
                }
                WalletKind::PrivateKey => unlocked.raw_keypair()?.secret_bytes().clone(),
            };
            Ok((owner, secret))
        }
        (None, Some(phrase), _) => {
            let mnemonic = resolve_mnemonic(Some(phrase), None)?;
            let keypair = derive_keypair(&mnemonic, "", &path)?;
            let owner = Address::from_public_key(keypair.public_key())?.to_base58();
            Ok((owner, keypair.secret_bytes().clone()))
        }
        (None, None, Some(path_buf)) => {
            let mnemonic = resolve_mnemonic(None, Some(path_buf))?;
            let keypair = derive_keypair(&mnemonic, "", &path)?;
            let owner = Address::from_public_key(keypair.public_key())?.to_base58();
            Ok((owner, keypair.secret_bytes().clone()))
        }
        (None, None, None) => Err(CliError::BadInput(
            "one of --wallet-id, --mnemonic, or --mnemonic-file is required".into(),
        )),
    }
}

/// Token amount in the contract's smallest unit.
async fn token_amount(
    client: &tron_wallet_core::chain::TronGridClient,
    net: Network,
    contract: &str,
    owner: &str,
    amount: &str,
) -> Result<U256> {
    let decimals = decimals_of(client, net, contract, owner).await?;
    let scaled = parse_decimal_amount(amount, u32::from(decimals))?;
    U256::from_dec_str(&scaled.to_string())
        .map_err(|e| CliError::BadInput(format!("amount out of range: {e}")))
}

/// `tron trc20 send` — TRC-20 transfer.
#[allow(clippy::too_many_arguments)]
pub async fn send(
    data_dir: &Path,
    mnemonic: Option<String>,
    mnemonic_file: Option<PathBuf>,
    wallet_id: Option<String>,
    contract: String,
    to: String,
    amount: String,
    fee_limit: Option<i64>,
    password: Option<String>,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    confirm_yes: bool,
    json: bool,
) -> Result<()> {
    let net = effective_network(data_dir, network)?;
    let contract = resolve_token(net, &contract)?;
    if !Address::is_valid(&to) {
        return Err(CliError::BadInput(format!(
            "recipient {to:?} is not a TRON address"
        )));
    }
    let (owner, secret) = signer(data_dir, wallet_id, mnemonic, mnemonic_file, password)?;
    let client = open_client(data_dir, network, rpc_url)?;
    let value = token_amount(&client, net, &contract, &owner, &amount).await?;

    if net == Network::Mainnet {
        confirm(
            &format!("send {amount} of {contract} from {owner} to {to} on MAINNET"),
            confirm_yes,
        )?;
    }

    let opts = SubmitOptions {
        fee_limit_sun: fee_limit_sun(fee_limit)?,
        ..SubmitOptions::default()
    };
    let submitted =
        submit::submit_trc20(&client, &secret, &owner, &contract, &to, value, opts).await?;
    report_broadcast(&submitted.txid(), submitted.receipt(), json)
}

/// `tron trc20 approve` — set a spender allowance.
#[allow(clippy::too_many_arguments)]
pub async fn approve(
    data_dir: &Path,
    mnemonic: Option<String>,
    mnemonic_file: Option<PathBuf>,
    wallet_id: Option<String>,
    contract: String,
    spender: String,
    amount: String,
    fee_limit: Option<i64>,
    password: Option<String>,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    confirm_yes: bool,
    json: bool,
) -> Result<()> {
    let net = effective_network(data_dir, network)?;
    let contract = resolve_token(net, &contract)?;
    if !Address::is_valid(&spender) {
        return Err(CliError::BadInput(format!(
            "spender {spender:?} is not a TRON address"
        )));
    }
    let (owner, secret) = signer(data_dir, wallet_id, mnemonic, mnemonic_file, password)?;
    let client = open_client(data_dir, network, rpc_url)?;

    // `max` is accepted as a spelling of "unlimited" so operators do not have
    // to paste 78 digits — and so the guard below has something to match on.
    let value = if amount.trim().eq_ignore_ascii_case("max") {
        U256::MAX
    } else {
        token_amount(&client, net, &contract, &owner, &amount).await?
    };

    if is_unlimited_approval(value) {
        confirm(
            &format!("grant {spender} an UNLIMITED allowance over {owner}'s {contract} balance"),
            confirm_yes,
        )?;
    } else if net == Network::Mainnet {
        confirm(
            &format!("approve {amount} of {contract} for {spender} on MAINNET"),
            confirm_yes,
        )?;
    }

    let opts = SubmitOptions {
        fee_limit_sun: fee_limit_sun(fee_limit)?,
        ..SubmitOptions::default()
    };
    let submitted =
        submit::submit_trc20_approve(&client, &secret, &owner, &contract, &spender, value, opts)
            .await?;
    report_broadcast(&submitted.txid(), submitted.receipt(), json)
}

/// Whether an allowance is effectively unlimited.
///
/// Delegates to the core threshold so the CLI confirmation gate and the ABI it
/// protects cannot drift. Takes `U256` rather than a decimal string: the
/// previous digit-count test read a *rendered* value, so any future change to
/// how amounts are formatted (padding, grouping, a `0x` prefix) would have
/// silently reclassified an unlimited approval as bounded and skipped the
/// prompt.
pub fn is_unlimited_approval(amount: U256) -> bool {
    tron_wallet_core::trc20::is_unlimited(amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_approval_detection() {
        assert!(is_unlimited_approval(U256::MAX));
        // 9 * 10^38 — a 39-digit value, which is what `max` renders as in
        // practice and above the 2^128-1 threshold.
        assert!(is_unlimited_approval(
            U256::from(9u64) * U256::from(10u64).pow(38.into())
        ));
        assert!(!is_unlimited_approval(U256::from(1000u64)));
        assert!(!is_unlimited_approval(U256::zero()));
    }

    #[test]
    fn is_unlimited_threshold_boundary() {
        // The gate is strictly-greater, so the threshold itself still prompts
        // as a bounded allowance.
        let threshold = tron_wallet_core::trc20::UNLIMITED_APPROVAL_THRESHOLD;
        assert!(is_unlimited_approval(threshold + 1));
        assert!(!is_unlimited_approval(threshold));
        assert!(!is_unlimited_approval(threshold - 1));
    }

    #[test]
    fn render_amount_falls_back_to_raw_beyond_u128() {
        let (raw, formatted) = render_amount(U256::MAX, 6);
        assert!(formatted.is_none(), "must not wrap a value beyond u128");
        assert_eq!(raw, U256::MAX.to_string());
    }

    #[test]
    fn render_amount_scales_by_decimals() {
        let (_, formatted) = render_amount(U256::from(1_500_000u64), 6);
        assert_eq!(formatted.as_deref(), Some("1.5"));
    }
}
