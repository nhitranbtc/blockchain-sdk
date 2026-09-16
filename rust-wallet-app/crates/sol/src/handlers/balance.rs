//! Balance command dispatcher — Phase 7.1d.
//!
//! Read-only RPC queries (no keypair resolution). Native SOL balance uses
//! `chain::get_balance`; SPL token balance uses `chain::get_token_account_balance`.

use anyhow::{anyhow, Result};
use sol_wallet_core::chain::get_balance;

use crate::cli::{BalanceCmd, Cli};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &BalanceCmd, ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        BalanceCmd::Sol { address, unit } => {
            let owner: solana_sdk::pubkey::Pubkey = address
                .parse()
                .map_err(|e| anyhow!("invalid --address base58: {e}"))?;
            if !unit.eq_ignore_ascii_case("sol") && !unit.eq_ignore_ascii_case("lamports") {
                return Err(anyhow!(
                    "unsupported --unit \"{unit}\" (use \"sol\" or \"lamports\")"
                ));
            }
            let rpc = sol_wallet_core::chain::RpcClient::new(&ctx.rpc_url)
                .map_err(|e| anyhow!("RpcClient::new: {e}"))?;
            let lamports = get_balance(&rpc, &owner)
                .await
                .map_err(|e| anyhow!("get_balance: {e}"))?;
            if unit.eq_ignore_ascii_case("lamports") {
                println!("{} {}", lamports, owner);
            } else {
                let sol = lamports as f64 / 1_000_000_000.0;
                println!("{} SOL ({})", sol, owner);
            }
            Ok(())
        }
        BalanceCmd::Spl { address, token } => {
            let owner: solana_sdk::pubkey::Pubkey = address
                .parse()
                .map_err(|e| anyhow!("invalid --address base58: {e}"))?;
            let mint: solana_sdk::pubkey::Pubkey = token
                .parse()
                .map_err(|e| anyhow!("invalid --token base58: {e}"))?;
            let rpc = sol_wallet_core::chain::RpcClient::new(&ctx.rpc_url)
                .map_err(|e| anyhow!("RpcClient::new: {e}"))?;
            let program_id = sol_wallet_core::disambig::classic_token_program_id();
            let ata = sol_wallet_core::tx::builder::derive_ata_with_program_id(
                &owner,
                &mint,
                &program_id,
            );
            let ui_amount = sol_wallet_core::chain::get_token_account_balance(&rpc, &ata)
                .await
                .map_err(|e| match e {
                    sol_wallet_core::Error::Rpc { .. } => {
                        anyhow!("ATA not found for owner={owner} mint={mint} (derive_ata={ata})")
                    }
                    other => anyhow!("get_token_account_balance: {other:?}"),
                })?;
            println!(
                "{} {} (decimals={}, ata={})",
                ui_amount.amount, token, ui_amount.decimals, ata
            );
            Ok(())
        }
    }
}
