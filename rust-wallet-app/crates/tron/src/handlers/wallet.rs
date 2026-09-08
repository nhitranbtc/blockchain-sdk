//! `tron wallet` handlers — plan §Phase 6 Task 5.2.
//!
//! All nine subcommands are wired: the Phase 2/3/5 core carry-overs they needed.
//! Test module sits mid-file (after `send_speedup`); `#[allow]` suppresses
//! the `items_after_test_module` lint — tests use `super::*` so position
//! is moot.
#![allow(clippy::items_after_test_module)]
//! (`chain::get_account`, `tx::submit_*`, wallet-record metadata) landed
//! alongside this file.
//!
//! Two safety rules live here rather than in the core, because they are about
//! an operator at a terminal, not about the chain:
//!
//! - a mainnet send asks for a typed `yes`;
//! - the recovery phrase goes to STDERR, never STDOUT.

use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tron_wallet_core::address::Address;
use tron_wallet_core::config::Network;
use tron_wallet_core::keys::{derive_keypair, Language, Mnemonic, MnemonicType, SECRET_KEY_LEN};
use tron_wallet_core::tx::submit::{self, SubmitOptions};
use tron_wallet_core::wallet::{WalletKind, WalletManager};

use super::{
    confirm, derivation_path, effective_network, emit, format_units, network_of, open_client,
    open_storage, parse_decimal_amount, parse_wallet_id, resolve_mnemonic, resolve_password,
    unlock, CliError, Result,
};
use crate::cli::{Network as NetworkArg, UnitArg};
use zeroize::Zeroizing;

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
        let keypair = unlocked.raw_keypair()?;
        let address = Address::from_public_key(keypair.public_key())?.to_base58();
        eprintln!("imported a raw-key wallet: it has no recovery phrase to back up");
        emit(
            json,
            serde_json::json!({
                "wallet_id": id.to_hex(),
                "address": address,
                "name": name,
                "network": net.tag(),
                "kind": "private_key",
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
            "kind": "mnemonic",
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
    let address = match unlocked.kind() {
        WalletKind::Mnemonic => {
            let keypair = unlocked.keypair(&derivation)?;
            Address::from_public_key(keypair.public_key())?.to_base58()
        }
        WalletKind::PrivateKey => {
            let keypair = unlocked.raw_keypair()?;
            Address::from_public_key(keypair.public_key())?.to_base58()
        }
    };
    let summary = unlocked.summary();

    emit(
        json,
        serde_json::json!({
            "wallet_id": id,
            "address": address,
            "name": summary.name,
            "network": summary.network,
            "is_private_key": summary.is_private_key(),
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
                    "is_private_key": s.is_private_key(),
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
    confirm(
        &format!("delete wallet {id}? the encrypted phrase cannot be recovered"),
        confirm_yes,
    )?;
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
                    "exists": account.exists(),
                    "balance_sun": account.balance_sun,
                    "balance_trx": trx,
                }),
                &trx,
            );
            if !account.exists() {
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
    pub mnemonic_file: Option<PathBuf>,
    pub to: Option<String>,
    pub to_wallet: Option<String>,
    pub amount: String,
    pub unit: UnitArg,
    pub fee_limit: Option<i64>,
    pub dry_run: bool,
    pub sign_only: bool,
    pub wait: bool,
    pub wait_timeout: u64,
    pub wait_poll_interval: u64,
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
    let (owner, secret) = signer(data_dir, &args)?;

    if net == Network::Mainnet {
        confirm(
            &format!(
                "send {} TRX from {owner} to {recipient} on MAINNET",
                format_units(u128::from(amount_sun), 6)
            ),
            args.confirm_yes,
        )?;
    }

    let client = open_client(data_dir, args.network, args.rpc_url.clone())?;
    let opts = SubmitOptions {
        fee_limit_sun: crate::handlers::wallet::fee_limit_sun(args.fee_limit)?,
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

    let signed = submit::sign_prepared(&secret, &params)?;
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
        wait_and_report(
            &client,
            &signed.txid_hex(),
            args.wait_timeout,
            args.wait_poll_interval,
            json,
        )
        .await?;
    }
    Ok(())
}

/// Convert a CLI `--fee-limit` into the core's non-zero type.
///
/// The flag stays `i64` at the CLI surface (that is what operators and existing
/// scripts pass), and this is the one place a negative or zero value is refused
/// — so `SubmitOptions` never has to represent "a fee limit of zero", which is
/// indistinguishable from "no override" once it reaches the builder.
pub(crate) fn fee_limit_sun(fee_limit: Option<i64>) -> Result<Option<NonZeroU64>> {
    match fee_limit {
        None => Ok(None),
        Some(raw) => u64::try_from(raw)
            .ok()
            .and_then(NonZeroU64::new)
            .map(Some)
            .ok_or_else(|| {
                CliError::BadInput(format!("--fee-limit must be greater than zero, got {raw}"))
            }),
    }
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

/// Resolve the signing secret and its owner address from `--wallet-id`,
/// `--mnemonic`, or `--mnemonic-file`.
///
/// Returns the **secret bytes** rather than a `KeyPair` so a raw-key
/// wallet (no derivation path) and a mnemonic wallet (derive fresh at
/// `path`) can both flow through the same return type — `KeyPair` no
/// longer derives `Clone` (PR #545 review finding G), so we cannot
/// hand back an owned `KeyPair` from a borrowed raw-key one. The bytes
/// live inside `Zeroizing`, so the wrapper's own zero-on-drop applies
/// to this copy independently of the source.
fn signer(data_dir: &Path, args: &SendArgs) -> Result<(String, Zeroizing<[u8; SECRET_KEY_LEN]>)> {
    let path = derivation_path(None, 0)?;
    match (&args.wallet_id, &args.mnemonic, &args.mnemonic_file) {
        (Some(id), _, _) => {
            let storage = open_storage(data_dir)?;
            let manager = WalletManager::new(&storage);
            let wallet_id = parse_wallet_id(id)?;
            let password = resolve_password(args.password.clone(), "wallet passphrase: ")?;
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
            let mnemonic = resolve_mnemonic(Some(phrase.clone()), None)?;
            let keypair = derive_keypair(&mnemonic, "", &path)?;
            let owner = Address::from_public_key(keypair.public_key())?.to_base58();
            Ok((owner, keypair.secret_bytes().clone()))
        }
        (None, None, Some(path_buf)) => {
            let mnemonic = resolve_mnemonic(None, Some(path_buf.clone()))?;
            let keypair = derive_keypair(&mnemonic, "", &path)?;
            let owner = Address::from_public_key(keypair.public_key())?.to_base58();
            Ok((owner, keypair.secret_bytes().clone()))
        }
        (None, None, None) => Err(CliError::BadInput(
            "one of --wallet-id, --mnemonic, or --mnemonic-file is required".into(),
        )),
    }
}

/// Print a broadcast result, and turn a node-side rejection into exit 3.
///
/// A rejected transaction is not a success with a warning: a script reading
/// exit 0 as "sent" would ship goods for a transfer the chain refused.
///
/// [`CoreError::Node`] rather than `TransactionBuild`: the envelope was built
/// and signed correctly, and the refusal is the *node's* answer about chain
/// state (insufficient balance, bandwidth). Exit 3 sends operators to the chain
/// and their funding, where the fix is; exit 5 would send them to this code.
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
        Err(CliError::Core(tron_wallet_core::Error::Node(format!(
            "node rejected {txid}: {} {}",
            receipt.code.clone().unwrap_or_default(),
            receipt.message.clone().unwrap_or_default()
        ))))
    }
}

/// Poll until the transaction confirms, then print the receipt.
///
/// The interval is validated through the core helper so `wallet send --wait`
/// and `tx wait` cannot disagree about what a zero interval means; the core
/// returns [`tron_wallet_core::Error::Config`], which maps to exit 2.
pub(crate) async fn wait_and_report(
    client: &tron_wallet_core::chain::TronGridClient,
    txid: &str,
    timeout_secs: u64,
    poll_interval_secs: u64,
    json: bool,
) -> Result<()> {
    let poll_interval = Duration::from_secs(poll_interval_secs);
    submit::validate_poll_interval(poll_interval)?;
    let info = submit::wait_for_confirm(
        client,
        txid,
        Duration::from_secs(timeout_secs),
        poll_interval,
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
    confirm_yes: bool,
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    let net = effective_network(data_dir, network)?;
    if net == Network::Mainnet {
        confirm(
            &format!("re-broadcast {txid} from {wallet_id} on MAINNET with fee_limit {fee_limit}"),
            confirm_yes,
        )?;
    }
    let storage = open_storage(data_dir)?;
    let manager = WalletManager::new(&storage);
    let id = parse_wallet_id(&wallet_id)?;
    let password = resolve_password(password, "wallet passphrase: ")?;
    let unlocked = manager.unlock(id, &password)?;
    let secret = match unlocked.kind() {
        WalletKind::Mnemonic => {
            let keypair = unlocked.keypair(&derivation_path(None, 0)?)?;
            keypair.secret_bytes().clone()
        }
        WalletKind::PrivateKey => unlocked.raw_keypair()?.secret_bytes().clone(),
    };

    let client = open_client(data_dir, network, rpc_url)?;
    // TRON has no replace-by-fee: this is a *new* transaction with a new txid.
    eprintln!("note: TRON has no replace-by-fee — this broadcasts a new transaction");
    let limit = fee_limit_sun(Some(fee_limit))?.expect("non-zero already checked above");
    let submitted = submit::submit_send_speedup(&client, &secret, &txid, limit).await?;
    report_broadcast(&submitted.txid(), submitted.receipt(), json)
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

    #[test]
    fn report_broadcast_rejection_uses_node_exit() {
        let receipt = tron_wallet_core::tx::broadcast::BroadcastReceipt {
            code: Some("CONTRACT_VALIDATE_ERROR".into()),
            txid: Some("deadbeef".into()),
            message: Some("balance is not sufficient".into()),
            error: None,
        };
        let err = report_broadcast("deadbeef", &receipt, true).expect_err("rejection is an error");
        // The envelope was fine; the chain refused it. Exit 3 points the
        // operator at the node and their funding, not at this code (exit 5).
        assert!(matches!(
            err,
            CliError::Core(tron_wallet_core::Error::Node(_))
        ));
        assert_eq!(super::super::exit_code(&err), 3);
    }

    #[test]
    fn fee_limit_must_be_positive() {
        // The CLI keeps `i64`; this is the single place zero and negative are
        // refused, so `SubmitOptions` never carries a meaningless zero ceiling.
        assert!(fee_limit_sun(None).expect("none is allowed").is_none());
        assert_eq!(
            fee_limit_sun(Some(1_000_000))
                .expect("positive")
                .map(|n| n.get()),
            Some(1_000_000)
        );
        assert!(matches!(fee_limit_sun(Some(0)), Err(CliError::BadInput(_))));
        assert!(matches!(
            fee_limit_sun(Some(-1)),
            Err(CliError::BadInput(_))
        ));
    }

    #[test]
    fn send_speedup_requires_confirm_on_mainnet() {
        // `confirm` is the gate both mainnet paths share: with `confirm_yes` it
        // returns Ok without reading STDIN, without it a non-tty aborts. That
        // is the whole conditional `send_speedup` relies on, and it is checked
        // before any broadcast.
        confirm("speed-up on MAINNET", true).expect("--confirm-yes skips the prompt");
        assert!(
            matches!(
                confirm("speed-up on MAINNET", false),
                Err(CliError::BadInput(_))
            ),
            "a non-interactive run must refuse rather than broadcast unattended"
        );
    }

    /// `signer` accepts a mnemonic supplied via file path, not just argv.
    #[test]
    fn signer_accepts_mnemonic_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let phrase_file = dir.path().join("phrase.txt");
        std::fs::write(&phrase_file, VECTOR).expect("write phrase");

        let args = SendArgs {
            wallet_id: None,
            mnemonic: None,
            mnemonic_file: Some(phrase_file),
            to: None,
            to_wallet: None,
            amount: "0".to_string(),
            unit: UnitArg::Trx,
            fee_limit: None,
            dry_run: false,
            sign_only: false,
            wait: false,
            wait_timeout: 90,
            wait_poll_interval: 3,
            confirm_yes: false,
            password: None,
            network: None,
            rpc_url: None,
        };
        let (owner, secret) = signer(dir.path(), &args).expect("signer from file");
        assert!(owner.starts_with('T'));
        // Signer hands back the 32-byte secret wrapped in `Zeroizing`; the
        // public key is recoverable through the core's `keypair_from_secret_bytes`.
        assert_eq!(secret.len(), 32);
    }
}

/// `tron wallet address --pubkey <hex>` — derive a T-address from a
/// SEC1-encoded uncompressed secp256k1 public key.
///
/// Plan §Task 7.15 spike invariant: black-box tests assert on the
/// `tron wallet address --pubkey <hex>` round-trip. The hex form is the
/// uncompressed `04 || X(32) || Y(32)` 65-byte encoding; a leading `0x`
/// is stripped. The resulting T-address is printed to STDOUT (no JSON —
/// the canonical form IS the printable base58check).
pub fn address_from_pubkey(pubkey_hex: &str) -> Result<()> {
    let addr = Address::from_pubkey_hex(pubkey_hex)?;
    println!("{}", addr.to_base58());
    Ok(())
}
