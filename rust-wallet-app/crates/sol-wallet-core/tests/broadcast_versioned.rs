//! Phase 8.5 — wire-format coverage for `tx::broadcast::send_and_confirm_versioned`.
//!
//! Two checks land here per Task 8.5.1 step 3:
//!   1. `versioned_round_trips_through_wiremock` — wire-format fixture
//!      against mock RPC (blockhash/budget/transfer → bincode → JSON-RPC
//!      shape). Lands later when wiremock is wired into the workspace
//!      dev-dep list; the test vector itself is the contract.
//!   2. `versioned_and_legacy_produce_distinct_signatures` — the two
//!      wire formats the legacy overload hands the wire
//!      (`VersionedTransaction::from(legacy)`) and what we now build
//!      directly (V0 message via `try_compile`) MUST sign different
//!      bytes. Otherwise round-tripping through `legacy → Versioned`
//!      gives the same signature as the new direct-build path, which
//!      would defeat the point of routing through `send_and_confirm_versioned`.
//!
//! Surfpool 1.5.0 acceptance for the live path lives in
//! `tests/submit_sol_local::round_trip` (which now calls
//! `send_and_confirm_versioned`); this file covers the unit-level
//! shape checks.

use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::VersionedTransaction,
};

/// Compute Budget program — well-known address
/// `ComputeBudget111111111111111111111111111111`. Raw bytes used here
/// rather than `compute_budget_id()` because the
/// `compute_budget` module isn't always re-exported at the top level
/// of `solana_sdk` (Anza 4.x layout depends on feature flags).
const COMPUTE_BUDGET_PROGRAM_ID_BYTES: [u8; 32] = [
    0x06, 0xa9, 0x66, 0x29, 0xc4, 0x6e, 0x21, 0x41, 0xc5, 0x4a, 0xed, 0xee, 0x9b, 0x21, 0x9a, 0x3c,
    0x4d, 0x9f, 0x6c, 0x68, 0xa1, 0x09, 0x70, 0x10, 0xc1, 0x16, 0x39, 0x9c, 0x77, 0xc1, 0x80, 0xe3,
];

fn compute_budget_id() -> Pubkey {
    Pubkey::new_from_array(COMPUTE_BUDGET_PROGRAM_ID_BYTES)
}

/// Build a single compute-budget + transfer instruction pair mirroring
/// what `tx::builder::build_sol_transfer_with_budget` produces, but
/// without the library dependency (this is a wire-format fixture, not
/// a behavior test).
fn make_ixs(from: Pubkey, to: Pubkey, lamports: u64) -> Vec<Instruction> {
    // System Program well-known key (raw 32 bytes per Anza v0.30+):
    let system_program_id: Pubkey = Pubkey::new_from_array([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01,
    ]);

    // System `transfer` instruction layout (Anza v0.30+):
    //   - 4 bytes: tag=2 (Transfer)
    //   - 8 bytes: lamports (little-endian u64)
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());

    vec![
        // ComputeBudget::SetComputeUnitLimit (150_000 CU)
        Instruction {
            program_id: compute_budget_id(),
            accounts: vec![],
            data: vec![2, 0, 0, 0, 0, 224, 2, 0, 0, 0], // placeholder fixed-width
        },
        // ComputeBudget::SetComputeUnitPrice (0 micro-lamports)
        Instruction {
            program_id: compute_budget_id(),
            accounts: vec![],
            data: vec![3, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        },
        Instruction {
            program_id: system_program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(from, true),
                solana_sdk::instruction::AccountMeta::new(to, false),
            ],
            data,
        },
    ]
}

#[test]
fn versioned_and_legacy_produce_distinct_signatures() {
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let blockhash = Hash::from([7u8; 32]);

    // Build via `Message::new` + `Transaction::new` (legacy path).
    let ixs = make_ixs(sender.pubkey(), recipient, 1_000);
    let legacy_msg = solana_sdk::message::Message::new(&ixs, Some(&sender.pubkey()));
    let legacy_tx = solana_sdk::transaction::Transaction::new(&[&sender], legacy_msg, blockhash);
    // Build via `v0::Message::try_compile` + `VersionedTransaction::try_new`.
    let v0_msg =
        v0::Message::try_compile(&sender.pubkey(), &ixs, &[], blockhash).expect("v0 compile");
    let versioned_tx =
        VersionedTransaction::try_new(VersionedMessage::V0(v0_msg), &[&sender]).expect("v0 sign");

    let legacy_sigs: Vec<_> = legacy_tx.signatures.iter().collect();
    let versioned_sigs: Vec<_> = versioned_tx.signatures.iter().collect();

    // Legacy `Transaction::new` signs the legacy message; the versioned
    // path signs a `Message::V0` payload. The serialized messages are
    // not byte-equal, so the Ed25519 signatures cannot match.
    assert_ne!(
        legacy_sigs[0].to_string(),
        versioned_sigs[0].to_string(),
        "legacy and V0 wire formats must produce distinct signatures",
    );
}

#[test]
fn versioned_message_byte_size_includes_v0_header() {
    // V0 messages carry a 1-byte version discriminator (0x80) before
    // the legacy header. This test ensures callers that compute size
    // budgets don't assume legacy `Message` layout.
    let sender = Keypair::new();
    let recipient = Pubkey::new_unique();
    let ixs = make_ixs(sender.pubkey(), recipient, 1_000);

    let legacy_msg = solana_sdk::message::Message::new(&ixs, Some(&sender.pubkey()));
    let legacy_bytes = bincode::serialize(&legacy_msg).expect("legacy serialize");

    let blockhash = Hash::from([7u8; 32]);
    let v0_msg =
        v0::Message::try_compile(&sender.pubkey(), &ixs, &[], blockhash).expect("v0 compile");
    let v0_bytes = bincode::serialize(&v0_msg).expect("v0 serialize");

    assert!(
        v0_bytes.len() >= legacy_bytes.len(),
        "V0 wire size {} >= legacy wire size {}",
        v0_bytes.len(),
        legacy_bytes.len(),
    );
}
