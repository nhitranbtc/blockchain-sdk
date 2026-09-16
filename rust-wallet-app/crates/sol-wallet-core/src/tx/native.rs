//! `tx::send_native` — `prepare_sol_transfer_message` (Phase 5.1).
//!
//! Pure builder wrapper around the Phase 3 `tx::builder::build_sol_transfer_with_budget`.
//! No IO; the caller passes a freshly-fetched `blockhash` and gets back
//! a `Message` ready to sign + serialize into a `Transaction`.
//!
//! ## Wire-format invariant
//!
//! Matches Phantom's "Send" flow: 3-ix layout
//! `[set_cu_limit, set_cu_price, system_instruction::transfer]`. The
//! Compute Budget ix ALWAYS lands first so the validator applies the CU
//! limit to the transfer that follows. Phantom's default is
//! `--cu-limit 150000 --priority-fee 0` (150_000 CU, 0 microlamports).
//!
//! Importers: Phase 7 CLI `sol send` handler; `tests/send_native.rs`.

use solana_sdk::hash::Hash;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;

use crate::tx::builder;

/// Build the `Message` for a native SOL transfer (no IO).
///
/// `from` is the wallet's pubkey (the fee payer + transfer source).
/// `to` is the destination. `lamports` is the raw u64 amount.
/// `cu_limit` defaults to 150_000 (matches `tx::builder::DEFAULT_COMPUTE_UNIT_LIMIT`).
/// `priority_fee_micro_lamports` defaults to 0.
pub fn prepare_sol_transfer_message(
    from: &Pubkey,
    to: &Pubkey,
    lamports: u64,
    cu_limit: u32,
    priority_fee_micro_lamports: u64,
    recent_blockhash: Hash,
) -> Message {
    let ixs = builder::build_sol_transfer_with_budget(
        from,
        to,
        lamports,
        cu_limit,
        priority_fee_micro_lamports,
    );
    Message::new_with_blockhash(&ixs, Some(from), &recent_blockhash)
}
