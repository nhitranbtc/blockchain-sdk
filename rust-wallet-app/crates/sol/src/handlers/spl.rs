//! SPL command dispatcher — Phase 7.1c.
//!
//! Input validation (P7-14 + P7-23) runs inline. Keypair resolution uses
//! `WalletManager::unlock → OwnedLock<Zeroizing<Keypair>>` per P7-2.
//! Actual RPC broadcast / signed-tx dispatch lands in Phase 7.2 with surfpool
//! integration (see `tests/submit_spl_local_*` `#[ignore]` stubs).
//!
//! Read-only paths (`balance`, `allowance`) build an `RpcClient` directly
//! from `AppContext::rpc_url` and call `chain::get_token_account_balance` +
//! `chain::get_token_accounts_by_owner`. They work against any reachable
//! Solana RPC; surfpool gates full e2e in 7.2.

use anyhow::{anyhow, Result};

use crate::cli::{Cli, SplCmd};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &SplCmd, ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        SplCmd::Send {
            wallet_id,
            to,
            amount,
            token,
            priority_fee: _,
            memo,
            skip_memo_required,
            i_understand_no_memo_enforcement,
            skip_ata_create: _,
        } => {
            send(
                ctx,
                wallet_id.as_deref(),
                to.as_deref(),
                amount.as_deref(),
                token,
                memo.as_deref(),
                *skip_memo_required,
                *i_understand_no_memo_enforcement,
            )
            .await
        }
        SplCmd::Approve {
            wallet_id,
            token,
            delegate,
            amount,
        } => approve(ctx, wallet_id.as_deref(), token, delegate, amount).await,
        SplCmd::Balance { address, token } => balance(ctx, address, token).await,
        SplCmd::Allowance {
            token,
            owner,
            delegate,
        } => allowance(ctx, token, owner, delegate).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn send(
    ctx: &AppContext,
    wallet_id: Option<&str>,
    to: Option<&str>,
    amount: Option<&str>,
    token: &str,
    memo: Option<&str>,
    skip_memo_required: bool,
    i_understand_no_memo_enforcement: bool,
) -> Result<()> {
    // P7-14: clap `requires` enforces this at parse time; defense in depth
    // in case a future contributor drops the clap constraint.
    if skip_memo_required && !i_understand_no_memo_enforcement {
        return Err(anyhow!(
            "--skip-memo-required requires --i-understand-no-memo-enforcement"
        ));
    }
    // P7-23: SPL Memo program accepts up to 566 bytes; reject longer + NUL.
    if let Some(m) = memo {
        if m.len() > 566 {
            return Err(anyhow!(
                "memo length {} exceeds SPL Memo program max 566 bytes",
                m.len()
            ));
        }
        if m.contains('\0') {
            return Err(anyhow!("memo may not contain NUL byte"));
        }
    }
    // Required flags.
    let wallet_id_str = wallet_id.ok_or_else(|| anyhow!("--wallet-id required for spl send"))?;
    let to_str = to.ok_or_else(|| anyhow!("--to required"))?;
    let amount_str = amount.ok_or_else(|| anyhow!("--amount required"))?;
    let _mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;
    let _dest: solana_sdk::pubkey::Pubkey = to_str
        .parse()
        .map_err(|e| anyhow!("--to invalid base58 pubkey: {e}"))?;
    let _amount_raw: u64 = amount_str
        .parse()
        .map_err(|e| anyhow!("--amount not a u64: {e}"))?;

    // P7-2: resolve keypair via OwnedLock (zeroizes on Drop).
    let id = crate::handlers::wallet::parse_wallet_id_pub(wallet_id_str)?;
    let password = crate::handlers::wallet::read_password_from_cli_pub()?;
    let _unlocked = ctx.wallet_manager.unlock(id, &password)?;

    // Full SPL transfer (mint decimals lookup + ATA derivation + RPC)
    // lands in Phase 7.2 (see tests/submit_spl_local_{held,fresh}.rs).
    Err(sol_wallet_core::Error::Unimplemented(
        "spl send broadcast — requires SPL decimals + ATA derivation + surfpool (Phase 7.2)",
    )
    .into())
}

async fn approve(
    ctx: &AppContext,
    wallet_id: Option<&str>,
    token: &str,
    delegate: &str,
    amount: &str,
) -> Result<()> {
    let wallet_id_str = wallet_id.ok_or_else(|| anyhow!("--wallet-id required for spl approve"))?;
    let _mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;
    let _delegate_pubkey: solana_sdk::pubkey::Pubkey = delegate
        .parse()
        .map_err(|e| anyhow!("--delegate invalid base58 pubkey: {e}"))?;
    let _amount_raw: u64 = amount
        .parse()
        .map_err(|e| anyhow!("--amount not a u64: {e}"))?;

    let id = crate::handlers::wallet::parse_wallet_id_pub(wallet_id_str)?;
    let password = crate::handlers::wallet::read_password_from_cli_pub()?;
    let _unlocked = ctx.wallet_manager.unlock(id, &password)?;

    // Delegate approval (build_spl_approve + send_and_confirm) lands in Phase 7.2.
    Err(sol_wallet_core::Error::Unimplemented(
        "spl approve broadcast — requires SPL delegation tx + surfpool (Phase 7.2)",
    )
    .into())
}

async fn balance(ctx: &AppContext, address: &str, token: &str) -> Result<()> {
    // P7-2 safe path: read-only, no keypair resolution needed.
    let _owner: solana_sdk::pubkey::Pubkey = address
        .parse()
        .map_err(|e| anyhow!("--address invalid base58: {e}"))?;
    let _mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;

    let _rpc = sol_wallet_core::chain::RpcClient::new(&ctx.rpc_url)
        .map_err(|e| anyhow!("RpcClient::new: {e}"))?;
    // Derive ATA + fetch balance. Full ATA derivation + Mint::unpack lives in
    // Phase 7.2 (chain::preflight::resolve_mint_decimals + chain::account::*).
    Err(sol_wallet_core::Error::Unimplemented(
        "spl balance query — requires ATA derivation + mint decimals (Phase 7.2)",
    )
    .into())
}

async fn allowance(_ctx: &AppContext, token: &str, owner: &str, delegate: &str) -> Result<()> {
    let _mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;
    let _owner_pubkey: solana_sdk::pubkey::Pubkey = owner
        .parse()
        .map_err(|e| anyhow!("--owner invalid base58: {e}"))?;
    let _delegate_pubkey: solana_sdk::pubkey::Pubkey = delegate
        .parse()
        .map_err(|e| anyhow!("--delegate invalid base58: {e}"))?;

    // Read-only: derive ATA(owner, mint) + fetch account info + parse delegate.
    Err(sol_wallet_core::Error::Unimplemented(
        "spl allowance query — requires ATA derivation + delegate parse (Phase 7.2)",
    )
    .into())
}
