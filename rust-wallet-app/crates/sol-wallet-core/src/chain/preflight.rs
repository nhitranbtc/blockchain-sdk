//! `chain::preflight` — 5 preflight checks (Phase 5.1).
//!
//! Per plan doc Q4 (grilled decisions): LIBRARY, not a monolithic
//! `check_all()`. Phase 7 `sol send` handler picks which to run:
//!
//! | Command                           | Calls                                          |
//! |-----------------------------------|-----------------------------------------------|
//! | `sol send <addr> <amount>`       | `check_native_balance` only                    |
//! | `sol send-token <mint> ...`       | `check_ata_exists` + `resolve_mint_decimals` + `check_token_balance` |
//! | `sol send-token --create-ata`     | above + `check_rent_exempt(165)`               |
//!
//! Saves 4 RPC round-trips on a 0.001 SOL transfer that doesn't need
//! mint decimals or token balance checks.
//!
//! Importers: `tx::broadcast` (none directly — preflight is called by
//! Phase 7 CLI handlers BEFORE `send_and_confirm`); Phase 7 CLI;
//! `tests/preflight.rs` (5 unit tests against wiremock).

use crate::chain::account::UiTokenAmount;
use solana_program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Mint;

use crate::chain::account::{
    get_account_info, get_balance, get_minimum_balance_for_rent_exemption,
    get_token_account_balance,
};
use crate::chain::client::RpcClient;
use crate::error::{Error, Result};

/// Check that `pubkey` holds at least `needed_lamports + fee_lamports`.
///
/// Returns `Err(InsufficientFunds { needed, have })` if the balance
/// is short. `fee_lamports` covers the transaction fee (5000 lamports
/// default) + priority fee + rent. Callers can pass 5000 for a
/// conservative estimate.
pub async fn check_native_balance(
    client: &RpcClient,
    pubkey: &Pubkey,
    needed_lamports: u64,
    fee_lamports: u64,
) -> Result<()> {
    let have = get_balance(client, pubkey).await?;
    let total_needed = needed_lamports
        .checked_add(fee_lamports)
        .ok_or_else(|| Error::Transport("preflight: needed + fee overflow".to_string()))?;
    if have < total_needed {
        return Err(Error::InsufficientFunds {
            needed: total_needed,
            have,
        });
    }
    Ok(())
}

/// Check that `ata` holds at least the expected mint's tokens.
///
/// Returns the base-unit balance. Verifies the ATA's mint matches
/// `expected_mint` (returns `Error::Transport` if mismatched — caller
/// sees a clear "ATA is for the wrong mint" error).
pub async fn check_token_balance(
    client: &RpcClient,
    ata: &Pubkey,
    expected_mint: &Pubkey,
) -> Result<u64> {
    let account = get_account_info(client, ata)
        .await?
        .ok_or_else(|| Error::Transport("preflight: token account does not exist".to_string()))?;
    if account.owner != spl_token::id() && account.owner != spl_token_2022::id() {
        return Err(Error::Transport(format!(
            "preflight: token account owner is {} (not SPL token program)",
            account.owner
        )));
    }
    let mint_in_account = Pubkey::try_from(account.data.as_slice())
        .map_err(|e| Error::Transport(format!("preflight: parse token account mint: {e}")))?;
    if &mint_in_account != expected_mint {
        return Err(Error::Transport(format!(
            "preflight: ATA mint {} != expected {}",
            mint_in_account, expected_mint
        )));
    }
    let UiTokenAmount {
        amount,
        decimals: _,
        ui_amount: _,
        ui_amount_string: _,
    } = get_token_account_balance(client, ata).await?;
    amount
        .parse::<u64>()
        .map_err(|e| Error::Transport(format!("preflight: parse token amount: {e}")))
}

/// Check if an ATA exists on-chain.
///
/// Returns `Ok(true)` if `get_account_info` returns `Some(_)`, `Ok(false)`
/// if it returns `None`. Used by `sol send-token --create-ata` to decide
/// whether to prepend the `create_associated_token_account_idempotent`
/// instruction.
pub async fn check_ata_exists(client: &RpcClient, ata: &Pubkey) -> Result<bool> {
    Ok(get_account_info(client, ata).await?.is_some())
}

/// Read the mint's `decimals` field from on-chain Mint state (Q10 — NEVER
/// hardcoded). Unpacks the 82-byte Mint layout via `spl_token::state::Mint`.
///
/// Returns `Err(InvalidTokenState(...))` if the account data is not a
/// valid Mint (wrong size, wrong owner, etc).
pub async fn resolve_mint_decimals(client: &RpcClient, mint: &Pubkey) -> Result<u8> {
    let account = get_account_info(client, mint).await?.ok_or_else(|| {
        Error::InvalidTokenState(format!("resolve_mint_decimals: {mint} not found"))
    })?;
    if account.owner != spl_token::id() && account.owner != spl_token_2022::id() {
        return Err(Error::InvalidTokenState(format!(
            "resolve_mint_decimals: {mint} owner is not SPL token program"
        )));
    }
    if account.data.len() != Mint::LEN {
        return Err(Error::InvalidTokenState(format!(
            "resolve_mint_decimals: {mint} data is {} bytes, expected {}",
            account.data.len(),
            Mint::LEN
        )));
    }
    let mint_state = Mint::unpack(&account.data)
        .map_err(|e| Error::InvalidTokenState(format!("resolve_mint_decimals: unpack: {e}")))?;
    Ok(mint_state.decimals)
}

/// Compute the lamports required to make an account of `data_len`
/// bytes rent-exempt. Used for the auto-ATA-create preflight (165 bytes
/// for a standard SPL token account).
pub async fn check_rent_exempt(client: &RpcClient, data_len: usize) -> Result<u64> {
    get_minimum_balance_for_rent_exemption(client, data_len).await
}
