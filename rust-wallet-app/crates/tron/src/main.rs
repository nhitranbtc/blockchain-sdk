//! `tron` — TRON wallet CLI binary. Plan §Phase 6.
//!
//! `main` does three things and nothing else: parse, dispatch, map the error
//! to an exit code. Every behaviour lives in `handlers`, so the exit-code
//! contract in `handlers::exit_code` is the single place a script's
//! expectations can break.

mod cli;
mod handlers;

use clap::Parser;

use cli::{
    AddressAction, BalanceArgs, Cli, Commands, ConfigAction, Trc20Action, TxAction, WalletAction,
};
use handlers::{exit_code, Result};

fn main() {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: cannot start async runtime: {e}");
            std::process::exit(5);
        }
    };

    if let Err(err) = runtime.block_on(run(cli)) {
        // Diagnostics on STDERR so `--json` consumers never see them on STDOUT.
        eprintln!("error: {err}");
        std::process::exit(exit_code(&err));
    }
}

async fn run(cli: Cli) -> Result<()> {
    let data_dir = handlers::resolve_data_dir(cli.data_dir)?;

    match cli.command {
        Commands::Wallet(cmd) => match cmd.action {
            WalletAction::Create {
                words,
                name,
                network,
                password,
                json,
            } => handlers::wallet::create(&data_dir, words, name, network, password, json),
            WalletAction::Import {
                mnemonic,
                mnemonic_file,
                private_key_file,
                name,
                network,
                password,
                json,
            } => handlers::wallet::import(
                &data_dir,
                mnemonic,
                mnemonic_file,
                private_key_file,
                name,
                network,
                password,
                json,
            ),
            WalletAction::Show {
                id,
                password,
                path,
                json,
            } => handlers::wallet::show(&data_dir, id, password, path, json),
            WalletAction::List {
                json,
                all_networks,
                password,
                network,
            } => handlers::wallet::list(&data_dir, json, all_networks, password, network),
            WalletAction::Delete { id, confirm_yes } => {
                handlers::wallet::delete(&data_dir, id, confirm_yes)
            }
            WalletAction::Rename { id, to, password } => {
                handlers::wallet::rename(&data_dir, id, to, password)
            }
            WalletAction::Balance {
                wallet_id,
                address,
                token,
                password,
                network,
                rpc_url,
                json,
            } => {
                handlers::wallet::balance(
                    &data_dir, wallet_id, address, token, password, network, rpc_url, json,
                )
                .await
            }
            WalletAction::Send {
                wallet_id,
                mnemonic,
                mnemonic_file,
                to,
                to_wallet,
                amount,
                unit,
                fee_limit,
                dry_run,
                sign_only,
                wait,
                wait_timeout,
                wait_poll_interval,
                confirm_yes,
                password,
                network,
                rpc_url,
                json,
            } => {
                handlers::wallet::send(
                    &data_dir,
                    handlers::wallet::SendArgs {
                        wallet_id,
                        mnemonic,
                        mnemonic_file,
                        to,
                        to_wallet,
                        amount,
                        unit,
                        fee_limit,
                        dry_run,
                        sign_only,
                        wait,
                        wait_timeout,
                        wait_poll_interval,
                        confirm_yes,
                        password,
                        network,
                        rpc_url,
                    },
                    json,
                )
                .await
            }
            WalletAction::SendSpeedup {
                wallet_id,
                txid,
                fee_limit,
                confirm_yes,
                password,
                network,
                rpc_url,
                json,
            } => {
                handlers::wallet::send_speedup(
                    &data_dir,
                    wallet_id,
                    txid,
                    fee_limit,
                    password,
                    confirm_yes,
                    network,
                    rpc_url,
                    json,
                )
                .await
            }
        },

        Commands::Address(cmd) => match cmd.action {
            AddressAction::New {
                mnemonic,
                mnemonic_file,
                index,
                path,
                bip39_passphrase,
                json,
            } => {
                handlers::address::new(mnemonic, mnemonic_file, index, path, bip39_passphrase, json)
            }
            AddressAction::Xpub {
                wallet_id,
                password,
                path,
                json,
            } => handlers::address::xpub_cmd(&data_dir, wallet_id, password, path, json),
        },

        Commands::Balance(args) => balance(&data_dir, args).await,

        Commands::Trc20(cmd) => match cmd.action {
            Trc20Action::Send {
                mnemonic,
                mnemonic_file,
                wallet_id,
                contract,
                to,
                amount,
                fee_limit,
                password,
                network,
                rpc_url,
                confirm_yes,
                json,
            } => {
                handlers::trc20::send(
                    &data_dir,
                    mnemonic,
                    mnemonic_file,
                    wallet_id,
                    contract,
                    to,
                    amount,
                    fee_limit,
                    password,
                    network,
                    rpc_url,
                    confirm_yes,
                    json,
                )
                .await
            }
            Trc20Action::Approve {
                mnemonic,
                mnemonic_file,
                wallet_id,
                contract,
                spender,
                amount,
                fee_limit,
                password,
                network,
                rpc_url,
                confirm_yes,
                json,
            } => {
                handlers::trc20::approve(
                    &data_dir,
                    mnemonic,
                    mnemonic_file,
                    wallet_id,
                    contract,
                    spender,
                    amount,
                    fee_limit,
                    password,
                    network,
                    rpc_url,
                    confirm_yes,
                    json,
                )
                .await
            }
            Trc20Action::Balance {
                address,
                contract,
                network,
                rpc_url,
                json,
            } => {
                handlers::trc20::balance(&data_dir, address, contract, network, rpc_url, json).await
            }
            Trc20Action::Allowance {
                contract,
                owner,
                spender,
                network,
                rpc_url,
                json,
            } => {
                handlers::trc20::allowance(
                    &data_dir, contract, owner, spender, network, rpc_url, json,
                )
                .await
            }
        },

        Commands::Tx(cmd) => match cmd.action {
            TxAction::Get {
                txid,
                network,
                rpc_url,
                json,
            } => handlers::tx::get(&data_dir, txid, network, rpc_url, json).await,
            TxAction::Wait {
                txid,
                timeout,
                poll_interval,
                network,
                rpc_url,
                json,
            } => {
                handlers::tx::wait(
                    &data_dir,
                    txid,
                    timeout,
                    poll_interval,
                    network,
                    rpc_url,
                    json,
                )
                .await
            }
        },

        Commands::Config(cmd) => match cmd.action {
            ConfigAction::Show { json } => handlers::config::show(&data_dir, json),
            ConfigAction::SetRpc { url } => handlers::config::set_rpc(&data_dir, url),
            ConfigAction::SetNetwork { network } => {
                handlers::config::set_network(&data_dir, network)
            }
        },
    }
}

/// Thin wrapper so the `Commands::Balance` arm stays one line.
async fn balance(data_dir: &std::path::Path, args: BalanceArgs) -> Result<()> {
    handlers::balance::run(data_dir, args).await
}
