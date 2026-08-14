//! `btc` — Bitcoin wallet CLI (PR-2, #70).
//!
//! Subcommands:
//! - `btc wallet create` — generate mnemonic, persist encrypted wallet
//! - `btc wallet show` — load, decrypt, sync, print addresses + balance
//!
//! **L28 / F49**: mnemonic is routed to STDERR; wallet_id to STDOUT.

use anyhow::{Context, Result};
use bitcoin_wallet_core::error::Error as LibError;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod handlers;

use cli::{
    Cli, Commands, ConfigAction, ConfigActionKind, DecryptAction, EncryptAction,
    FeeEstimatesAction, MessageAction, TxListAction, WalletAction, WalletActionKind,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Trace logs → STDERR so they don't pollute scriptable STDOUT output
    // (wallet_id on `create`, JSON on `show`).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let data_dir =
        handlers::resolve_data_dir(cli.data_dir).context("failed to resolve data dir")?;

    // Run the command dispatch in an async block so we can intercept the
    // error chain for Story 2 AC compliance: invalid mnemonic → exit code 2
    // (not anyhow's default exit 1).
    let dispatch_result: Result<()> = async {
        match cli.command {
            Commands::Wallet(WalletAction { action }) => match action {
                WalletActionKind::Create {
                    words,
                    network,
                    password,
                } => handlers::handle_create(words, network, password, &data_dir).await,
                WalletActionKind::Import {
                    mnemonic,
                    network,
                    password,
                } => handlers::handle_import(mnemonic, network, password, &data_dir).await,
                WalletActionKind::Show {
                    id,
                    network,
                    password,
                    esplora_url,
                    esplora_spki_pin,
                    db_path,
                } => {
                    handlers::handle_show(
                        id,
                        network,
                        password,
                        esplora_url,
                        esplora_spki_pin,
                        db_path,
                        &data_dir,
                    )
                    .await
                }
                WalletActionKind::Sync {
                    mnemonic,
                    network,
                    esplora_url,
                    pin_spki,
                } => handlers::handle_wallet_sync(mnemonic, network, esplora_url, pin_spki).await,
                WalletActionKind::Balance {
                    mnemonic,
                    network,
                    esplora_url,
                    pin_spki,
                } => {
                    handlers::handle_wallet_balance(mnemonic, network, esplora_url, pin_spki).await
                }
                WalletActionKind::Send {
                    mnemonic,
                    network,
                    address,
                    amount_sat,
                    esplora_url,
                    pin_spki,
                    fee_rate_sat_per_vb,
                } => {
                    handlers::handle_wallet_send(
                        mnemonic,
                        network,
                        address,
                        amount_sat,
                        esplora_url,
                        pin_spki,
                        fee_rate_sat_per_vb,
                    )
                    .await
                }
            },
            Commands::Message(MessageAction { action }) => match action {
                cli::MessageActionKind::Sign {
                    mnemonic,
                    network,
                    address,
                    message,
                } => handlers::handle_message_sign(mnemonic, network, address, message),
                cli::MessageActionKind::Verify {
                    address,
                    message,
                    signature,
                } => handlers::handle_message_verify(address, message, signature),
            },
            Commands::Encrypt(EncryptAction {
                password,
                password_file,
                password_stdin,
                r#in,
                out,
            }) => handlers::handle_encrypt(password, password_file, password_stdin, r#in, out),
            Commands::Decrypt(DecryptAction {
                password,
                password_file,
                password_stdin,
                r#in,
                out,
            }) => handlers::handle_decrypt(password, password_file, password_stdin, r#in, out),
            Commands::Config(ConfigAction { action }) => match action {
                ConfigActionKind::Show { json } => handlers::handle_config_show(json, &data_dir),
            },
            Commands::FeeEstimates(FeeEstimatesAction {
                network,
                esplora_url,
                pin_spki,
                json,
            }) => handlers::handle_fee_estimates(network, esplora_url, pin_spki, json).await,
            Commands::TxList(TxListAction {
                mnemonic,
                network,
                esplora_url,
                pin_spki,
                limit,
                json,
            }) => {
                handlers::handle_tx_list(mnemonic, network, esplora_url, pin_spki, limit, json)
                    .await
            }
        }
    }
    .await;

    // Per Story 2 AC: invalid mnemonic → exit code 2 (not anyhow default 1).
    // Walk the error chain looking for the lib-level InvalidMnemonic variant
    // (wrapped by handle_import's .context(...)? call).
    if let Err(e) = dispatch_result {
        let is_invalid_mnemonic = e.chain().any(|c| {
            matches!(
                c.downcast_ref::<LibError>(),
                Some(LibError::InvalidMnemonic(_))
            )
        });
        if is_invalid_mnemonic {
            eprintln!("{e:?}");
            std::process::exit(2);
        }
        return Err(e);
    }
    Ok(())
}
