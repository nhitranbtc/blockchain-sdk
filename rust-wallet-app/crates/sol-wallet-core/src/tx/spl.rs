//! `tx::send_token` — `prepare_spl_transfer_message` (Phase 5.1).
//!
//! Pure builder wrapper around the Phase 4 SPL builders. No IO; the
//! caller passes a freshly-fetched `blockhash` and gets back a `Message`
//! ready to sign + serialize into a `Transaction`.
//!
//! ## Wire-format invariant
//!
//! ALWAYS uses `transfer_checked` (NOT `transfer`) per Q10 — decimals
//! pulled from on-chain Mint state via `spl_token::state::Mint::unpack`
//! (NEVER hardcoded). The caller is responsible for resolving decimals
//! via `chain::preflight::resolve_mint_decimals` before calling.
//!
//! When `prepend_ata_create` is true, prepends the idempotent
//! `create_associated_token_account_idempotent` instruction so the
//! destination ATA is auto-created on-chain if it doesn't exist
//! (Phantom-equivalent behavior for first-time sends).
//!
//! Importers: Phase 7 CLI `sol send-token` handler; `tests/send_token.rs`.

use solana_sdk::hash::Hash;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;

use crate::disambig::TokenProgram;
use crate::tx::builder::{self, compute_budget_instructions, prepend_create_ata};

/// Build the `Message` for an SPL token transfer (no IO).
///
/// `wallet_pubkey` is the fee payer (the wallet's own pubkey).
/// `source_ata` and `dest_ata` are the source and destination Associated
/// Token Accounts (derived by the caller via `tx::builder::derive_ata_with_program_id`
/// — Q6: classic vs Token-2022 ATA differs).
/// `mint` is the SPL token mint.
/// `program` is the Token Program (Classic or Token-2022, disambiguated
/// from on-chain `mint.owner` per Q6).
/// `amount` is the raw u64 base units.
/// `decimals` is the mint's decimals (from `preflight::resolve_mint_decimals`).
/// `prepend_ata_create` adds the `create_associated_token_account_idempotent`
/// instruction before the transfer (for first-time sends).
pub fn prepare_spl_transfer_message(
    wallet_pubkey: &Pubkey,
    source_ata: &Pubkey,
    dest_ata: &Pubkey,
    mint: &Pubkey,
    program: TokenProgram,
    amount: u64,
    decimals: u8,
    cu_limit: u32,
    priority_fee_micro_lamports: u64,
    recent_blockhash: Hash,
    prepend_ata_create: bool,
) -> Message {
    let mut ixs: Vec<Instruction> = Vec::with_capacity(if prepend_ata_create { 5 } else { 4 });

    // 1. Optional ATA-create (idempotent) — lands BEFORE the transfer
    //    so the destination ATA exists when the validator reads it.
    if prepend_ata_create {
        let program_id = match program {
            TokenProgram::Classic => spl_token::id(),
            TokenProgram::Token2022 => spl_token_2022::id(),
        };
        ixs.push(prepend_create_ata(
            wallet_pubkey,
            dest_ata,
            mint,
            &program_id,
        ));
    }

    // 2. Compute Budget (CU limit + CU price) — ALWAYS before the
    //    transfer so the validator applies the limit to the transfer.
    let budget = compute_budget_instructions(cu_limit, priority_fee_micro_lamports);
    ixs.push(budget[0].clone());
    ixs.push(budget[1].clone());

    // 3. The `transfer_checked` itself. Q10 invariant: NEVER use
    //    `transfer` (unchecked); ALWAYS use `transfer_checked` with
    //    the mint's decimals byte on the wire.
    let transfer_ixs = builder::build_spl_transfer_checked(
        source_ata,
        mint,
        dest_ata,
        wallet_pubkey,
        program,
        amount,
        decimals,
    );
    ixs.push(transfer_ixs.into_iter().next().expect(
        "build_spl_transfer_checked returns exactly 1 ix for non-Token-2022-hook mints (Q10)",
    ));

    Message::new_with_blockhash(&ixs, Some(wallet_pubkey), &recent_blockhash)
}
