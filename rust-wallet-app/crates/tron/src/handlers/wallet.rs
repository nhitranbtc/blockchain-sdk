//! `tron wallet` handlers — plan §Phase 6 Task 5.2.
//!
//! All nine subcommands are wired: the Phase 2/3/5 core carry-overs they needed
//! (`chain::get_account`, `tx::submit_*`, wallet-record metadata) landed
//! alongside this file.
//!
//! Two safety rules live here rather than in the core, because they are about
//! an operator at a terminal, not about the chain:
//!
//! - a mainnet send asks for a typed `yes`;
//! - the recovery phrase goes to STDERR, never STDOUT.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tron_wallet_core::address::Address;
use tron_wallet_core::keys::{derive_keypair, KeyPair, Language, Mnemonic, MnemonicType};
use tron_wallet_core::tx::submit::{self, SubmitOptions};
use tron_wallet_core::wallet::WalletManager;

use super::{
    confirm, derivation_path, effective_network, emit, format_units, network_of, open_client,
    open_storage, parse_decimal_amount, parse_wallet_id, resolve_mnemonic, resolve_password,
    unlock, CliError, Result,
};
use crate::cli::{NetworkArg, UnitArg};

/// Derives the account-0 (or `path`-overridden) T-address for a mnemonic.
fn address_of(mnemonic: &Mnemonic, path: Option<&str>) -> Result<String> {
    let path = derivation_path(path, 0)?;
    let keypair = derive_keypair(mnemonic, "", &path)?;
    Ok(Address::from_public_key(keypair.public_key())?.to_base58())
}

/// `tron wallet create` — generate a mnemonic, persist it encrypted.
///
/// The mnemonic goes to STDERR and the wallet id to STDOUT (plan §Task 5.8),
/// so `id=$(tron wallet create ...)` captures the id without ever putting the
/// seed phrase into a shell variable or a log pipe.
pub fn create(
    data_dir: &Path,
    words: u8,
    name: Option<String>,
    network: NetworkArg,
    password: Option<String>,
    json: bool,
) -> Result<()> {
    let word_count = match words {
        12 => MnemonicType::Words12,
        24 => MnemonicType::Words24,
        other => {
            return Err(CliError::BadInput(format!(
                "--words must be 12 or 24, got {other}"
            )))
        }
    };
    let password = resolve_password(password, "new wallet passphrase: ")?;
    let mnemonic = Mnemonic::generate(word_count, Language::English);
    let net = network_of(network);
    let storage = open_storage(data_dir)?;
    let id = WalletManager::new(&storage).create_with_meta(
        &mnemonic,
        &password,
        name.as_deref(),
        Some(net.tag()),
    )?;
    let address = address_of(&mnemonic, None)?;

    eprintln!("network: {}", net.tag());
    eprintln!("RECOVERY PHRASE (write this down; it is shown once):");
    eprintln!("  {}", mnemonic.phrase());
    emit(
        json,
        serde_json::json!({
            "wallet_id": id.to_hex(),
            "address": address,
            "name": name,
            "network": net.tag(),
        }),
        &id.to_hex(),
    );
    Ok(())
}

/// `tron wallet import` — persist an existing mnemonic or raw private key.
#[allow(clippy::too_many_arguments)]
pub fn import(
    data_dir: &Path,
    mnemonic: Option<String>,
    mnemonic_file: Option<PathBuf>,
    private_key_file: Option<PathBuf>,
    name: Option<String>,
    network: NetworkArg,
    password: Option<String>,
    json: bool,
) -> Result<()> {
    let net = network_of(network);
    let storage = open_storage(data_dir)?;
    let manager = WalletManager::new(&storage);

    // The raw-key branch reads from a file only: a private key on argv would
    // land in shell history and in every `ps` listing on the box.
    if let Some(path) = private_key_file {
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            CliError::Core(tron_wallet_core::Error::Config(format!(
                "read {}: {e}",
                path.display()
            )))
        })?;
        let password = resolve_password(password, "new wallet passphrase: ")?;
        let id =
            manager.import_private_key(raw.trim(), &password, name.as_deref(), Some(net.tag()))?;
        let unlocked = manager.unlock(id, &password)?;
        let keypair = unlocked.keypair(&derivation_path(None, 0)?)?;
        let address = Address::from_public_key(keypair.public_key())?.to_base58();
        eprintln!("imported a raw-key wallet: it has no recovery phrase to back up");
        emit(
            json,
            serde_json::json!({
                "wallet_id": id.to_hex(),
                "address": address,
                "name": name,
                "network": net.tag(),
                "is_private_key": true,
            }),
            &id.to_hex(),
        );
        return Ok(());
    }

    let mnemonic = resolve_mnemonic(mnemonic, mnemonic_file)?;
    let password = resolve_password(password, "new wallet passphrase: ")?;
    let id = manager.create_with_meta(&mnemonic, &password, name.as_deref(), Some(net.tag()))?;
    let address = address_of(&mnemonic, None)?;
    emit(
        json,
        serde_json::json!({
            "wallet_id": id.to_hex(),
            "address": address,
            "name": name,
            "network": net.tag(),
            "is_private_key": false,
        }),
        &id.to_hex(),
    );
    Ok(())
}

/// `tron wallet show` — decrypt and print the wallet's address + metadata.
pub fn show(
    data_dir: &Path,
    id: String,
    password: Option<String>,
    path: Option<String>,
    json: bool,
) -> Result<()> {
    let storage = open_storage(data_dir)?;
    let manager = WalletManager::new(&storage);
    let wallet_id = parse_wallet_id(&id)?;
    let password = resolve_password(password, "wallet passphrase: ")?;
    let unlocked = manager.unlock(wallet_id, &password)?;

    let derivation = derivation_path(path.as_deref(), 0)?;
    let keypair = unlocked.keypair(&derivation)?;
    let address = Address::from_public_key(keypair.public_key())?.to_base58();
    let summary = unlocked.summary();

    emit(
        json,
        serde_json::json!({
            "wallet_id": id,
            "address": address,
            "name": summary.name,
            "network": summary.network,
            "is_private_key": summary.is_private_key,
            "path": derivation.to_string(),
        }),
        &address,
    );
    Ok(())
}

/// `tron wallet list` — ids, or full summaries when a passphrase is available.
///
/// Without a passphrase only ids can be shown: the label and network live
/// inside the encrypted record.
pub fn list(
    data_dir: &Path,
    json: bool,
    all_networks: bool,
    password: Option<String>,
    network: Option<NetworkArg>,
) -> Result<()> {
    let storage = open_storage(data_dir)?;
    let manager = WalletManager::new(&storage);

    let Some(password) = password else {
        let ids: Vec<String> = manager.list()?.into_iter().map(|id| id.to_hex()).collect();
        if json {
            println!("{}", serde_json::json!({ "wallets": ids }));
        } else {
            for id in &ids {
                println!("{id}");
            }
            if !ids.is_empty() {
                eprintln!("pass --password (or TRON_PASSWORD) to see names and networks");
            }
        }
        return Ok(());
    };

    let filter = if all_networks {
        None
    } else {
        network.map(|n| network_of(n).tag().to_string())
    };
    let summaries: Vec<_> = manager
        .list_summaries(&password)?
        .into_iter()
        .filter(|s| match (&filter, &s.network) {
            (Some(want), Some(have)) => want == have,
            (Some(_), None) => false,
            (None, _) => true,
        })
        .collect();

    if json {
        let rows: Vec<_> = summaries
            .iter()
            .map(|s| {
                serde_json::json!({
                    "wallet_id": s.id.to_hex(),
                    "name": s.name,
                    "network": s.network,
                    "is_private_key": s.is_private_key,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "wallets": rows }));
    } else {
        for s in &summaries {
            println!(
                "{}  {}  {}",
                s.id.to_hex(),
                s.network.as_deref().unwrap_or("-"),
                s.name.as_deref().unwrap_or("-")
            );
        }
    }
    Ok(())
}

/// `tron wallet delete` — remove a stored blob after confirmation.
///
/// Irreversible: the blob is the only copy of the encrypted phrase this machine
/// holds.
pub fn delete(data_dir: &Path, id: String, confirm_yes: bool) -> Result<()> {
    let wallet_id = parse_wallet_id(&id)?;
    if !confirm_yes {
        confirm(&format!(
            "delete wallet {id}? the encrypted phrase cannot be recovered"
        ))?;
    }
    let storage = open_storage(data_dir)?;
    WalletManager::new(&storage).delete(wallet_id)?;
    eprintln!("deleted {id}");
    Ok(())
}

/// `tron wallet rename` — rewrite the label inside the encrypted record.
pub fn rename(data_dir: &Path, id: String, to: String, password: Option<String>) -> Result<()> {
    let wallet_id = parse_wallet_id(&id)?;
    let password = resolve_password(password, "wallet passphrase: ")?;
    let storage = open_storage(data_dir)?;
    WalletManager::new(&storage).rename(wallet_id, &password, &to)?;
    eprintln!("renamed {id} to {to:?}");
    Ok(())
}

/// Resolve the address a balance query is about.
fn balance_target(
    data_dir: &Path,
    wallet_id: Option<String>,
    address: Option<String>,
    password: Option<String>,
) -> Result<String> {
    match (address, wallet_id) {
        (Some(a), _) => Ok(a),
        (None, Some(id)) => {
            let mnemonic = unlock(data_dir, &id, password)?;
            address_of(&mnemonic, None)
        }
        (None, None) => Err(CliError::BadInput(
            "one of --address or --wallet-id is required".into(),
        )),
    }
}

/// `tron wallet balance` — native TRX, or a TRC-20 token with `--token`.
#[allow(clippy::too_many_arguments)]
pub async fn balance(
    data_dir: &Path,
    wallet_id: Option<String>,
    address: Option<String>,
    token: Option<String>,
    password: Option<String>,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    let owner = balance_target(data_dir, wallet_id, address, password)?;
    match token {
        Some(token) => {
            super::trc20::print_balance(data_dir, owner, token, network, rpc_url, json).await
        }
        None => {
            let client = open_client(data_dir, network, rpc_url)?;
            let account = client.get_account(&owner).await?;
            let trx = format_units(u128::from(account.balance_sun), 6);
            emit(
                json,
                serde_json::json!({
                    "address": owner,
                    "exists": account.exists,
                    "balance_sun": account.balance_sun,
                    "balance_trx": trx,
                }),
                &trx,
            );
            if !account.exists {
                eprintln!("note: this address has no on-chain record yet (never funded)");
            }
            Ok(())
        }
    }
}

/// Everything `wallet send` needs, grouped so the handler keeps one argument.
#[derive(Debug, Clone)]
pub struct SendArgs {
    pub wallet_id: Option<String>,
    pub mnemonic: Option<String>,
    pub to: Option<String>,
    pub to_wallet: Option<String>,
    pub amount: String,
    pub unit: UnitArg,
    pub fee_limit: Option<i64>,
    pub dry_run: bool,
    pub sign_only: bool,
    pub wait: bool,
    pub confirm_yes: bool,
    pub password: Option<String>,
    pub network: Option<NetworkArg>,
    pub rpc_url: Option<String>,
}

/// `tron wallet send` — native TRX transfer.
///
/// `--dry-run` stops after building, `--sign-only` after signing; neither
/// broadcasts, though both still read the head block, because TAPOS binds a
/// transaction to a reference block before it can be signed at all.
pub async fn send(data_dir: &Path, args: SendArgs, json: bool) -> Result<()> {
    let net = effective_network(data_dir, args.network)?;
    let recipient = match (&args.to, &args.to_wallet) {
        (Some(to), _) => to.clone(),
        (None, Some(wallet)) => {
            let mnemonic = unlock(data_dir, wallet, args.password.clone())?;
            address_of(&mnemonic, None)?
        }
        (None, None) => {
            return Err(CliError::BadInput(
                "one of --to or --to-wallet is required".into(),
            ))
        }
    };
    if !Address::is_valid(&recipient) {
        return Err(CliError::BadInput(format!(
            "recipient {recipient:?} is not a TRON address"
        )));
    }

    let amount_sun = parse_trx_amount(&args.amount, args.unit)?;
    let (owner, keypair) = signer(data_dir, &args)?;

    if net == tron_wallet_core::config::Network::Mainnet && !args.confirm_yes {
        confirm(&format!(
            "send {} TRX from {owner} to {recipient} on MAINNET",
            format_units(u128::from(amount_sun), 6)
        ))?;
    }

    let client = open_client(data_dir, args.network, args.rpc_url.clone())?;
    let opts = SubmitOptions {
        fee_limit_sun: args.fee_limit,
        ..SubmitOptions::default()
    };

    let params = submit::prepare_trx(&client, &owner, &recipient, amount_sun, opts).await?;
    if args.dry_run {
        emit(
            json,
            serde_json::json!({
                "dry_run": true,
                "from": owner,
                "to": recipient,
                "amount_sun": amount_sun,
            }),
            &format!("would send {amount_sun} SUN from {owner} to {recipient}"),
        );
        return Ok(());
    }

    let signed = submit::sign_prepared(keypair.secret_bytes(), &params)?;
    if args.sign_only {
        emit(
            json,
            serde_json::json!({
                "txid": signed.txid_hex(),
                "raw_data_hex": signed.raw_data_hex,
                "signature_hex": signed.signature_hex,
                "signed_envelope_hex": signed.signed_envelope_hex,
            }),
            &signed.signed_envelope_hex,
        );
        return Ok(());
    }

    let receipt = submit::broadcast_signed(&client, &signed).await?;
    report_broadcast(&signed.txid_hex(), &receipt, json)?;

    if args.wait {
        wait_and_report(&client, &signed.txid_hex(), json).await?;
    }
    Ok(())
}

/// Parse an amount in TRX or SUN into SUN.
pub(crate) fn parse_trx_amount(amount: &str, unit: UnitArg) -> Result<u64> {
    match unit {
        UnitArg::Sun => amount
            .trim()
            .parse::<u64>()
            .map_err(|e| CliError::BadInput(format!("amount in SUN must be an integer: {e}"))),
        UnitArg::Trx => u64::try_from(parse_decimal_amount(amount, 6)?)
            .map_err(|_| CliError::BadInput("amount overflows u64 SUN".into())),
    }
}

/// Resolve the signing keypair and its address from `--wallet-id` or
/// `--mnemonic`.
fn signer(data_dir: &Path, args: &SendArgs) -> Result<(String, KeyPair)> {
    let path = derivation_path(None, 0)?;
    match (&args.wallet_id, &args.mnemonic) {
        (Some(id), _) => {
            let storage = open_storage(data_dir)?;
            let manager = WalletManager::new(&storage);
            let wallet_id = parse_wallet_id(id)?;
            let password = resolve_password(args.password.clone(), "wallet passphrase: ")?;
            let unlocked = manager.unlock(wallet_id, &password)?;
            let keypair = unlocked.keypair(&path)?;
            let owner = Address::from_public_key(keypair.public_key())?.to_base58();
            Ok((owner, keypair))
        }
        (None, Some(phrase)) => {
            let mnemonic = resolve_mnemonic(Some(phrase.clone()), None)?;
            let keypair = derive_keypair(&mnemonic, "", &path)?;
            let owner = Address::from_public_key(keypair.public_key())?.to_base58();
            Ok((owner, keypair))
        }
        (None, None) => Err(CliError::BadInput(
            "one of --wallet-id or --mnemonic is required".into(),
        )),
    }
}

/// Print a broadcast result, and turn a node-side rejection into exit 5.
///
/// A rejected transaction is not a success with a warning: a script reading
/// exit 0 as "sent" would ship goods for a transfer the chain refused.
pub(crate) fn report_broadcast(
    txid: &str,
    receipt: &tron_wallet_core::tx::broadcast::BroadcastReceipt,
    json: bool,
) -> Result<()> {
    emit(
        json,
        serde_json::json!({
            "txid": txid,
            "broadcast_success": receipt.is_success(),
            "code": receipt.code,
            "message": receipt.message,
        }),
        txid,
    );
    if receipt.is_success() {
        Ok(())
    } else {
        Err(CliError::Core(tron_wallet_core::Error::TransactionBuild(
            format!(
                "node rejected {txid}: {} {}",
                receipt.code.clone().unwrap_or_default(),
                receipt.message.clone().unwrap_or_default()
            ),
        )))
    }
}

/// Poll until the transaction confirms, then print the receipt.
pub(crate) async fn wait_and_report(
    client: &tron_wallet_core::chain::TronGridClient,
    txid: &str,
    json: bool,
) -> Result<()> {
    let info = submit::wait_for_confirm(
        client,
        txid,
        Duration::from_secs(90),
        Duration::from_secs(3),
    )
    .await?;
    emit(
        json,
        serde_json::json!({
            "txid": info.id,
            "block_number": info.block_number,
            "contract_result": info.contract_result,
            "fee": info.fee,
        }),
        &format!(
            "confirmed in block {}",
            info.block_number
                .map(|b| b.to_string())
                .unwrap_or_else(|| "-".into())
        ),
    );
    Ok(())
}

/// `tron wallet send-speedup` — re-send a stuck call with a higher fee limit.
#[allow(clippy::too_many_arguments)]
pub async fn send_speedup(
    data_dir: &Path,
    wallet_id: String,
    txid: String,
    fee_limit: i64,
    password: Option<String>,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    if fee_limit <= 0 {
        return Err(CliError::BadInput(
            "--fee-limit must be greater than zero".into(),
        ));
    }
    let storage = open_storage(data_dir)?;
    let manager = WalletManager::new(&storage);
    let id = parse_wallet_id(&wallet_id)?;
    let password = resolve_password(password, "wallet passphrase: ")?;
    let keypair = manager
        .unlock(id, &password)?
        .keypair(&derivation_path(None, 0)?)?;

    let client = open_client(data_dir, network, rpc_url)?;
    // TRON has no replace-by-fee: this is a *new* transaction with a new txid.
    eprintln!("note: TRON has no replace-by-fee — this broadcasts a new transaction");
    let submitted =
        submit::submit_send_speedup(&client, keypair.secret_bytes(), &txid, fee_limit).await?;
    report_broadcast(&submitted.signed.txid_hex(), &submitted.receipt, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The known BIP-39 all-`abandon` vector.
    const VECTOR: &str = "abandon abandon abandon abandon abandon abandon \
                          abandon abandon abandon abandon abandon about";

    #[test]
    fn address_of_uses_the_tron_account_zero_path() {
        let mnemonic = Mnemonic::from_phrase(VECTOR, Language::English).expect("vector");
        let addr = address_of(&mnemonic, None).expect("derive");
        assert!(addr.starts_with('T'), "TRON base58 addresses start with T");
        assert_eq!(addr.len(), 34);
    }

    #[test]
    fn create_then_unlock_round_trips_with_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = open_storage(dir.path()).expect("storage");
        let manager = WalletManager::new(&storage);
        let mnemonic = Mnemonic::from_phrase(VECTOR, Language::English).expect("vector");
        let id = manager
            .create_with_meta(&mnemonic, "correct-horse", Some("daily"), Some("nile"))
            .expect("create");
        let unlocked = manager.unlock(id, "correct-horse").expect("unlock");
        assert_eq!(unlocked.mnemonic().expect("phrase").phrase(), VECTOR);
        assert_eq!(unlocked.name(), Some("daily"));
    }

    #[test]
    fn trx_amounts_parse_in_both_units() {
        assert_eq!(
            parse_trx_amount("1.5", UnitArg::Trx).expect("trx"),
            1_500_000
        );
        assert_eq!(
            parse_trx_amount("1500000", UnitArg::Sun).expect("sun"),
            1_500_000
        );
    }

    #[test]
    fn a_fractional_sun_amount_is_rejected() {
        // SUN is the base unit; "0.5 SUN" is not a thing, and rounding it would
        // silently change the transfer.
        assert!(matches!(
            parse_trx_amount("0.5", UnitArg::Sun),
            Err(CliError::BadInput(_))
        ));
    }

    #[test]
    fn a_rejected_broadcast_is_an_error_not_a_warning() {
        let receipt = tron_wallet_core::tx::broadcast::BroadcastReceipt {
            code: Some("CONTRACT_VALIDATE_ERROR".into()),
            txid: Some("deadbeef".into()),
            message: Some("balance is not sufficient".into()),
            error: None,
        };
        assert!(report_broadcast("deadbeef", &receipt, true).is_err());
    }
}
