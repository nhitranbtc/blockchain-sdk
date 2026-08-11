//! `btc` — Bitcoin wallet CLI (PR-2, #70).
//!
//! Subcommands:
//! - `btc wallet create` — generate mnemonic, persist encrypted wallet
//! - `btc wallet show` — load, decrypt, sync, print addresses + balance
//!
//! **L28 / F49**: mnemonic is routed to STDERR; wallet_id to STDOUT.

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod handlers;

use cli::{Cli, Commands, MessageAction, WalletAction, WalletActionKind};

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
    let data_dir = handlers::resolve_data_dir(cli.data_dir)?;

    match cli.command {
        Commands::Wallet(WalletAction { action }) => match action {
            WalletActionKind::Create {
                words,
                network,
                password,
            } => handlers::handle_create(words, network, password, &data_dir).await,
            WalletActionKind::Show {
                id,
                network,
                password,
                esplora_url,
                esplora_spki_pin,
            } => {
                handlers::handle_show(
                    id,
                    network,
                    password,
                    esplora_url,
                    esplora_spki_pin,
                    &data_dir,
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
    }
}
