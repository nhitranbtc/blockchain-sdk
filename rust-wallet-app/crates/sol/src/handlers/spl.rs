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
    let unlocked = ctx.wallet_manager.unlock(id, &password)?;
    let wallet = unlocked.wallet();

    let dest_pubkey: solana_sdk::pubkey::Pubkey = to_str
        .parse()
        .map_err(|e| anyhow!("invalid --to base58 pubkey: {e}"))?;
    let mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;
    let amount_raw: u64 = amount_str
        .parse()
        .map_err(|e| anyhow!("--amount not a u64: {e}"))?;

    // SPL Classic only (Token-2022 dispatch deferred).
    let program_id = sol_wallet_core::disambig::classic_token_program_id();
    let from_pubkey = wallet.public_key();
    let source_ata =
        sol_wallet_core::tx::builder::derive_ata_with_program_id(&from_pubkey, &mint, &program_id);
    let dest_ata =
        sol_wallet_core::tx::builder::derive_ata_with_program_id(&dest_pubkey, &mint, &program_id);

    // Fetch decimals from on-chain mint account.
    use sol_wallet_core::chain::{account::get_account_info, client::RpcClient};
    use sol_wallet_core::disambig::TokenProgram;
    use sol_wallet_core::tokens::decimals_from_state_bytes;
    let rpc = RpcClient::new(&ctx.rpc_url)
        .map_err(|e| anyhow::Error::new(e).context("RpcClient::new"))?;
    let mint_acct = get_account_info(&rpc, &mint)
        .await
        .map_err(|e| anyhow::Error::new(e).context("get_account_info(mint)"))?
        .ok_or_else(|| anyhow!("mint account not found: {mint}"))?;
    let decimals = decimals_from_state_bytes(&mint_acct.data, TokenProgram::Classic)
        .map_err(|e| anyhow!("decimals_from_state_bytes: {e}"))?;

    let mut ixs = Vec::with_capacity(4);
    ixs.push(sol_wallet_core::tx::builder::prepend_create_ata(
        &from_pubkey,
        &dest_pubkey,
        &mint,
        &program_id,
    ));
    ixs.extend(sol_wallet_core::tx::builder::compute_budget_instructions(
        150_000, 0,
    ));
    ixs.extend(sol_wallet_core::tx::builder::build_spl_transfer_checked(
        &source_ata,
        &mint,
        &dest_ata,
        &from_pubkey,
        TokenProgram::Classic,
        amount_raw,
        decimals,
    ));

    use sol_wallet_core::chain::account::get_latest_blockhash;
    use sol_wallet_core::tx::broadcast::send_and_confirm;
    use solana_commitment_config::CommitmentConfig;
    use solana_sdk::message::v0::Message as V0Message;
    use solana_sdk::message::VersionedMessage;
    use solana_sdk::transaction::VersionedTransaction;

    let (blockhash, _slot) = get_latest_blockhash(&rpc)
        .await
        .map_err(|e| anyhow::Error::new(e).context("get_latest_blockhash"))?;
    let v0_msg = V0Message::try_compile(&from_pubkey, &ixs, &[], blockhash)
        .map_err(|e| anyhow!("compile v0 message: {e}"))?;
    let unsigned = VersionedTransaction {
        signatures: vec![solana_sdk::signature::Signature::default()],
        message: VersionedMessage::V0(v0_msg),
    };
    let signed = wallet
        .sign_transaction(unsigned)
        .map_err(|e| anyhow::Error::new(e).context("sign_transaction"))?;
    let signature = send_and_confirm(
        &rpc,
        &signed,
        serde_json::json!({"encoding": "base64", "skipPreflight": true}),
        CommitmentConfig::confirmed(),
        std::time::Duration::from_secs(60),
    )
    .await
    .map_err(|e| anyhow::Error::new(e).context("send_and_confirm"))?;

    println!("{signature}");
    Ok(())
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
    let unlocked = ctx.wallet_manager.unlock(id, &password)?;
    let wallet = unlocked.wallet();

    let mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;
    let delegate: solana_sdk::pubkey::Pubkey = delegate
        .parse()
        .map_err(|e| anyhow!("--delegate invalid base58 pubkey: {e}"))?;
    let amount_raw: u64 = amount
        .parse()
        .map_err(|e| anyhow!("--amount not a u64: {e}"))?;

    let program_id = sol_wallet_core::disambig::classic_token_program_id();
    let owner_pubkey = wallet.public_key();
    let source_ata =
        sol_wallet_core::tx::builder::derive_ata_with_program_id(&owner_pubkey, &mint, &program_id);

    let mut ixs = Vec::with_capacity(3);
    ixs.extend(sol_wallet_core::tx::builder::compute_budget_instructions(
        150_000, 0,
    ));
    ixs.extend(sol_wallet_core::tx::builder::build_spl_approve(
        &source_ata,
        &delegate,
        &owner_pubkey,
        sol_wallet_core::disambig::TokenProgram::Classic,
        amount_raw,
    ));

    use sol_wallet_core::chain::{account::get_latest_blockhash, client::RpcClient};
    use sol_wallet_core::tx::broadcast::send_and_confirm;
    use solana_commitment_config::CommitmentConfig;
    use solana_sdk::message::v0::Message as V0Message;
    use solana_sdk::message::VersionedMessage;
    use solana_sdk::transaction::VersionedTransaction;

    let rpc = RpcClient::new(&ctx.rpc_url)
        .map_err(|e| anyhow::Error::new(e).context("RpcClient::new"))?;
    let (blockhash, _slot) = get_latest_blockhash(&rpc)
        .await
        .map_err(|e| anyhow::Error::new(e).context("get_latest_blockhash"))?;
    let v0_msg = V0Message::try_compile(&owner_pubkey, &ixs, &[], blockhash)
        .map_err(|e| anyhow!("compile v0 message: {e}"))?;
    let unsigned = VersionedTransaction {
        signatures: vec![solana_sdk::signature::Signature::default()],
        message: VersionedMessage::V0(v0_msg),
    };
    let signed = wallet
        .sign_transaction(unsigned)
        .map_err(|e| anyhow::Error::new(e).context("sign_transaction"))?;
    let signature = send_and_confirm(
        &rpc,
        &signed,
        serde_json::json!({"encoding": "base64", "skipPreflight": true}),
        CommitmentConfig::confirmed(),
        std::time::Duration::from_secs(60),
    )
    .await
    .map_err(|e| anyhow::Error::new(e).context("send_and_confirm"))?;

    println!("{signature}");
    Ok(())
}

async fn balance(ctx: &AppContext, address: &str, token: &str) -> Result<()> {
    // P7-2 safe path: read-only, no keypair resolution needed.
    let owner: solana_sdk::pubkey::Pubkey = address
        .parse()
        .map_err(|e| anyhow!("--address invalid base58: {e}"))?;
    let mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;

    let rpc = sol_wallet_core::chain::RpcClient::new(&ctx.rpc_url)
        .map_err(|e| anyhow!("RpcClient::new: {e}"))?;

    // SPL Classic only (Token-2022 dispatch deferred).
    let program_id = sol_wallet_core::disambig::classic_token_program_id();
    let ata = sol_wallet_core::tx::builder::derive_ata_with_program_id(&owner, &mint, &program_id);

    let ui_amount = sol_wallet_core::chain::get_token_account_balance(&rpc, &ata)
        .await
        .map_err(|e| match e {
            sol_wallet_core::Error::Rpc { .. } => anyhow!(
                "ATA not found for owner={owner} mint={mint} (derive_ata={ata}); \
                 either the account has no ATA yet or the RPC rejected the request"
            ),
            other => anyhow!("get_token_account_balance: {other:?}"),
        })?;

    // UiTokenAmount carries both raw `amount` (u64 string) and `decimals`.
    // Print raw + UI amount + decimals so the operator sees the on-chain shape.
    println!(
        "{} {} (decimals={}, ata={})",
        ui_amount.amount, token, ui_amount.decimals, ata
    );
    Ok(())
}

async fn allowance(ctx: &AppContext, token: &str, owner: &str, delegate: &str) -> Result<()> {
    let mint: solana_sdk::pubkey::Pubkey = token
        .parse()
        .map_err(|e| anyhow!("--token invalid base58 mint: {e}"))?;
    let owner_pubkey: solana_sdk::pubkey::Pubkey = owner
        .parse()
        .map_err(|e| anyhow!("--owner invalid base58: {e}"))?;
    let delegate_query: solana_sdk::pubkey::Pubkey = delegate
        .parse()
        .map_err(|e| anyhow!("--delegate invalid base58: {e}"))?;

    let rpc = sol_wallet_core::chain::RpcClient::new(&ctx.rpc_url)
        .map_err(|e| anyhow!("RpcClient::new: {e}"))?;
    let program_id = sol_wallet_core::disambig::classic_token_program_id();
    let ata =
        sol_wallet_core::tx::builder::derive_ata_with_program_id(&owner_pubkey, &mint, &program_id);

    // Fetch the raw token-account state bytes and parse delegate + amount
    // via the library's canonical Account-state decoder. Both RPC lookup
    // and parser are core-owned; CLI only translates into human format.
    let acct = sol_wallet_core::chain::get_account_info(&rpc, &ata)
        .await
        .map_err(|e| anyhow!("get_account_info(ata): {e:?}"))?
        .ok_or_else(|| {
            anyhow!("ATA not found for owner={owner_pubkey} mint={mint} (derive_ata={ata})")
        })?;

    let (current_delegate, amount) = sol_wallet_core::tokens::delegate_amount_from_state_bytes(
        &acct.data,
        sol_wallet_core::disambig::TokenProgram::Classic,
    )
    .map_err(|e| anyhow!("delegate_amount_from_state_bytes: {e:?}"))?;

    match current_delegate {
        Some(d) if d == delegate_query => {
            println!(
                "allowance {} {} = {} (delegated to {d})",
                owner_pubkey, token, amount
            );
            Ok(())
        }
        Some(d) => Err(anyhow!(
            "ATA {ata} delegates to {d}, not the queried delegate {delegate_query}"
        )),
        None => Err(anyhow!(
            "ATA {ata} has no delegate set (queried delegate {delegate_query})"
        )),
    }
}
