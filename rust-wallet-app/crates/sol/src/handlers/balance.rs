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
            // SPL balance requires ATA derivation + token-account lookup.
            // Defer to Phase 7.2 alongside spl::balance.
            let _owner: solana_sdk::pubkey::Pubkey = address
                .parse()
                .map_err(|e| anyhow!("invalid --address base58: {e}"))?;
            let _mint: solana_sdk::pubkey::Pubkey = token
                .parse()
                .map_err(|e| anyhow!("invalid --token base58: {e}"))?;
            Err(sol_wallet_core::Error::Unimplemented(
                "sol balance --token — requires ATA derivation (Phase 7.2)",
            )
            .into())
        }
    }
}
