//! `tx::builder` — native SOL transfer + Compute Budget auto-attach.
//!
//! Phase 3 lands two builders used by every Phase 7 CLI handler and
//! every Phantom-equivalent UX screen:
//!
//! | Builder                              | Returns                            | Used by |
//! |--------------------------------------|------------------------------------|---------|
//! | `build_sol_transfer`                 | 1-ix vec (system_instruction::transfer) | Phase 7 wallet send |
//! | `build_sol_transfer_with_budget`     | 3-ix vec (2 budget + 1 transfer)  | Phase 7 wallet send (default) |
//! | `compute_budget_instructions`        | 2-ix array (CU limit + CU price)  | Reused by Phase 4 SPL builder |
//!
//! Wire-format invariants (Phantom-equivalent parity):
//!   - The SOL transfer is the canonical `system_instruction::transfer`
//!     instruction (program ID `11111111111111111111111111111111`).
//!   - Compute Budget ix ALWAYS lands before the system ix —
//!     validators apply CU limits to the rest of the message and a
//!     late CU-limit ix would NOT constrain the transfer itself.
//!   - `priority_fee = 0` is the canonical "no priority fee" state
//!     (validators treat it identically to "no budget ix present"),
//!     but per Q8 the builder still emits the budget ix for UX parity
//!     with wallets that always show a fee slider.
//!
//! Deep-dive reference: `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md`
//! §D lines 1604–1640 (SOL transfer builder) + §C line 1592 (per-tx
//! CU default 150_000); plan §Phase 3 Task 3.1.

use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_system_interface::instruction as system_instruction;
#[cfg(test)]
use solana_system_interface::program::ID as SYSTEM_PROGRAM_ID;

/// Default compute unit limit per Phase 3.1 Step 4 + Q8.
///
/// Matches Solana's validator default (200_000) divided to leave headroom
/// for a single SOL transfer (Phantom-equivalent wallets send this when
/// the user does not override; the wallet CLI exposes `--cu-limit` to
/// raise or lower it).
pub const DEFAULT_COMPUTE_UNIT_LIMIT: u32 = 150_000;

/// Build a single SOL transfer instruction.
///
/// Wraps `solana_system_interface::instruction::transfer`. The
/// returned vec always has length 1 — Phase 4 SPL builders extend the
/// `Vec<Instruction>` shape so the surface is uniform across native +
/// SPL sends.
///
/// `lamports` is a raw `u64`; callers using a user-facing SOL decimal
/// should convert via [`crate::amount::Amount::from_sol`] first to
/// reject NaN/Inf/overflow at the parser boundary.
pub fn build_sol_transfer(from: &Pubkey, to: &Pubkey, lamports: u64) -> Vec<Instruction> {
    vec![system_instruction::transfer(from, to, lamports)]
}

/// Compute Budget instruction pair (CU limit + CU price).
///
/// Returned as a 2-element array so callers can spread it into a
/// builder result. Wire order matters: the validator applies the CU
/// limit to the rest of the message, so this MUST land before the
/// payload instructions.
///
/// `priority_fee_micro_lamports = 0` is the canonical "no priority
/// fee" state — the validator processes a zero-priced CU-price ix
/// identically to "no budget ix present" but the budget ix is still
/// emitted for UX parity (Phantom-equivalent wallets always show a
/// fee slider with a default).
pub fn compute_budget_instructions(
    units: u32,
    priority_fee_micro_lamports: u64,
) -> [Instruction; 2] {
    [
        ComputeBudgetInstruction::set_compute_unit_limit(units),
        ComputeBudgetInstruction::set_compute_unit_price(priority_fee_micro_lamports),
    ]
}

/// Build a SOL transfer prepended with Compute Budget instructions.
///
/// Returns `[set_cu_limit, set_cu_price, transfer]` — the order is
/// the wire-format invariant: budget ix first so the validator
/// applies CU limits to the transfer that follows. Phantom's "Send"
/// screen submits exactly this 3-ix layout under the default
/// `--cu-limit 150000 --priority-fee 0` flags.
///
/// `cu_limit` and `priority_fee` use the [`DEFAULT_COMPUTE_UNIT_LIMIT`]
/// + `priority_fee = 0` defaults from Q8 when callers pass through
///
/// `SolanaConfig` (Phase 7.1 wires that config to these args).
pub fn build_sol_transfer_with_budget(
    from: &Pubkey,
    to: &Pubkey,
    lamports: u64,
    cu_limit: u32,
    priority_fee_micro_lamports: u64,
) -> Vec<Instruction> {
    let budget = compute_budget_instructions(cu_limit, priority_fee_micro_lamports);
    vec![
        budget[0].clone(),
        budget[1].clone(),
        system_instruction::transfer(from, to, lamports),
    ]
}

#[cfg(test)]
mod tests {
    //! Unit tests for the SOL builders — round-trip + bincode wire-format
    //! coverage lives in `tests/tx_serde.rs`; budget variant + default
    //! constant lives in `tests/compute_budget.rs`.

    use super::*;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn build_sol_transfer_emits_one_system_instruction() {
        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        let ixs = build_sol_transfer(&from, &to, 1_000_000);
        assert_eq!(ixs.len(), 1);
        assert_eq!(ixs[0].program_id, SYSTEM_PROGRAM_ID);
    }

    #[test]
    fn compute_budget_instructions_returns_two_ixs_in_canonical_order() {
        let ixs = compute_budget_instructions(200_000, 1_000);
        assert_eq!(ixs[0].program_id, solana_compute_budget_interface::ID);
        assert_eq!(ixs[1].program_id, solana_compute_budget_interface::ID);
        // Data byte 0 differs between the two variants — confirms the
        // order is preserved (limit ix = 0x02 tag, price ix = 0x03 tag
        // per ComputeBudgetInstruction bincode encoding).
        assert_ne!(
            ixs[0].data[0], ixs[1].data[0],
            "CU-limit and CU-price must have distinct variant tags"
        );
    }

    #[test]
    fn build_sol_transfer_with_budget_emits_three_ixs_in_order() {
        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        let ixs = build_sol_transfer_with_budget(&from, &to, 2_500_000_000, 200_000, 5_000);
        assert_eq!(ixs.len(), 3);
        assert_eq!(ixs[0].program_id, solana_compute_budget_interface::ID);
        assert_eq!(ixs[1].program_id, solana_compute_budget_interface::ID);
        assert_eq!(ixs[2].program_id, SYSTEM_PROGRAM_ID);
    }
}
