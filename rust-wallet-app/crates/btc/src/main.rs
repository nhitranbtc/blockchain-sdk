//! `btc` — Bitcoin wallet CLI (PR-2, #70).
//!
//! Subcommands:
//! - `btc wallet create` — generate mnemonic, persist encrypted wallet
//! - `btc wallet show` — load, decrypt, sync, print addresses + balance
//!
//! **L28 / F49**: mnemonic is routed to STDERR; wallet_id to STDOUT.

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod error;
mod handlers;

use cli::{
    Cli, Commands, ConfigAction, ConfigActionKind, DecryptAction, EncryptAction,
    FeeEstimatesAction, MessageAction, TxListAction, WalletAction, WalletActionKind,
};
use error::classify_anyhow;

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

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // A clap usage error is a user error (exit 1), not a network error
            // (exit 2). `--help` and `--version` keep clap's exit 0.
            let code = e.exit_code();
            // `print()` returns `io::Result<()>` and `Result` is `#[must_use]` —
            // the write result is dropped deliberately: the process exits either way.
            let _ = e.print();
            std::process::exit(if code == 0 {
                0
            } else {
                wallet_errors::ErrorCategory::User.exit_code()
            });
        }
    };
    let data_dir =
        match handlers::resolve_data_dir(cli.data_dir).context("failed to resolve data dir") {
            Ok(dir) => dir,
            Err(e) => {
                // Same stderr shape and the same classification as the dispatch
                // path below, so no error path keeps Rust's `Error: ` prefix.
                // The exit code does not move: this chain holds no typed
                // `bitcoin_wallet_core::Error`, so `classify_anyhow` falls back
                // to `User` (1) -- the same code `Termination` gave it before.
                eprintln!("{e:?}");
                std::process::exit(classify_anyhow(&e).exit_code());
            }
        };

    // Run the command dispatch in an async block so the error chain can be
    // classified into a cross-chain exit code (see `error::classify_anyhow`).
    let dispatch_result: Result<()> = async {
        match cli.command {
            Commands::Wallet(WalletAction { action }) => match action {
                WalletActionKind::Create {
                    words,
                    network,
                    password,
                    address_type,
                    confirm_yes,
                } => {
                    handlers::handle_create(
                        words,
                        network,
                        password,
                        address_type,
                        confirm_yes,
                        &data_dir,
                    )
                    .await
                }
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
                } => {
                    handlers::handle_wallet_send(
                        mnemonic,
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
                    )
                    .await
                }
                WalletActionKind::List { network, json } => {
                    handlers::handle_wallet_list(network, json, &data_dir)
                }
                WalletActionKind::Delete { id, network } => {
                    handlers::handle_wallet_delete(id, network, &data_dir)
                }
                WalletActionKind::Rename { id, to, network } => {
                    handlers::handle_wallet_rename(id, to, network, &data_dir)
                }
                WalletActionKind::BumpFee {
                    mnemonic,
                    network,
                    txid,
                    fee_rate_sat_per_vb,
                    esplora_url,
                    pin_spki,
                    confirm_yes,
                    dry_run,
                } => {
                    handlers::handle_wallet_bump_fee(
                        mnemonic,
                        network,
                        txid,
                        fee_rate_sat_per_vb,
                        esplora_url,
                        pin_spki,
                        confirm_yes,
                        dry_run,
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

    // Map the failure onto the shared cross-chain exit-code table
    // (`wallet_errors::ErrorCategory::exit_code()`). An error that never
    // reached the core is a CLI-level rejection and classifies as `User`.
    if let Err(e) = dispatch_result {
        eprintln!("{e:?}");
        std::process::exit(classify_anyhow(&e).exit_code());
    }
    Ok(())
}
