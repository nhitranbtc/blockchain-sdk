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

use crate::disambig::{classic_token_program_id, token_2022_program_id, TokenProgram};

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

// =============================================================================
// Phase 4 — SPL token program builders (Q6 disambig + Q10 transfer_checked).
// =============================================================================
//
// These builders wrap `spl_token::instruction::*` +
// `spl_associated_token_account::*` so the wallet CLI / FFI surface
// never imports the SPL crates directly. The `TokenProgram` argument
// forces the caller to commit to classic vs Token-2022 at the
// builder site — the RPC layer (`chain::account::mint_token_program`,
// Phase 5) is the only path that fills it in from `mint.owner`.
// Passing the wrong program for the mint is the Q6 footgun;
// `disambig::reject_wrong_token_program` catches it at the parser
// boundary.
//
// Wire-format invariants (Phantom-equivalent parity):
//   - `transfer_checked` (NOT `transfer`) — the validator rejects
//     unchecked `transfer` if `decimals` ≠ mint.decimals. Q10
//     mandates `transfer_checked` for every Phase 7+ builder.
//   - ATA derivation seed = `[owner, token_program_id, mint]` —
//     different `token_program_id` ⇒ different ATA (Q6 +
//     `derive_ata_with_program_id` test).
//   - `prepend_create_ata(idempotent)` lands BEFORE the transfer so
//     the validator creates the destination ATA before the transfer
//     reads it. Idempotent variant: returns Ok(()) if ATA already
//     exists, no-op.
//   - Compute Budget is NOT prepended here — Phase 7 composes
//     `compute_budget_instructions` + these builders at the CLI
//     layer, matching the Phase 3 SOL builder shape.
//
// Deep-dive reference: `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md`
// §D lines 1640–1680 (SPL transfer + ATA lifecycle); plan
// §Phase 4 Task 4.1.

use spl_associated_token_account::get_associated_token_address_with_program_id;

/// Derive the Associated Token Account address for `(owner, mint)`
/// under a specific token program.
///
/// Q6 — classic SPL and Token-2022 produce DIFFERENT ATAs for the
/// same `(owner, mint)` because the token program ID is part of
/// the derivation seed. Pass
/// `&TokenProgram::Classic.program_id()` for classic SPL mints;
/// `&TokenProgram::Token2022.program_id()` for Token-2022 mints.
/// Mixing the two produces an ATA that the mint's token program
/// will NOT recognize as the user's token account.
pub fn derive_ata_with_program_id(
    owner: &Pubkey,
    mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Pubkey {
    get_associated_token_address_with_program_id(owner, mint, token_program_id)
}

/// Build the idempotent `create_associated_token_account` instruction.
///
/// The idempotent variant is the right primitive for wallet UX: it
/// returns success if the ATA already exists (no error), and creates
/// the ATA on-chain if it does not. Pair with
/// `build_spl_transfer_checked` for the auto-create-on-first-receive
/// flow that Phantom-equivalent wallets expose.
///
/// Caller must have already resolved the `token_program_id` via
/// `disambig::reject_wrong_token_program` (Phase 5 wires this to the
/// on-chain `mint.owner`).
pub fn prepend_create_ata(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Instruction {
    spl_associated_token_account::instruction::create_associated_token_account_idempotent(
        payer,
        owner,
        mint,
        token_program_id,
    )
}

/// Build a single SPL `transfer_checked` instruction.
///
/// Q10 — `transfer_checked` (NOT `transfer`) carries the mint's
/// `decimals` byte on the wire; the validator rejects any tx where
/// the claimed decimals do not match the on-chain mint. `transfer`
/// alone would silently truncate on a 6/9 mismatch. Always pull
/// `decimals` from `tokens::decimals_from_state_bytes` (or the
/// bundled registry) — never hardcode.
///
/// `token_program` = the program the mint actually lives under
/// (per Phase 5 `mint_token_program`). The builder embeds the
/// program ID into the ix; the wire form encodes the program ID
/// first, followed by the serialized `TransferChecked` args.
///
/// Returns a 1-element vec to keep the shape uniform across all
/// builders (`build_sol_transfer` returns 1,
/// `build_sol_transfer_with_budget` returns 3, this returns 1).
/// Phase 7 CLI concatenates these with Compute Budget + optional
/// `prepend_create_ata` to assemble full transactions.
pub fn build_spl_transfer_checked(
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    token_program: TokenProgram,
    amount: u64,
    decimals: u8,
) -> Vec<Instruction> {
    // Q6 dispatch — `spl_token::instruction::transfer_checked` validates
    // `program_id == spl_token::ID` (rejects Token-2022 with
    // `IncorrectProgramId`), and the Token-2022 sibling validates the
    // Token-2022 program ID. A wrong-route call fails at builder time
    // before the ix ever reaches the wire.
    let ix = match token_program {
        TokenProgram::Classic => spl_token::instruction::transfer_checked(
            &classic_token_program_id(),
            source,
            mint,
            destination,
            authority,
            &[authority],
            amount,
            decimals,
        )
        .expect("hard-coded spl_token::transfer_checked args — encode failure is an Anza API drift"),
        TokenProgram::Token2022 => spl_token_2022::instruction::transfer_checked(
            &token_2022_program_id(),
            source,
            mint,
            destination,
            authority,
            &[authority],
            amount,
            decimals,
        )
        .expect("hard-coded spl_token_2022::transfer_checked args — encode failure is an Anza API drift"),
    };
    vec![ix]
}

/// Build a single SPL `approve` instruction (delegate authorisation).
///
/// Approves `delegate` to transfer up to `amount` tokens from the
/// `source` ATA, signed by `owner`. Phantom-equivalent wallets
/// expose this for dApp delegation (e.g. swapping USDC on a DEX
/// without re-prompting for signature).
///
/// Pair with `spl allowance` view (`chain::spl_allowance`, Phase 7)
/// to read the current approved amount.
pub fn build_spl_approve(
    source: &Pubkey,
    delegate: &Pubkey,
    owner: &Pubkey,
    token_program: TokenProgram,
    amount: u64,
) -> Vec<Instruction> {
    let ix = match token_program {
        TokenProgram::Classic => spl_token::instruction::approve(
            &classic_token_program_id(),
            source,
            delegate,
            owner,
            &[owner],
            amount,
        )
        .expect("hard-coded spl_token::approve args — encode failure is an Anza API drift"),
        TokenProgram::Token2022 => spl_token_2022::instruction::approve(
            &token_2022_program_id(),
            source,
            delegate,
            owner,
            &[owner],
            amount,
        )
        .expect("hard-coded spl_token_2022::approve args — encode failure is an Anza API drift"),
    };
    vec![ix]
}

/// Build a single SPL `close_account` instruction (reclaim rent).
///
/// Closes the SPL token account at `account`, transferring the
/// account's lamport balance (rent) to `destination`. Phantom-
/// equivalent wallets expose this for the "remove token" UX.
/// After close, the ATA can be re-derived + recreated
/// idempotently without paying rent twice.
pub fn build_spl_close_account(
    account: &Pubkey,
    destination: &Pubkey,
    owner: &Pubkey,
    token_program: TokenProgram,
) -> Vec<Instruction> {
    let ix = match token_program {
        TokenProgram::Classic => spl_token::instruction::close_account(
            &classic_token_program_id(),
            account,
            destination,
            owner,
            &[owner],
        )
        .expect("hard-coded spl_token::close_account args — encode failure is an Anza API drift"),
        TokenProgram::Token2022 => spl_token_2022::instruction::close_account(
            &token_2022_program_id(),
            account,
            destination,
            owner,
            &[owner],
        )
        .expect(
            "hard-coded spl_token_2022::close_account args — encode failure is an Anza API drift",
        ),
    };
    vec![ix]
}

#[cfg(test)]
mod spl_tests {
    //! Unit tests for the SPL builders. Wire-format + bincode
    //! coverage lives in `tests/spl_instruction.rs` (Phase 4.1);
    //! disambig + ATA coverage lives in
    //! `tests/token2022_disambig.rs`.

    use super::*;
    use crate::disambig::{classic_token_program_id, token_2022_program_id};
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn derive_ata_with_program_id_returns_different_addrs_per_program() {
        let owner = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let classic_ata = derive_ata_with_program_id(&owner, &mint, &classic_token_program_id());
        let token2022_ata = derive_ata_with_program_id(&owner, &mint, &token_2022_program_id());
        assert_ne!(
            classic_ata, token2022_ata,
            "classic ATA ≠ Token-2022 ATA for same (owner, mint) — Q6 invariant"
        );
    }

    #[test]
    fn build_spl_transfer_checked_emits_one_ix_under_correct_program() {
        let source = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let dest = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let ixs = build_spl_transfer_checked(
            &source,
            &mint,
            &dest,
            &authority,
            TokenProgram::Classic,
            1_000_000,
            6,
        );
        assert_eq!(ixs.len(), 1);
        assert_eq!(ixs[0].program_id, classic_token_program_id());
    }

    #[test]
    fn build_spl_transfer_checked_emits_token2022_program_when_asked() {
        let source = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let dest = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let ixs = build_spl_transfer_checked(
            &source,
            &mint,
            &dest,
            &authority,
            TokenProgram::Token2022,
            1_000_000,
            9,
        );
        assert_eq!(ixs[0].program_id, token_2022_program_id());
    }

    #[test]
    fn build_spl_approve_emits_one_ix_under_correct_program() {
        let source = Pubkey::new_unique();
        let delegate = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let ixs = build_spl_approve(&source, &delegate, &owner, TokenProgram::Classic, 500_000);
        assert_eq!(ixs.len(), 1);
        assert_eq!(ixs[0].program_id, classic_token_program_id());
    }

    #[test]
    fn build_spl_close_account_emits_one_ix_under_correct_program() {
        let account = Pubkey::new_unique();
        let destination = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let ixs = build_spl_close_account(&account, &destination, &owner, TokenProgram::Classic);
        assert_eq!(ixs.len(), 1);
        assert_eq!(ixs[0].program_id, classic_token_program_id());
    }

    #[test]
    fn prepend_create_ata_emits_ata_program_ix() {
        let payer = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let ix = prepend_create_ata(&payer, &owner, &mint, &classic_token_program_id());
        assert_eq!(
            ix.program_id,
            spl_associated_token_account::id(),
            "ATA-create ix must target the ATA program (not the token program)"
        );
    }
}
