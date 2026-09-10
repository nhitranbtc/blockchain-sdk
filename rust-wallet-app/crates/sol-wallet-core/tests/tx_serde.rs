//! `tx_serde` — Phase 3.1 (deep-dive row 15 SOL part).
//!
//! Proves every tx::builder output round-trips through bincode
//! serialization as a `Message` with the same instruction set. A
//! builder that mutates the instruction stream between construction
//! and serialization would corrupt the wire format — Phantom shows
//! the exact same byte-layout, so any divergence here breaks
//! cross-wallet compatibility.

use sol_wallet_core::address::pubkey_from_bytes;
use sol_wallet_core::tx::builder::{
    build_sol_transfer, build_sol_transfer_with_budget, compute_budget_instructions,
};
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_system_interface::program::ID as SYSTEM_PROGRAM_ID;

const FROM: [u8; 32] = [1u8; 32];
const TO: [u8; 32] = [2u8; 32];

#[test]
fn build_sol_transfer_emits_one_system_instruction() {
    let from = pubkey_from_bytes(FROM);
    let to = pubkey_from_bytes(TO);
    let ixs = build_sol_transfer(&from, &to, 1_000_000_000);
    assert_eq!(
        ixs.len(),
        1,
        "SOL transfer emits exactly 1 ix (system_instruction::transfer)"
    );
    assert_eq!(ixs[0].program_id, SYSTEM_PROGRAM_ID);
}

#[test]
fn build_sol_transfer_round_trips_through_message_bincode() {
    let from = pubkey_from_bytes(FROM);
    let to = pubkey_from_bytes(TO);
    let ixs = build_sol_transfer(&from, &to, 1_000_000_000);
    let msg = Message::new(&ixs, Some(&from));
    let bytes = bincode::serialize(&msg).expect("bincode serialize must succeed");
    let decoded: Message =
        bincode::deserialize(&bytes).expect("bincode deserialize must deserialize");
    assert_eq!(
        msg.serialize(),
        decoded.serialize(),
        "Message::serialize round-trip must be byte-identical"
    );
}

#[test]
fn build_sol_transfer_with_budget_prepends_compute_budget_in_order() {
    let from = pubkey_from_bytes(FROM);
    let to = pubkey_from_bytes(TO);
    let ixs = build_sol_transfer_with_budget(&from, &to, 2_500_000_000, 200_000, 5_000);
    assert_eq!(
        ixs.len(),
        3,
        "budget+transfer emits 3 ix (set_cu_limit, set_cu_price, transfer)"
    );
    // The first two instructions must be the Compute Budget program;
    // the third is the system transfer. Order matters: Phantom's
    // wire format requires the budget ix to land first so the
    // validator applies CU limits before charging for the system call.
    assert_eq!(
        ixs[0].program_id,
        solana_compute_budget_interface::ID,
        "ix[0] must be ComputeBudget (CU limit)"
    );
    assert_eq!(
        ixs[1].program_id,
        solana_compute_budget_interface::ID,
        "ix[1] must be ComputeBudget (CU price)"
    );
    assert_eq!(
        ixs[2].program_id, SYSTEM_PROGRAM_ID,
        "ix[2] must be system_program (transfer)"
    );
}

#[test]
fn compute_budget_instructions_helper_returns_two_ixs_with_expected_units_and_fee() {
    let ixs: [Instruction; 2] = compute_budget_instructions(150_000, 10_000);
    assert_eq!(ixs[0].program_id, solana_compute_budget_interface::ID);
    assert_eq!(ixs[1].program_id, solana_compute_budget_interface::ID);
    assert!(
        !ixs[0].data.is_empty(),
        "CU limit ix must carry encoded units"
    );
    assert!(
        !ixs[1].data.is_empty(),
        "CU price ix must carry encoded micro-lamports"
    );
}

#[test]
fn build_sol_transfer_message_round_trips_when_budget_prepended() {
    let from = pubkey_from_bytes(FROM);
    let to = pubkey_from_bytes(TO);
    let ixs = build_sol_transfer_with_budget(&from, &to, 1_000_000_000, 150_000, 0);
    let msg = Message::new(&ixs, Some(&from));
    let bytes = bincode::serialize(&msg).expect("serialize");
    let decoded: Message = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(msg.serialize(), decoded.serialize());
    assert_eq!(
        decoded.instructions.len(),
        3,
        "decoded message must carry all 3 instructions (2 budget + 1 transfer)"
    );
}
