//! `token2022_disambig` — Phase 4.1 (deep-dive rows 13, 14 part).
//!
//! Proves classic SPL (`TokenkegQ...`) and Token-2022 (`TokenzQdB...`)
//! live at distinct program IDs, that `disambig::reject_wrong_token_program`
//! catches the Q6 footgun at the parser boundary, and that
//! `Mint::unpack` reads the on-chain `decimals` byte at offset 44 of
//! the 82-byte Mint state for both classic and Token-2022.
//!
//! Pure unit-level coverage (no RPC); integration with Anza
//! `RpcClient` lives in Phase 5 `chain::account`.

use sol_wallet_core::address::pubkey_from_bytes;
use sol_wallet_core::disambig::{
    classic_token_program_id, reject_wrong_token_program, token_2022_program_id, TokenProgram,
};
use sol_wallet_core::error::Error;
use sol_wallet_core::tokens::decimals_from_state_bytes;
use sol_wallet_core::tx::builder::derive_ata_with_program_id;

const OWNER: [u8; 32] = [1u8; 32];
const MINT: [u8; 32] = [2u8; 32];

#[test]
fn classic_and_token2022_program_ids_are_distinct() {
    assert_ne!(
        classic_token_program_id(),
        token_2022_program_id(),
        "Q6 invariant: classic ≠ Token-2022 program ID"
    );
}

#[test]
fn token_program_from_program_id_resolves_both_variants() {
    assert_eq!(
        TokenProgram::from_program_id(&classic_token_program_id()).unwrap(),
        TokenProgram::Classic,
    );
    assert_eq!(
        TokenProgram::from_program_id(&token_2022_program_id()).unwrap(),
        TokenProgram::Token2022,
    );
}

#[test]
fn token_program_from_program_id_rejects_unknown_program() {
    let bogus = solana_sdk::pubkey::Pubkey::new_unique();
    let err = TokenProgram::from_program_id(&bogus).unwrap_err();
    assert!(
        matches!(err, Error::InvalidTokenProgram(_)),
        "expected Error::InvalidTokenProgram, got {err:?}"
    );
}

#[test]
fn reject_wrong_token_program_passes_when_programs_match() {
    reject_wrong_token_program(&classic_token_program_id(), &classic_token_program_id())
        .expect("matching classic programs must pass");
    reject_wrong_token_program(&token_2022_program_id(), &token_2022_program_id())
        .expect("matching token-2022 programs must pass");
}

#[test]
fn reject_wrong_token_program_errors_on_classic_vs_token2022_mismatch() {
    let err = reject_wrong_token_program(&token_2022_program_id(), &classic_token_program_id())
        .unwrap_err();
    assert!(
        matches!(err, Error::InvalidTokenProgram(_)),
        "expected Error::InvalidTokenProgram on swap, got {err:?}"
    );
}

#[test]
fn derive_ata_with_program_id_returns_distinct_addrs_per_program() {
    let owner = pubkey_from_bytes(OWNER);
    let mint = pubkey_from_bytes(MINT);

    let classic_ata = derive_ata_with_program_id(&owner, &mint, &classic_token_program_id());
    let token2022_ata = derive_ata_with_program_id(&owner, &mint, &token_2022_program_id());

    assert_ne!(
        classic_ata, token2022_ata,
        "Q6: classic ATA ≠ Token-2022 ATA for same (owner, mint)"
    );
}

#[test]
fn mint_unpack_reads_decimals_for_classic_state() {
    // 82-byte classic Mint state. Layout per spl_token::state::Mint:
    //   offset 0..4   : mint_authority (COption<Pubkey> — empty → 4 zero bytes)
    //   offset 4..36  : mint_authority key (32 bytes)
    //   offset 36..44 : supply (u64)
    //   offset 44     : decimals (u8)  ← Q10
    // We construct a synthetic 82-byte zero state and patch `decimals=6`.
    let mut data = vec![0u8; 82];
    data[44] = 6; // USDC-shaped: 6 decimals
    data[45] = 1; // is_initialized = true (Pack::unpack rejects zero-init accounts)
    let dec = decimals_from_state_bytes(&data, TokenProgram::Classic).expect("classic unpack");
    assert_eq!(
        dec, 6,
        "Q10: classic Mint::unpack must read the `decimals` byte at offset 44"
    );
}

#[test]
fn mint_unpack_reads_decimals_for_token2022_state() {
    // Token-2022 base Mint state shares the same 82-byte prefix before
    // any TLV extensions. Patch decimals=9 (JitoSOL-shaped).
    let mut data = vec![0u8; 82];
    data[44] = 9;
    data[45] = 1; // is_initialized = true (Pack::unpack rejects zero-init accounts)
    let dec = decimals_from_state_bytes(&data, TokenProgram::Token2022).expect("token-2022 unpack");
    assert_eq!(dec, 9);
}

#[test]
fn mint_unpack_rejects_truncated_state() {
    let too_short = vec![0u8; 10];
    let err = decimals_from_state_bytes(&too_short, TokenProgram::Classic).unwrap_err();
    assert!(
        matches!(err, Error::InvalidTokenState(_)),
        "expected Error::InvalidTokenState for truncated mint data, got {err:?}"
    );
}
