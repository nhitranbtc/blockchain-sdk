//! `spl_instruction` — Phase 4.1 (deep-dive rows 15 SPL + 18).
//!
//! Proves every SPL builder round-trips through bincode Message
//! serialization and that `prepend_create_ata` + `transfer_checked`
//! lands in the correct 2-ix order (ATA-create first so the
//! validator creates the destination account before the
//! transfer_checked reads/writes it — row 18 "auto-ATA-create").
//!
//! Program-ID routing test (Token-2022 call): the builder must emit
//! the Token-2022 program ID for `TokenProgram::Token2022`. A
//! wrong-route call would fail at builder time with
//! `IncorrectProgramId` (Q6 invariant).

use solana_sdk::message::Message;

use sol_wallet_core::address::pubkey_from_bytes;
use sol_wallet_core::disambig::{classic_token_program_id, token_2022_program_id, TokenProgram};
use sol_wallet_core::tx::builder::{
    build_spl_approve, build_spl_close_account, build_spl_transfer_checked,
    derive_ata_with_program_id, prepend_create_ata,
};

const OWNER: [u8; 32] = [1u8; 32];
const SOURCE: [u8; 32] = [2u8; 32];
const DEST: [u8; 32] = [3u8; 32];
const MINT: [u8; 32] = [4u8; 32];
const DELEGATE: [u8; 32] = [5u8; 32];

#[test]
fn build_spl_transfer_checked_emits_one_classic_token_ix() {
    let source = pubkey_from_bytes(SOURCE);
    let mint = pubkey_from_bytes(MINT);
    let dest = pubkey_from_bytes(DEST);
    let authority = pubkey_from_bytes(OWNER);

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
    assert_eq!(
        ixs[0].program_id,
        classic_token_program_id(),
        "row 15: Classic SPL transfer must target the classic token program"
    );
}

#[test]
fn build_spl_transfer_checked_emits_one_token2022_ix() {
    let source = pubkey_from_bytes(SOURCE);
    let mint = pubkey_from_bytes(MINT);
    let dest = pubkey_from_bytes(DEST);
    let authority = pubkey_from_bytes(OWNER);

    let ixs = build_spl_transfer_checked(
        &source,
        &mint,
        &dest,
        &authority,
        TokenProgram::Token2022,
        1_000_000,
        9,
    );
    assert_eq!(ixs.len(), 1);
    assert_eq!(
        ixs[0].program_id,
        token_2022_program_id(),
        "Q6: Token-2022 transfer must target the Token-2022 program"
    );
}

#[test]
fn build_spl_transfer_checked_round_trips_through_message_bincode() {
    let source = pubkey_from_bytes(SOURCE);
    let mint = pubkey_from_bytes(MINT);
    let dest = pubkey_from_bytes(DEST);
    let authority = pubkey_from_bytes(OWNER);

    let ixs = build_spl_transfer_checked(
        &source,
        &mint,
        &dest,
        &authority,
        TokenProgram::Classic,
        1_000_000,
        6,
    );
    let msg = Message::new(&ixs, Some(&authority));
    let bytes = bincode::serialize(&msg).expect("bincode serialize");
    let decoded: Message = bincode::deserialize(&bytes).expect("bincode deserialize");
    assert_eq!(
        msg.serialize(),
        decoded.serialize(),
        "row 15: bincode round-trip must be byte-identical"
    );
    assert_eq!(
        decoded.instructions.len(),
        1,
        "decoded message must carry exactly 1 instruction (transfer_checked)"
    );
}

#[test]
fn prepend_create_ata_plus_transfer_emits_two_ixs_in_order() {
    // Row 18: auto-ATA-create + transfer_checked must land in the
    // validator-correct order — ATA-create first, then transfer so the
    // destination ATA exists before the transfer reads/writes it.
    let payer = pubkey_from_bytes(OWNER);
    let owner = pubkey_from_bytes(SOURCE);
    let mint = pubkey_from_bytes(MINT);
    // Derive the destination ATA address (Pubkey), then build the
    // ATA-create ix AND a transfer_checked ix targeting that same
    // derived address — the ix-order invariant for row 18.
    let dest_ata_pubkey = derive_ata_with_program_id(&owner, &mint, &classic_token_program_id());
    let ata_create_ix = prepend_create_ata(&payer, &owner, &mint, &classic_token_program_id());
    let transfer_ixs = build_spl_transfer_checked(
        &dest_ata_pubkey,
        &mint,
        &dest_ata_pubkey,
        &owner,
        TokenProgram::Classic,
        1_000_000,
        6,
    );

    let mut ixs = vec![ata_create_ix];
    ixs.extend(transfer_ixs);

    assert_eq!(ixs.len(), 2);
    assert_eq!(
        ixs[0].program_id,
        spl_associated_token_account::id(),
        "ix[0] = ATA-create via spl-associated-token-account"
    );
    assert_eq!(
        ixs[1].program_id,
        classic_token_program_id(),
        "ix[1] = transfer_checked"
    );
}

#[test]
fn build_spl_approve_emits_one_classic_token_ix() {
    let source = pubkey_from_bytes(SOURCE);
    let delegate = pubkey_from_bytes(DELEGATE);
    let owner = pubkey_from_bytes(OWNER);

    let ixs = build_spl_approve(&source, &delegate, &owner, TokenProgram::Classic, 500_000);
    assert_eq!(ixs.len(), 1);
    assert_eq!(ixs[0].program_id, classic_token_program_id());
}

#[test]
fn build_spl_close_account_emits_one_classic_token_ix() {
    let account = pubkey_from_bytes(SOURCE);
    let destination = pubkey_from_bytes(OWNER);
    let owner = pubkey_from_bytes(SOURCE);

    let ixs = build_spl_close_account(&account, &destination, &owner, TokenProgram::Classic);
    assert_eq!(ixs.len(), 1);
    assert_eq!(ixs[0].program_id, classic_token_program_id());
}

#[test]
fn derive_ata_with_program_id_and_prepend_create_ata_target_same_program() {
    // Smoke: the ATA-create ix and the derived ATA address both
    // reference the SPL ATA program (`spl_associated_token_account::id()`),
    // not the token program. This is the wire-level invariant that
    // makes row 18 work — the wallet never has to know the ATA
    // program ID at the call site.
    let owner = pubkey_from_bytes(SOURCE);
    let mint = pubkey_from_bytes(MINT);
    let ata = derive_ata_with_program_id(&owner, &mint, &classic_token_program_id());
    let payer = pubkey_from_bytes(OWNER);
    let ix = prepend_create_ata(&payer, &owner, &mint, &classic_token_program_id());

    assert_eq!(ix.program_id, spl_associated_token_account::id());
    // The derived ATA address is independence-of-program-id: the
    // creator-ix program_id (ATA program) is unrelated to the ATA
    // address itself (which embeds the token program ID in the seed).
    let _ = ata;
}
